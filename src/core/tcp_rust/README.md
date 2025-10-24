# Rust TCP Implementation for lwIP

This directory contains a Rust implementation of the TCP protocol layer that integrates with lwIP via FFI (Foreign Function Interface).

## Architecture

The TCP layer is implemented in Rust while the rest of lwIP remains in C:

```
┌─────────────────────────────┐
│   C Application Layer       │
│   (uses tcp_write(), etc.)  │
└──────────────┬──────────────┘
               │ C API (unchanged)
               ↓
┌─────────────────────────────┐
│    C Wrapper Layer          │
│    tcp_rust_wrapper.c       │
└──────────────┬──────────────┘
               │ FFI Boundary
               ↓
┌─────────────────────────────┐
│    Rust TCP Layer           │
│    • tcp_input_rust()       │
│    • tcp_output_rust()      │
│    • tcp_new_rust()         │
│    • tcp_bind_rust()        │
│    • tcp_connect_rust()     │
│    • etc.                   │
└──────────────┬──────────────┘
               │ FFI calls back to C
               ↓
┌─────────────────────────────┐
│   C IP Layer                │
│   (ip4_output, pbuf, etc.)  │
└─────────────────────────────┘
```

## Files

- **`Cargo.toml`**: Rust project configuration with size optimization settings
- **`build.rs`**: Build script that uses bindgen to generate Rust bindings from C headers
- **`wrapper.h`**: C headers to parse for FFI bindings
- **`src/lib.rs`**: Main Rust library with exported functions
- **`src/ffi.rs`**: FFI type definitions and bindings to C functions

## FFI Explained

### What is FFI?

**FFI (Foreign Function Interface)** allows Rust and C code to call each other. Key concepts:

1. **`extern "C"`**: Tells Rust to use C calling conventions
2. **`#[no_mangle]`**: Prevents Rust from renaming functions
3. **`#[repr(C)]`**: Makes Rust structs have C memory layout
4. **`#[no_std]`**: Don't use Rust standard library (reduces binary size)

### How It Works

#### C → Rust Call Flow

1. C code calls `tcp_input(pbuf, netif)` in `tcp_rust_wrapper.c`
2. Wrapper calls `tcp_input_rust(pbuf, netif)`
3. **FFI boundary crossed** - control passes to Rust
4. Rust function processes the packet
5. Rust may call back into C (e.g., `pbuf_free()`)
6. **FFI boundary crossed again**
7. Control returns to C

#### Rust → C Call Flow

When Rust needs C functionality:

```rust
unsafe {
    // Call C function to allocate packet buffer
    let p = ffi::pbuf_alloc(PBUF_TRANSPORT, len, PBUF_RAM);

    // Call C function to send to IP layer
    ffi::ip_output_if(p, &src, &dest, ttl, tos, proto, netif);
}
```

### Bindgen

**bindgen** automatically generates Rust FFI bindings from C headers:

- Input: C header files (via `wrapper.h`)
- Output: Rust code in `target/.../bindings.rs`
- Provides: Type definitions, function declarations, constants

Example: C's `struct pbuf` becomes Rust's `ffi::pbuf`

## Building

The Rust library is automatically built by CMake:

```bash
cd /workspaces/mlwip
mkdir build && cd build
cmake ..
make lwipcore
```

CMake:
1. Runs `cargo build --release` to compile Rust code
2. Generates `target/release/liblwip_tcp_rust.a`
3. Links the static library with C code

## Memory Safety Benefits

Rust provides:

- **No buffer overflows**: Bounds checking on array access
- **No use-after-free**: Ownership system prevents dangling pointers
- **No data races**: Rust's type system enforces safe concurrency
- **No null pointer dereferences**: `Option<T>` instead of null

However, FFI is inherently `unsafe` because:
- C pointers must be trusted
- C memory management must be respected
- Type compatibility must be manually ensured

The `unsafe` blocks in the Rust code mark where we trust the C layer.

## Size Optimization

Configuration in `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"           # Optimize for size
lto = true                # Link-time optimization
codegen-units = 1         # Better optimization
panic = "abort"           # No unwinding overhead
strip = "symbols"         # Strip debug symbols
```

Current overhead: ~50-60KB (compiler builtins + TCP code)

## Current Status

✅ **Complete:**
- FFI infrastructure
- Build system integration
- C wrapper layer
- Rust function stubs
- CMake integration
- Documentation

🔜 **TODO:**
- Implement actual TCP state machine
- Implement packet processing logic
- Implement congestion control
- Implement flow control
- Add comprehensive testing
- Port existing TCP unit tests

## Testing

To run Rust tests:

```bash
cd src/core/tcp_rust
cargo test
```

To run lwIP integration tests:

```bash
cd /workspaces/mlwip
# Build test suite
cmake -B test_build -S test/unit
cmake --build test_build
# Run tests
cd test_build && ctest
```

## Implementation Guide

### Adding a New TCP Function

1. **Add to `src/lib.rs`**:
```rust
#[no_mangle]
pub unsafe extern "C" fn tcp_example_rust(arg: i32) -> i8 {
    // Implementation
    ffi::ErrT::Ok.to_c()
}
```

2. **Add to `tcp_rust_wrapper.c`**:
```c
extern err_t tcp_example_rust(int arg);

err_t tcp_example(int arg) {
    return tcp_example_rust(arg);
}
```

3. **Rebuild**:
```bash
cd /workspaces/mlwip/build
make clean && make lwipcore
```

### Calling C Functions from Rust

1. **Add to `wrapper.h`** (if not already there):
```c
#include "lwip/new_header.h"
```

2. **Rebuild bindings**:
```bash
cd src/core/tcp_rust
cargo clean
cargo build --release
```

3. **Use in Rust**:
```rust
unsafe {
    ffi::new_c_function(arg);
}
```

## Safety Considerations

### Always Check

- ✅ Null pointers before dereferencing
- ✅ Buffer lengths before copying
- ✅ PCB validity before accessing
- ✅ Return values from C functions

### Never

- ❌ Panic in FFI functions (use `panic = "abort"`)
- ❌ Hold Rust references across FFI boundary
- ❌ Assume C pointers are valid
- ❌ Mix C and Rust memory allocators

## Performance

The FFI overhead is minimal:
- Function calls: ~1-2 CPU cycles
- No data copying (pointers are passed)
- Rust compiles to native code
- LTO optimizes across the boundary

## References

- [Rust FFI Guide](https://doc.rust-lang.org/nomicon/ffi.html)
- [bindgen User Guide](https://rust-lang.github.io/rust-bindgen/)
- [no_std Embedded Book](https://docs.rust-embedded.org/book/)
- [lwIP Documentation](https://www.nongnu.org/lwip/)
