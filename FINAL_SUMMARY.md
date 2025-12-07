# TCP Rust Modularization: Complete Technical Reference

> "We decomposed TCP into five components with non-overlapping write scopes, enforced by Rust's type system at compile time."

## What's the Problem?

### Traditional TCP = Spaghetti State

```
┌─────────────────────────────────────┐
│         Monolithic tcp_pcb          │
│  ┌───┬───┬───┬───┬───┬───┬───┐    │
│  │snd│rcv│cwnd│wnd│state│...│60+ │    │
│  └───┴───┴───┴───┴───┴───┴───┘    │
│         ↑   ↑   ↑   ↑   ↑          │
│    ANY FUNCTION CAN WRITE ANYWHERE  │
└─────────────────────────────────────┘
```

**Why is this bad?**

| Problem | Real-World Impact |
|---------|-------------------|
| **Bugs spread** | A congestion control bug can corrupt connection state |
| **Hard to test** | Need full TCP stack to test one component |
| **Hard to verify** | Can't prove correctness of isolated logic |
| **Slow innovation** | Changing ACK logic requires touching 10 files |
| **Hard to offload** | No clear boundaries for hardware acceleration |

## What's Our Solution?

### Five Disjoint Components

```
┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
│ ConnMgmt│ │   ROD   │ │FlowCtrl │ │CongCtrl │ │  Demux  │
├─────────┤ ├─────────┤ ├─────────┤ ├─────────┤ ├─────────┤
│ state   │ │ snd_nxt │ │ snd_wnd │ │ cwnd    │ │(no state│
│ 4-tuple │ │ rcv_nxt │ │ rcv_wnd │ │ ssthresh│ │  uses   │
│ timers  │ │ lastack │ │ scaling │ │         │ │ 4-tuple)│
│ options │ │ iss/irs │ │ persist │ │         │ │         │
└────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └─────────┘
     │           │           │           │
     ▼           ▼           ▼           ▼
  ONLY writes  ONLY writes  ONLY writes  ONLY writes
  to ConnMgmt  to ROD       to FlowCtrl  to CongCtrl
```

**The Rule:** Each component method can only write to its own state.

## Why Rust?

### Compile-Time Enforcement

```rust
impl ConnectionManagementState {
    pub fn on_syn(&mut self) {
        self.state = SynRcvd;     // ✅ Allowed (my field)
        self.snd_nxt = 100;       // ❌ COMPILE ERROR (ROD's field)
    }
}
```

**Key insight:** In C, you'd need discipline. In Rust, the compiler enforces it.

### Quick Comparison

| C (lwIP) | Rust (Ours) |
|----------|-------------|
| `pcb->state = SYN_RCVD; pcb->snd_nxt = x;` | Each component has its own `&mut self` |
| Runtime bugs if wrong field touched | Compile-time error if wrong field touched |
| Trust the programmer | Trust the type system |

## Architecture

### Original lwIP Structure (C)
```
src/core/
├── tcp.c       (~2,700 lines) - Connection management, timers, API
├── tcp_in.c    (~2,200 lines) - Input processing, state machine
├── tcp_out.c   (~1,800 lines) - Output, segment transmission
└── include/lwip/priv/tcp_priv.h - tcp_pcb structure (~300 bytes)
```

### New Rust Structure
```
src/core/tcp_rust/src/
├── lib.rs              (791 lines)  - FFI bridge, C-compatible exports
├── state.rs            (89 lines)   - TcpConnectionState composition
├── tcp_api.rs          (222 lines)  - High-level API orchestration
├── tcp_types.rs        (65 lines)   - Shared types (TcpFlags, TcpSegment, etc.)
├── tcp_proto.rs        (177 lines)  - Protocol constants, TcpHdr struct
└── components/
    ├── mod.rs                       - Component exports
    ├── connection_mgmt.rs (283 lines) - TCP state machine
    ├── rod.rs             (295 lines) - Reliable Ordered Delivery
    ├── flow_control.rs    (172 lines) - Window management
    └── congestion_control.rs (170 lines) - Congestion window
```

**Total: ~2,300 lines of Rust source + ~2,000 lines of tests**

## The 5 Components

### Design Principle
> Each component **owns its own state** and **only modifies its own state**.
> The API layer orchestrates calls across components.

### 1. ConnectionManagementState (`connection_mgmt.rs`)

