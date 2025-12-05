# Rust TCP Integration into lwIP

## 1. High-Level Architecture: The Opaque Pointer Strategy

**The Core Challenge**: Integration of Rust into a legacy C codebase like lwIP faces a fundamental hurdle: **Memory Layout Incompatibility**.
- C structs (`struct tcp_pcb`) are flat, contiguous blocks of memory.
- Rust structs (`TcpConnectionState`) is composed of sub-structs, enums, and have undefined internal padding unless `#[repr(C)]` is used (which we avoided to use idiomatic Rust).

To solve this without rewriting the entire stack, we implemented the **Opaque Pointer Strategy**.

### How It Works
1.  **C Side (The "Blind" Holder)**:
    - The C application calls `tcp_new()`.
    - It receives a `struct tcp_pcb*` pointer.
    - **Crucially**: C treats this *only* as a handle (an address). It never dereferences it to access fields like `pcb->state` or `pcb->snd_buf` directly.

2.  **Rust Side (The Owner)**:
    - Rust allocates the full state object (`TcpConnectionState`) on the heap using `Box::new()`.
    - We convert this Box into a raw pointer (`Box::into_raw()`).
    - This raw address is what we return to C.

3.  **The Bridge (FFI)**:
    - A thin "shim" layer sits between C and Rust.
    - Every time C needs to read/write state, it calls a function in the Bridge.
    - The Bridge casts the raw pointer back to a Rust reference (`&mut TcpConnectionState`) and performs the action.

### Why is this a problem? (Memory Layout Mismatch)

lwIP's C API relies on `struct tcp_pcb *` being a pointer to a specific C memory layout. Functions like `tcp_new()` return this pointer, and legacy C code often accesses fields directly.

**The Mismatch:**

1.  **C `struct tcp_pcb`**: A flat structure with fields at specific offsets.
    ```c
    struct tcp_pcb {
        struct tcp_pcb *next;   // Offset 0
        void *callback_arg;     // Offset 4/8
        enum tcp_state state;   // Offset 8/16
        // ...
        u16_t snd_buf;          // Offset ~40 (Hypothetical)
    };
    ```

2.  **Rust `TcpConnectionState`**: A composition of modular structs.
    ```rust
    struct TcpConnectionState {
        conn_mgmt: ConnectionManagementState, // Offset 0
        rod: ReliableOrderedDeliveryState,    // Offset ???
        flow_ctrl: FlowControlState,          // Offset ???
    }
    ```

**The Crash Scenario:**
If C code tries to read `pcb->snd_buf`, it blindly reads memory at "Offset 40". In the Rust struct, that offset might correspond to `rod.retransmit_timer` or be unmapped memory.

```c
// DANGEROUS C CODE (If accessing Rust pointer directly)
struct tcp_pcb *pcb = tcp_new(); // Returns pointer to Rust Box
if (pcb->snd_buf > 100) {        // CRASH/CORRUPTION! C reads garbage at offset 40
    // ...
}
```

**The Scale of the Problem:**
This wasn't an isolated edge case. We found **over 400** instances of direct struct access across the codebase that had to be refactored.

The problem was that **upper layers** (Socket API, Netconn API) and **lower layers** (IP layer, Driver interfaces) were *also* reaching into `struct tcp_pcb` to read state, check flags, or update buffers.

- `pcb->state`: Accessed ~250 times (e.g., `api_msg.c` checking if a connection is ESTABLISHED).
- `pcb->flags`: Accessed ~140 times (e.g., `sockets.c` enabling/disabling Nagle).
- `pcb->snd_buf`: Accessed by application layers to see if they can write more data.

Since these "outsider" files (`src/api/*.c`) are still compiled and linked against our Rust backend, their direct access attempts would cause immediate segfaults. We explicitly **did not** want to rewrite these layers in Rust because we mandated **strict lwIP API compatibility**. Existing applications expect standard C headers and linking behavior. Rewriting the Socket/Netconn API would have broken this promise. Thus, we needed a solution to intercept access *in place*.

This is why we went through all dereferencing the pointer and force all access through the FFI getters/setters.

---

## 2. Alternative Approaches & Why We Rejected Them

Before settling on the Opaque Pointer Strategy, we evaluated several other integration methods. Here is why they were discarded.

### 2.1. The "Layout Mirroring" Strategy (`#[repr(C)]`)
**The Approach**: Define a Rust struct that mimics the exact memory layout of the C `struct tcp_pcb` byte-for-byte using `#[repr(C)]`.

