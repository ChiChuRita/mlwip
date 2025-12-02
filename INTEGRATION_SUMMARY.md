# Rust TCP Integration into lwIP - Technical Summary
---

## PCB Ownership Strategy (Opaque Pointer)

### The Problem

Existing C applications call `tcp_new()` and expect a `struct tcp_pcb*` pointer back. But we want Rust to own and manage the TCP state for safety and modularity. The issue is:

- C's `struct tcp_pcb` and Rust's `TcpConnectionState` are completely different types with different memory layouts
- If C owned the real `tcp_pcb`, it could access/corrupt fields directly, bypassing Rust
- We'd lose Rust's safety guarantees and the modular architecture

### Why the Opaque Pointer?

The solution is a **handle-based API**:
- C holds a the pointer but never dereferences it
- Rust holds the actual data and mediates all access
- C API remains unchanged—applications don't need modification

### How It Works

1. **Rust allocates state on heap**:
   ```rust
   pub unsafe extern "C" fn tcp_new_rust() -> *mut ffi::tcp_pcb {
       let state = Box::new(TcpConnectionState::new());
       Box::into_raw(state) as *mut ffi::tcp_pcb  // Cast to opaque pointer
   }
   ```

2. **Rust deallocates on close/abort**:
   ```rust
   pub unsafe extern "C" fn tcp_abort_rust(pcb: *mut ffi::tcp_pcb) {
       if !pcb.is_null() {
           let _ = Box::from_raw(pcb as *mut TcpConnectionState);  // Drop
       }
   }
   ```

3. **C accesses fields via macros** (in `tcp.h`):
   ```c
   #if LWIP_USE_RUST_TCP
   #define tcp_sndbuf(pcb)      tcp_get_sndbuf_rust(pcb)
   #define tcp_sndqueuelen(pcb) tcp_get_sndqueuelen_rust(pcb)
   #define tcp_state_get(pcb)   ((enum tcp_state)tcp_get_state_rust(pcb))
   #endif
   ```

4. **Rust implements getters/setters**:
   ```rust
   pub unsafe extern "C" fn tcp_get_state_rust(pcb: *const ffi::tcp_pcb) -> u8 {
       let state = &*(pcb as *const TcpConnectionState);
       state.conn_mgmt.state as u8
   }
   ```

This way, C never dereferences the PCB pointer directly—all access goes through Rust.

---

## Modular Rust Architecture

`TcpConnectionState` aggregates five components:

| Component | File | Responsibility |
|-----------|------|----------------|
| `ConnectionManagementState` | `connection_management.rs` | TCP state machine, IP/port binding |
| `ReliableOrderedDeliveryState` | `reliable_ordered_delivery.rs` | Sequence numbers, ISS, retransmission |
| `FlowControlState` | `flow_control.rs` | Send/receive windows, buffer sizes |
| `CongestionControlState` | `congestion_control.rs` | CWND, SSTHRESH, slow start |
| `DemuxState` | `demux.rs` | Connection lookup, binding |

Plus callback storage for C function pointers (`recv_fn`, `sent_fn`, `err_fn`, etc.).

---

## FFI Layer

### wrapper.c
- Implements standard lwIP API functions (`tcp_new`, `tcp_bind`, `tcp_connect`, etc.)
- Each function simply forwards to the corresponding `*_rust()` function
- Compiled only when `LWIP_USE_RUST_TCP=1`

### lib.rs
- Exports `#[no_mangle] pub unsafe extern "C"` functions
- Uses `bindgen` to generate Rust bindings from C headers
- Separate `#[cfg(test)]` mock FFI for isolated Rust testing

---

## Build System

CMake integration in `src/Filelists.cmake`:

1. **Toggle**: `option(LWIP_USE_RUST_TCP "Use Rust TCP" ON)`
2. **Build Rust**: Custom target runs `cargo build --release`
3. **Link**: `target_link_libraries(lwipcore ${RUST_TCP_LIB} ${LIBDL})`
4. **Define**: `target_compile_definitions(lwipcore PUBLIC LWIP_USE_RUST_TCP=1)`

On Linux, `libdl` is required because Rust std uses dynamic loading.

---

## Testing

### Rust Tests (65 tests)
- Test individual components in isolation
- Use mock FFI types (`#[cfg(test)]` module)
- Run with: `cargo test`

### C Integration Tests (26 tests)
- Test full C→wrapper→Rust→C roundtrip
- Verify PCB allocation, state transitions, getters/setters
- Run with: `./lwip_unittests` (built with `-DLWIP_USE_RUST_TCP=ON`)

---

## Key Files

```
src/core/tcp_rust/
├── wrapper.c              # C FFI wrapper
├── src/lib.rs             # Rust FFI exports
├── src/state.rs           # TcpConnectionState
├── src/components/*.rs    # 5 modular components
└── Cargo.toml

src/include/lwip/tcp.h     # Accessor macros (#if LWIP_USE_RUST_TCP)

test/unit/tcp/
├── test_tcp_rust.c        # C integration tests
└── test_tcp_rust.h
```

---

## Summary

The integration works by:
1. Rust owns all TCP state via `TcpConnectionState`
2. C receives an opaque pointer (cast from Rust heap allocation)
3. All field access from C goes through macro-redirected FFI calls
4. Build system conditionally compiles wrapper.c and links Rust static library
5. Both Rust unit tests and C integration tests verify correctness