**Purpose:** TCP state machine, connection lifecycle, connection tuple

**Owned State:**
```rust
pub struct ConnectionManagementState {
    // Connection Tuple
    pub local_ip: ip_addr_t,
    pub remote_ip: ip_addr_t,
    pub local_port: u16,
    pub remote_port: u16,

    // State Machine
    pub state: TcpState,  // 11 states: Closed → Listen → SynSent → ...

    // Keep-Alive
    pub keep_idle: u32,   // Default: 7200000ms (2 hours)
    pub keep_intvl: u32,  // Default: 75000ms
    pub keep_cnt: u32,    // Default: 9

    // Options
    pub mss: u16,         // Default: 536
    pub ttl: u8,          // Default: 255
    pub prio: u8,         // Default: 64
    pub flags: u16,
}
```

**Key Methods:**
| Method | Transition | Description |
|--------|------------|-------------|
| `on_bind()` | CLOSED→CLOSED | Store local IP/port |
| `on_listen()` | CLOSED→LISTEN | Start listening |
| `on_connect()` | CLOSED→SYN_SENT | Initiate active open |
| `on_syn_in_listen()` | LISTEN→SYN_RCVD | Received SYN |
| `on_synack_in_synsent()` | SYN_SENT→ESTABLISHED | Received SYN+ACK |
| `on_ack_in_synrcvd()` | SYN_RCVD→ESTABLISHED | Handshake complete |
| `on_close()` | Various→FIN states | Initiate close |
| `on_fin_in_established()` | ESTABLISHED→CLOSE_WAIT | Peer closed |
| `on_rst()` | ANY→CLOSED | Connection reset |
| `on_abort()` | ANY→CLOSED | Abort connection |

### 2. ReliableOrderedDeliveryState (`rod.rs`)

**Purpose:** Sequence numbers, acknowledgments, retransmission tracking

**Owned State:**
```rust
pub struct ReliableOrderedDeliveryState {
    // Sequence Numbers
    pub snd_nxt: u32,     // Next sequence number to send
    pub rcv_nxt: u32,     // Next sequence number expected
    pub lastack: u32,     // Last cumulative ACK received
    pub iss: u32,         // Initial Send Sequence
    pub irs: u32,         // Initial Receive Sequence

    // Send Buffer Tracking
    pub snd_lbb: u32,     // Next byte to buffer
    pub snd_buf: u16,     // Available send buffer space
    pub snd_queuelen: u16,// Queued pbuf count

    // RTT & Retransmission
    pub rto: i16,         // Retransmit timeout (default: 3000ms)
    pub nrtx: u8,         // Retransmit count
    pub dupacks: u8,      // Duplicate ACK count
}
```

**Key Methods:**
| Method | Description |
|--------|-------------|
| `on_connect()` | Generate ISS, initialize send state |
| `on_syn_in_listen()` | Store IRS from SYN, generate ISS |
| `on_synack_in_synsent()` | Validate ACK of our SYN, store IRS |
| `validate_sequence_number()` | RFC 793 sequence validation |
| `validate_ack()` | Returns: Duplicate, Valid, TooOld, TooNew |
| `validate_rst()` | RFC 5961 RST validation |

### 3. FlowControlState (`flow_control.rs`)

**Purpose:** Send and receive window management

**Owned State:**
```rust
pub struct FlowControlState {
    // Peer's Advertised Window
    pub snd_wnd: u16,         // Current send window
    pub snd_wnd_max: u16,     // Maximum seen
    pub snd_wl1: u32,         // Seq# for window update validation
    pub snd_wl2: u32,         // Ack# for window update validation

    // Our Receive Window
    pub rcv_wnd: u16,         // Available receive buffer
    pub rcv_ann_wnd: u16,     // Window to advertise

    // Window Scaling
    pub snd_scale: u8,
    pub rcv_scale: u8,
}
```

**Key Methods:**
| Method | Description |
|--------|-------------|
| `on_connect()` | Initialize rcv_wnd = 4096 |
| `on_syn_in_listen()` | Store peer's advertised window |
| `on_synack_in_synsent()` | Update window from SYN+ACK |

### 4. CongestionControlState (`congestion_control.rs`)

**Purpose:** Congestion window management

**Owned State:**
```rust
pub struct CongestionControlState {
    pub cwnd: u16,      // Congestion window
    pub ssthresh: u16,  // Slow start threshold (default: 0xFFFF)
}
```