**Why it was rejected**:
- **Blocks Refactoring**: The main goal was to break the "spaghetti code" of the monolithic `tcp_pcb`. Mirroring the C layout forces Rust to adopt the exact same bad architecture, preventing the use of idiomatic Rust features like `Enums` (for state) and modular sub-structs.
- **Undefined Padding**: C compilers insert padding bytes between fields that vary by architecture. Ensuring Rust matches this padding exactly across all platforms is difficult and dangerous.

### 2.2. The "Sidecar" Strategy (State Synchronization)
**The Approach**: Keep the original C `struct tcp_pcb` and attach a Rust object to it (e.g., via a `void* user_data` field). Rust calculates logic and then writes the results back into the C struct so legacy code can read it.

**Why it was rejected**:
- **Synchronization Nightmares**: Two sources of truth. If C code updates `pcb->snd_wnd` and Rust code updates its internal window state, they will drift out of sync.
- **Race Conditions**: Ensuring atomic updates across both the C struct and the Rust sidecar is extremely scary.

### 2.3. The "Bindgen" Approach (Automatic Generation)
**The Approach**: Use tools like `bindgen` to automatically generate Rust struct definitions from the C headers.

**Why it was rejected**:
- **Inherits C's Problems**: Solves manual layout matching but still forces the use of the C architecture ("C code in Rust syntax").
- **No Modularity**: You cannot decompose the TCP logic into `FlowControl`, `CongestionControl`, etc., because `bindgen` generates one massive `struct tcp_pcb`. The project's goal was architectural improvement, not just language translation.

### 2.4. The "Clean Slate" Rewrite
**The Approach**: Rewrite the entire stack (including upper layers `api_msg.c` and lower layers `ip.c`) in Rust.

**Why it was rejected**:
- **API Compatibility Mandate**: The project required strict lwIP API compatibility. Existing applications link against C functions like `tcp_new()`, `tcp_bind()`, and expect C headers. A full rewrite would break the ecosystem of drivers and applications built on top of lwIP.
- **Scope Creep**: lwIP is massive. Rewriting the entire stack is scary.

---

## 3. Build System Integration

We modified the build system to seamlessly switch between the legacy C TCP implementation and our new Rust backend. This allows A/B testing and ensures we didn't break the original stack.

### CMake Integration (`src/Filelists.cmake`)

We introduced a toggle `LWIP_USE_RUST_TCP`.

1.  **Configuration Option**:
    ```cmake
    option(LWIP_USE_RUST_TCP "Use Rust TCP backend (wrapper) instead of legacy C" ON)
    ```

2.  **Source Selection (The Switch)**:
    This is where the magic happens. The build system effectively "swaps out the brain" of the TCP stack.
    - **Enabled**: We compile `wrapper.c` (our bridge) and ignore the original `tcp.c`.
    - **Disabled**: We compile the original `tcp.c` files, preserving the legacy behavior.
    ```cmake
    if (LWIP_USE_RUST_TCP)
        list(APPEND lwipcore_SRCS ${LWIP_DIR}/src/core/tcp_rust/wrapper.c)
    else()
        list(APPEND lwipcore_SRCS ${LWIP_DIR}/src/core/tcp.c ...)
    endif()
    ```

3.  **Rust Compilation**:
    We added a custom command to trigger the `cargo` build chain from within CMake. This compiles the Rust code into a static library (`liblwip_tcp_rust.a`).
    ```cmake
    add_custom_command(
        OUTPUT ${RUST_TCP_LIB}
        COMMAND cargo build --release
        WORKING_DIRECTORY ${RUST_TCP_DIR}
        ...
    )
    ```

4.  **Linking**:
    Finally, we link this static Rust library into the main `lwipcore` library. This makes the Rust symbols available to the C linker.
    *Note: On Linux, we also must link `libdl` because the Rust standard library depends on it.*

---

## 4. Codebase Modifications & The Access Problem

This was the most intrusive part of the integration. lwIP is an embedded stack designed for performance, meaning upper layers (Socket API) and lower layers (IP/Netif) is constantly accessing the tcp_pcb.

### The Problem: Direct Member Access
Legacy C code was littered with direct property access. This is fast in C but fatal for our integration because of the memory layout mismatch explained earlier.

```c
// OLD CODE (Direct Access) - Fast, but dangerous for us
if (pcb->snd_buf > 200) { ... }
```

### The Solution: Macro Abstraction
We refactored `src/include/lwip/tcp.h` to replace these direct accesses with macros. This allows us to "intercept" the access and route it to Rust without rewriting the calling logic.