**Key Methods:**
| Method | Description |
|--------|-------------|
| `on_connect()` | IW = min(4*MSS, max(2*MSS, 4380)) per RFC 5681 |
| `on_syn_in_listen()` | Same IW calculation |
| `on_synack_in_synsent()` | cwnd = MSS |

### 5. DemuxState (`mod.rs`)

**Purpose:** Connection demultiplexing (placeholder)

**Placeholder:**
```rust
pub struct DemuxState {}  // Currently empty - demux uses 4-tuple from conn_mgmt
```

# Rust TCP Integration into lwIP

## 1. Integration Principles

Before diving into implementation details, here are the guiding principles that shaped our integration approach:

### 1.1. Minimal Changes Outside TCP
We aimed to avoid modifications to the rest of the lwIP stack. Almost all code changes are confined to the `src/core/tcp_rust/` directory. The only external changes were:
- Macro definitions in `tcp.h` (to intercept field access)
- Build system toggles in `CMakeLists.txt` and `Filelists.cmake`
- Minor call-site refactors replacing direct struct access with macros

### 1.2. Preserve lwIP Design Principles
We tried to retain core lwIP philosophies where possible:
- **Callback-driven architecture**: The async callback model (`tcp_recv`, `tcp_sent`, `tcp_err`) remains unchanged
- **Zero-copy aspirations**: We preserve the `pbuf` interface for buffer management
- **Single-threaded core**: No threading or synchronization added to the TCP layer
- **Configurability**: The `LWIP_USE_RUST_TCP` toggle allows switching backends without recompilation of application code

### 1.3. Drop-in API Compatibility
The Rust TCP implementation must be a **drop-in replacement**. Applications using lwIP's Socket API, Netconn API, or Raw API should work without any code changes:
- Same function signatures: `tcp_new()`, `tcp_bind()`, `tcp_connect()`, `tcp_write()`, etc.
- Same header files: `#include "lwip/tcp.h"` works for both backends
- Same linking: Applications link against `lwipcore` regardless of backend

This means existing applications, drivers, and middleware built on lwIP can switch to the Rust TCP backend by simply toggling a CMake flag.

## 2. Build System Integration

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

## 3. The Wrapper Layer (FFI)

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

## 4. The Opaque Pointer Strategy

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

### Actual Code Implementation

**State → PCB** (creation):
```rust
pub unsafe extern "C" fn tcp_new_rust() -> *mut ffi::tcp_pcb {
    let state = Box::new(TcpConnectionState::new());
    Box::into_raw(state) as *mut ffi::tcp_pcb
}
```

**PCB → State** (conversion helpers):
```rust
unsafe fn pcb_to_state_mut<'a>(pcb: *mut ffi::tcp_pcb) -> Option<&'a mut TcpConnectionState> {
    if pcb.is_null() { None } else { Some(&mut *(pcb as *mut TcpConnectionState)) }
}
```

**Cleanup** (reclaim and drop):
```rust
pub unsafe extern "C" fn tcp_abort_rust(pcb: *mut ffi::tcp_pcb) {
    let _ = Box::from_raw(pcb as *mut TcpConnectionState); // Drops and frees memory
}
```

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

## 5. Codebase Modifications & The Access Problem

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

## 6. Alternative Approaches & Why We Rejected Them

Before settling on the Opaque Pointer Strategy, we evaluated several other integration methods. Here is why they were discarded.

### 6.1. The "Layout Mirroring" Strategy (`#[repr(C)]`)
**The Approach**: Define a Rust struct that mimics the exact memory layout of the C `struct tcp_pcb` byte-for-byte using `#[repr(C)]`.

**Why it was rejected**:
- **Blocks Refactoring**: The main goal was to break the "spaghetti code" of the monolithic `tcp_pcb`. Mirroring the C layout forces Rust to adopt the exact same bad architecture, preventing the use of idiomatic Rust features like `Enums` (for state) and modular sub-structs.
- **Undefined Padding**: C compilers insert padding bytes between fields that vary by architecture. Ensuring Rust matches this padding exactly across all platforms is difficult and dangerous.

### 6.2. The "Sidecar" Strategy (State Synchronization)
**The Approach**: Keep the original C `struct tcp_pcb` and attach a Rust object to it (e.g., via a `void* user_data` field). Rust calculates logic and then writes the results back into the C struct so legacy code can read it.

**Why it was rejected**:
- **Synchronization Nightmares**: Two sources of truth. If C code updates `pcb->snd_wnd` and Rust code updates its internal window state, they will drift out of sync.
- **Race Conditions**: Ensuring atomic updates across both the C struct and the Rust sidecar is extremely scary.

### 6.3. The "Bindgen" Approach (Automatic Generation)
**The Approach**: Use tools like `bindgen` to automatically generate Rust struct definitions from the C headers.

**Why it was rejected**:
- **Inherits C's Problems**: Solves manual layout matching but still forces the use of the C architecture ("C code in Rust syntax").
- **No Modularity**: You cannot decompose the TCP logic into `FlowControl`, `CongestionControl`, etc., because `bindgen` generates one massive `struct tcp_pcb`. The project's goal was architectural improvement, not just language translation.

### 6.4. The "Clean Slate" Rewrite
**The Approach**: Rewrite the entire stack (including upper layers `api_msg.c` and lower layers `ip.c`) in Rust.

**Why it was rejected**:
- **API Compatibility Mandate**: The project required strict lwIP API compatibility. Existing applications link against C functions like `tcp_new()`, `tcp_bind()`, and expect C headers. A full rewrite would break the ecosystem of drivers and applications built on top of lwIP.
- **Scope Creep**: lwIP is massive. Rewriting the entire stack is scary.

## 7. Trade-offs & Philosophies

Integrating two different paradigms (C vs Rust) required significant compromises.

### Violation of lwIP Philosophy: Heap Allocation
**The lwIP Way (Memory Pools)**:
- Traditional lwIP uses `memp` (Memory Pools). These are static arrays allocated at compile time.
- **Pros**: Deterministic (allocations never fail due to fragmentation), fast (O(1) allocation), no heap required.
- **Cons**: Inflexible (fixed maximum number of connections), fixed struct size.

**Our Rust Approach (Heap/Box)**:
- We use `Box::new()`, which uses the system allocator (`malloc`/Heap).
- **Reason**: Rust's ownership model is tied to its allocation strategy. To have a `TcpConnectionState` that owns its sub-components (`FlowControl`, `Demux`), a dynamic heap allocation is the most idiomatic and safe way in Rust. Fitting a dynamic Rust struct into a rigid C memory pool would require `unsafe` pointer casting that defeats the purpose of using Rust.
- **Why This Breaks lwIP Philosophy**: lwIP is designed around static, deterministic memory pools (`memp`), which guarantee fixed allocation size, zero fragmentation, and bounded, predictable memory usage—key for embedded and resource-constrained environments. By switching to Rust's `Box::new()` (heap allocation), we violate these expectations: memory usage becomes non-deterministic, fragmentation is possible, and allocations may fail at runtime. While Rust's approach yields strong safety and clean modular boundaries, it fundamentally diverges from lwIP's goals of maximal predictability and minimal runtime overhead.

## 8. Why we thought it was a good idea (Motivation)

Despite the challenges, the initial motivation was strong and rooted in the superior architectural capabilities of Rust compared to C:

1.  **Memory Safety**: Network stacks are notorious for buffer overflows and use-after-free vulnerabilities. Rust's borrow checker guarantees memory safety at compile time (within the Rust boundary), eliminating entire classes of security bugs that plague C implementations.

2.  **Real Modularization vs. Monolithic C**:
    - **The C Reality**: The existing lwIP TCP implementation is effectively a "monolith." `tcp_in.c` contains thousands of lines of mixed logic where state, protocol handling, and buffer management are deeply intertwined.
    - **The Rust Advantage**: Rust's type system and ownership model enforce **true** modularity. We could define distinct structs for `FlowControl`, `CongestionControl`, and `Reliability`, enforcing clear boundaries via public/private interfaces. This prevents the "spaghetti code" effect common in C, where any part of the code might mutate any part of the state. The compiler forces you to respect these boundaries, making the complex TCP state machine much easier to reason about and maintain.

## 9. Honest Reflection: Is it worth it?

To be honest: **replacing a core C component with Rust in a tightly coupled legacy codebase like lwIP is likely not a good idea for production.**