#### 1. Redefining Accessors
We introduced conditional macros. If `LWIP_USE_RUST_TCP` is defined, the macro expands to a function call. If not, it expands to the original direct access (preserving zero-cost for legacy users).

```c
// src/include/lwip/tcp.h

#if LWIP_USE_RUST_TCP
  // Rust Integration: Dispatch to FFI function (Slower, Safe wrapper)
  #define tcp_sndbuf(pcb)          tcp_get_sndbuf_rust(pcb)
  #define tcp_state_get(pcb)       ((enum tcp_state)tcp_get_state_rust(pcb))
  #define tcp_nagle_disable(pcb)   tcp_set_flags_rust(pcb, TF_NODELAY)
#else
  // Legacy C: Direct struct access (Fast, Zero-overhead)
  #define tcp_sndbuf(pcb)          ((pcb)->snd_buf)
  #define tcp_state_get(pcb)       ((pcb)->state)
  #define tcp_nagle_disable(pcb)   ((pcb)->flags |= TF_NODELAY)
#endif
```

#### 2. Refactoring Call Sites (The "Hunt")
We had to manually identify every location in the codebase that touched a `tcp_pcb` field. For example, in `src/api/api_msg.c`:

**Before (Direct access):**
```c
// Crashes with Rust backend
if ((conn->pcb.tcp->snd_buf > TCP_SNDLOWAT) && ...
```

**After (Macro access):**
```c
// Works with both backends
if ((tcp_sndbuf(conn->pcb.tcp) > TCP_SNDLOWAT) && ...
```

This refactoring touched hundreds of lines but established a clean API boundary that didn't exist before.

#### 3. The Global PCB List Problem
Another major issue was that lwIP exposes global linked lists of PCBs (`tcp_active_pcbs`, `tcp_listen_pcbs`, etc.) directly to external modules.
- **The Problem**: C code iterates over these lists directly (`pcb = tcp_active_pcbs; while(pcb != NULL) ...`).
- **The Solution**: We had to implement these globals in Rust but expose them as `#[no_mangle] static mut` pointers that C can link against.
    ```rust
    // lib.rs
    #[no_mangle]
    pub static mut tcp_active_pcbs: *mut c_void = ptr::null_mut();
    ```
    Rust manages these lists internally (updating the `next` pointers), but provides the head pointer to C so legacy iteration logic (like in `memp.c` for stats) still works, but is unsafe which is why this area remains a high-risk point for future bugs in future work.

---

## 5. The Wrapper Layer (FFI)

The "Bridge" consists of two distinct halves that communicate across the language barrier.

### Half 1: The C Shim (`src/core/tcp_rust/wrapper.c`)
This file implements the *exact same* public function signatures as the original lwIP TCP stack. It acts as a "dumb forwarder."

```c
// wrapper.c

// 1. C Application calls this standard lwIP function
struct tcp_pcb * tcp_new(void) {
    // 2. Wrapper forwards it to the Rust "extern C" function
    return tcp_new_rust();
}

err_t tcp_bind(struct tcp_pcb *pcb, const ip_addr_t *ipaddr, u16_t port) {
    return tcp_bind_rust(pcb, ipaddr, port);
}
```

### Half 2: The Rust Entry Point (`src/core/tcp_rust/src/lib.rs`)
This is the "reception desk" on the Rust side. It exposes functions compatible with the C ABI.

**Key Technical Concepts:**

1.  **`#[no_mangle]`**:
    - Rust compilers normally "mangle" function names (e.g., `_ZN3tcp8new17h...`) to support features like generics and namespaces.
    - `#[no_mangle]` turns this off, ensuring the symbol in the binary is exactly `tcp_new_rust`, so the C linker can find it.

2.  **`unsafe extern "C"`**:
    - `extern "C"`: Tells Rust to use the standard C calling convention (placing arguments in specific registers/stack slots).
    - `unsafe`: We are accepting raw pointers from C. The compiler cannot guarantee these pointers are valid, so we must explicitly mark this block as unsafe.

3.  **The Cast (The Magic Trick)**:
    This is how we convert the opaque "void pointer" back into a usable Rust object.

    ```rust
    // lib.rs
    unsafe fn pcb_to_state<'a>(pcb: *const ffi::tcp_pcb) -> Option<&'a TcpConnectionState> {
        if pcb.is_null() { return None; }

        // 1. Cast raw C pointer to specific Rust Pointer type
        let rust_ptr = pcb as *const TcpConnectionState;

        // 2. Dereference (*) to get the object
        // 3. Borrow (&) to get a reference
        Some(&*rust_ptr)
    }
    ```

---

## 6. Trade-offs & Philosophies

Integrating two different paradigms (C vs Rust) required significant compromises.

### Violation of lwIP Philosophy: Heap Allocation
**The lwIP Way (Memory Pools)**:
- Traditional lwIP uses `memp` (Memory Pools). These are static arrays allocated at compile time.
- **Pros**: Deterministic (allocations never fail due to fragmentation), fast (O(1) allocation), no heap required.
- **Cons**: Inflexible (fixed maximum number of connections), fixed struct size.

**Our Rust Approach (Heap/Box)**:
- We use `Box::new()`, which uses the system allocator (`malloc`/Heap).
- **Reason**: Rust's ownership model is tied to its allocation strategy. To have a `TcpConnectionState` that owns its sub-components (`FlowControl`, `Demux`), a dynamic heap allocation is the most idiomatic and safe way in Rust. Fitting a dynamic Rust struct into a rigid C memory pool would require `unsafe` pointer casting that defeats the purpose of using Rust.
- **Trade-off**: We gain Memory Safety and Modularity, but we lose Determinism and introduce Heap Fragmentation.

### Performance Overhead
There is a measurable overhead to this architecture:
1.  **Function Call Overhead**: Replacing a simple pointer offset (`pcb->state`, 1 CPU instruction) with a function call (`tcp_get_state_rust`, jump + execute + return) is significantly slower in tight loops.
2.  **FFI Boundary**: The compiler cannot optimize across the C/Rust boundary (e.g., it cannot inline the Rust getter into the C loop).

However, we argue that for the *control plane* logic (TCP state transitions), correctness is more critical than raw cycle-counting.

---

## 7. Summary for Presentation

1.  **Build**: We used CMake to swap out the heart of lwIP (TCP) with a Rust library.
2.  **Interop**: We used an **Opaque Pointer** strategy so C never sees the Rust memory layout.
3.  **Refactor**: We replaced fragile direct struct access with **Macros** in `tcp.h`, creating a unified API.
4.  **Bridge**: A `wrapper.c` forwards calls to `extern "C"` Rust functions.
5.  **Cost**: We traded the static memory pools of lwIP for Rust's heap allocation to gain ownership and safety.

---

## 8. Why we thought it was a good idea (Motivation)

Despite the challenges, the initial motivation was strong and rooted in the superior architectural capabilities of Rust compared to C:

1.  **Memory Safety**: Network stacks are notorious for buffer overflows and use-after-free vulnerabilities. Rust's borrow checker guarantees memory safety at compile time (within the Rust boundary), eliminating entire classes of security bugs that plague C implementations.

2.  **Real Modularization vs. Monolithic C**:
    - **The C Reality**: The existing lwIP TCP implementation is effectively a "monolith." `tcp_in.c` contains thousands of lines of mixed logic where state, protocol handling, and buffer management are deeply intertwined.
    - **The Rust Advantage**: Rust's type system and ownership model enforce **true** modularity. We could define distinct structs for `FlowControl`, `CongestionControl`, and `Reliability`, enforcing clear boundaries via public/private interfaces. This prevents the "spaghetti code" effect common in C, where any part of the code might mutate any part of the state. The compiler forces you to respect these boundaries, making the complex TCP state machine much easier to reason about and maintain.

---

## 9. Honest Reflection: Is it worth it?

To be brutally honest: **replacing a core C component with Rust in a tightly coupled legacy codebase like lwIP is likely not a good idea for production.**

The engineering effort required was massive ("very hard engineering"):
- **Intrusiveness**: We had to touch hundreds of lines of C code (`tcp.h` macros, `api_msg.c` refactors) just to support the *possibility* of a different backend. This creates merge conflicts and maintenance burden for the upstream C project.
- **Complexity**: The "Opaque Pointer" dance adds significant complexity to debugging. You can no longer just inspect a `pcb` in GDB; you have to know to cast it to the Rust type.
- **Loss of Idioms**: We fought against the grain of lwIP (static memory pools vs. Rust's heap). This friction suggests that a rewrite of the *entire* stack in Rust would be cleaner than this hybrid "Frankenstein" approach.
- **Fragility**: The FFI boundary is unsafe. One wrong cast or one missed pointer update on the C side can crash the Rust side instantly, negating Rust's safety promises at the boundary.

**Conclusion**: While this proves it *can* be done, the high cost of integration and the resulting architectural complexity suggest that a clean-slate Rust implementation (or keeping it all C) is preferable to this hybrid integration especially for shorter time frames.