The engineering effort required was massive ("very hard engineering"):
- **Intrusiveness**: We had to touch hundreds of lines of C code (`tcp.h` macros, `api_msg.c` refactors) just. This creates maintenance burden for the upstream C project.
- **Complexity**: The "Opaque Pointer" dance adds significant complexity to debugging. You can no longer just inspect a `pcb` in GDB; you have to know to cast it to the Rust type.
- **Loss of Idioms**: We fought against the grain of lwIP (static memory pools vs. Rust's heap). This friction suggests that a rewrite of the *entire* stack in Rust would be cleaner than this hybrid "Frankenstein" approach.
- **Fragility**: The FFI boundary is unsafe. One wrong cast or one missed pointer update on the C side can crash the Rust side instantly, negating Rust's safety promises at the boundary.

**Conclusion**: While this proves it *can* be done, the high cost of integration and the resulting architectural complexity suggest that a clean-slate Rust implementation (or keeping it all C) is preferable to this hybrid integration especially for shorter time frames.

## API Orchestration Layer (`tcp_api.rs`)

The API layer coordinates component methods without directly modifying component state:

```rust
pub fn tcp_connect(
    state: &mut TcpConnectionState,
    remote_ip: ip_addr_t,
    remote_port: u16,
) -> Result<(), &'static str> {
    // Validate precondition
    if state.conn_mgmt.state != TcpState::Closed {
        return Err("Can only connect from CLOSED state");
    }

    // Each component initializes its own state
    state.rod.on_connect()?;           // Generate ISS
    state.flow_ctrl.on_connect()?;     // Init rcv_wnd
    state.cong_ctrl.on_connect(&state.conn_mgmt)?;  // Init cwnd
    state.conn_mgmt.on_connect(remote_ip, remote_port)?;  // → SYN_SENT

    Ok(())
}
```

### Input Processing (`tcp_input`)

```rust
pub fn tcp_input(state: &mut TcpConnectionState, seg: &TcpSegment, ...)
    -> Result<InputAction, &'static str>
{
    // 1. RST handling (any state)
    if seg.flags.rst {
        match state.rod.validate_rst(seg, state.flow_ctrl.rcv_wnd) {
            RstValidation::Valid => { state.conn_mgmt.on_rst()?; return Ok(InputAction::Abort); }
            RstValidation::Challenge => return Ok(InputAction::SendChallengeAck),
            RstValidation::Invalid => return Ok(InputAction::Drop),
        }
    }

    // 2. State-specific dispatch
    match state.conn_mgmt.state {
        TcpState::Listen => { /* SYN handling */ }
        TcpState::SynSent => { /* SYN+ACK handling */ }
        TcpState::Established => { /* Data + FIN handling */ }
        // ... all 11 states covered
    }
}
```

## Comparison with Original lwIP

### ✅ Identical Behavior

| Feature | lwIP | Rust | Notes |
|---------|------|------|-------|
| State machine | 11 states in `tcp_pcb->state` | `conn_mgmt.state` | Same transitions |
| Sequence fields | `snd_nxt, rcv_nxt, iss, irs` | Same in `rod` | Same semantics |
| Window fields | `snd_wnd, rcv_wnd` | Same in `flow_ctrl` | Same semantics |
| Congestion fields | `cwnd, ssthresh` | Same in `cong_ctrl` | Same semantics |
| IW calculation | `LWIP_TCP_CALC_INITIAL_CWND` | RFC 5681 formula | Identical |
| RST validation | RFC 5961 | `validate_rst()` | Identical |
| Seq# validation | RFC 793 | `validate_sequence_number()` | Identical |

### ⚠️ Simplified (Known Deviations)

| Feature | lwIP | Rust | Reason |
|---------|------|------|--------|
| ISS generation | Time-based (RFC 6528) | Simple counter | TODO noted |
| Port 0 | Allocates ephemeral | Returns error | Not implemented |
| rcv_wnd default | Based on config | Hardcoded 4096 | Simplified |
| PCB allocation | Pool (memp) | Box heap | Rust idiom |

### ❌ Not Yet Implemented

| Feature | Description |
|---------|-------------|
| Data path | `tcp_write`, `tcp_output` - stubs only |
| Segment TX/RX | No actual packet construction/parsing |
| Retransmission | Timer and logic not implemented |
| Timers | keepalive, 2MSL, retransmit all stubs |
| PCB lists | `tcp_active_pcbs` etc. not managed |
| Out-of-order | `ooseq` queue not implemented |
| Options | Window scaling, SACK not implemented |

## Test Coverage

### Test Files
| File | Tests | Coverage |
|------|-------|----------|
| `handshake_tests.rs` | 5 | 3-way handshake, RST, initialization |
| `control_path_tests.rs` | ~40 | State machine, API, validation |
| `lib.rs` (ffi_tests) | 10 | FFI functions, null handling |
| `tcp_proto.rs` | 3 | Header parsing |

**Total: 58 tests, all passing**

### Tested Scenarios
- Active open (client): CLOSED → SYN_SENT → ESTABLISHED
- Passive open (server): CLOSED → LISTEN → SYN_RCVD → ESTABLISHED
- Active close: ESTABLISHED → FIN_WAIT_1 → FIN_WAIT_2 → TIME_WAIT
- Passive close: ESTABLISHED → CLOSE_WAIT → LAST_ACK → CLOSED
- Simultaneous close: ESTABLISHED → FIN_WAIT_1 → CLOSING → TIME_WAIT
- RST generation and validation
- Sequence number validation (RFC 793)
- ACK validation (duplicate, valid, old, future)

## Key Design Decisions

### 1. Component Boundaries
**Challenge:** Where to draw lines between components?

**Resolution:**
- `conn_mgmt`: State machine + connection tuple + options
- `rod`: Anything involving sequence numbers
- `flow_ctrl`: Window management
- `cong_ctrl`: Congestion window only (minimal)

### 2. Cross-Component Data Access
**Challenge:** `cong_ctrl.on_connect()` needs MSS from `conn_mgmt`

**Resolution:** Pass immutable reference:
```rust
state.cong_ctrl.on_connect(&state.conn_mgmt)?;
```

### 3. Method Naming Convention
**Pattern:** `on_<event>_in_<state>`
```rust
on_syn_in_listen()       // Received SYN while in LISTEN
on_synack_in_synsent()   // Received SYN+ACK while in SYN_SENT
on_fin_in_established()  // Received FIN while in ESTABLISHED
```

### 4. Return Types for Validation
**Rich enums instead of bool:**
```rust
pub enum RstValidation { Valid, Challenge, Invalid }
pub enum AckValidation { Duplicate, Valid, TooOld, TooNew }
pub enum InputAction { SendSynAck, SendAck, SendRst, Accept, Abort, Drop, ... }
```

## Future Work

1. **Data Path Implementation**
   - Implement `tcp_write` buffering
   - Implement `tcp_output` segment construction
   - Connect `tcp_input_rust` to actual parsing

2. **Timers**
   - Retransmission timer
   - Keepalive timer
   - 2MSL timeout for TIME_WAIT

3. **Congestion Control**
   - Slow start algorithm
   - Congestion avoidance
   - Fast retransmit/recovery

4. **Options**
   - Window scaling negotiation
   - SACK support
   - Timestamps

5. **PCB Management**
   - Maintain `tcp_active_pcbs` list
   - Implement proper demux in `tcp_input_rust`

## Summary

| Aspect | Status |
|--------|--------|
| **Architecture** | ✅ Complete - 4 components + API layer |
| **State Machine** | ✅ Complete - all 11 states, all transitions |
| **FFI Bridge** | ✅ Complete - 35+ functions exported |
| **Control Path** | ✅ Complete - handshake, close, RST |
| **Validation** | ✅ Complete - RFC 793/5961 compliant |
| **Tests** | ✅ Complete - 58 tests passing |
| **Data Path** | ❌ Stubs only |
| **Timers** | ❌ Stubs only |
| **Segment TX/RX** | ❌ Not implemented |

# Additional Docs:
### lwIP TCP Stack Architecture and Control Path**: [lwip_control_path.md](./lwip_control_path.md)
[Notion](https://www.notion.so/paulburkhardt/lwip-TCP-Stack-Architecture-and-Control-Path-Documentation-2a2168e7cc6b8097b1d4ff089a1e1dcd?source=copy_link)



### Demikernel TCP Stack Architecture and Control Path Documentation: [demikernel_control_path.md](./demikernel_control_path.md)
[Notion](https://www.notion.so/paulburkhardt/Demikernel-TCP-Stack-Architecture-and-Control-Path-Documentation-2c2168e7cc6b80f9a04df0f1cc35b26d?source=copy_link)
