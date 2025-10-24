//! Rust implementation of lwIP TCP layer
//!
//! This crate provides a memory-safe Rust implementation of TCP that integrates
//! with the existing lwIP C codebase via FFI (Foreign Function Interface).

#![no_std]  // Don't use Rust standard library - reduces binary size
#![allow(dead_code)]
#![allow(unused_variables)]

mod ffi;
// mod tcp;
// mod tcp_in;
// mod tcp_out;

// Re-export for testing
pub use ffi::*;

/// TCP input function called from C IP layer
///
/// # Safety
/// This function is unsafe because it:
/// - Receives raw pointers from C
/// - Dereferences opaque C structures
/// - Calls back into C code
#[no_mangle]
pub unsafe extern "C" fn tcp_input_rust(p: *mut ffi::pbuf, inp: *mut ffi::netif) {
    // Null pointer checks
    if p.is_null() {
        return;
    }

    // TODO: Implement TCP input processing
    // For now, just free the pbuf to avoid memory leak
    ffi::pbuf_free(p);
}

/// Create a new TCP PCB (Protocol Control Block)
///
/// # Safety
/// Returns a raw pointer that must be freed by the caller
#[no_mangle]
pub unsafe extern "C" fn tcp_new_rust() -> *mut core::ffi::c_void {
    // TODO: Implement PCB allocation
    // For now, return null
    core::ptr::null_mut()
}

/// TCP timer function called periodically from C
///
/// # Safety
/// Must be called from the same thread as other TCP functions
#[no_mangle]
pub unsafe extern "C" fn tcp_tmr_rust() {
    // TODO: Implement TCP timer processing
}

/// Bind a TCP PCB to a local address and port
///
/// # Safety
/// pcb and ipaddr must be valid pointers
#[no_mangle]
pub unsafe extern "C" fn tcp_bind_rust(
    _pcb: *mut core::ffi::c_void,
    _ipaddr: *const ffi::ip_addr_t,
    _port: u16,
) -> i8 {
    // TODO: Implement bind logic
    ffi::ErrT::Ok.to_c()
}

/// Connect to a remote TCP endpoint
///
/// # Safety
/// All pointers must be valid
#[no_mangle]
pub unsafe extern "C" fn tcp_connect_rust(
    _pcb: *mut core::ffi::c_void,
    _ipaddr: *const ffi::ip_addr_t,
    _port: u16,
    _connected: Option<extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, i8) -> i8>,
) -> i8 {
    // TODO: Implement connect logic
    ffi::ErrT::Ok.to_c()
}

/// Write data to a TCP connection
///
/// # Safety
/// All pointers must be valid and dataptr must point to at least len bytes
#[no_mangle]
pub unsafe extern "C" fn tcp_write_rust(
    _pcb: *mut core::ffi::c_void,
    _dataptr: *const core::ffi::c_void,
    _len: u16,
    _apiflags: u8,
) -> i8 {
    // TODO: Implement write logic
    ffi::ErrT::Ok.to_c()
}

/// Trigger TCP output (send buffered data)
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_output_rust(_pcb: *mut core::ffi::c_void) -> i8 {
    // TODO: Implement output logic
    ffi::ErrT::Ok.to_c()
}

/// Close a TCP connection
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_close_rust(_pcb: *mut core::ffi::c_void) -> i8 {
    // TODO: Implement close logic
    ffi::ErrT::Ok.to_c()
}

/// Abort a TCP connection (immediate termination)
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_abort_rust(_pcb: *mut core::ffi::c_void) {
    // TODO: Implement abort logic
}

/// Indicate that application has processed received data
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_recved_rust(_pcb: *mut core::ffi::c_void, _len: u16) {
    // TODO: Implement recved logic (update receive window)
}

// Panic handler for no_std
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // In FFI code, we must never panic
    // This should never be reached if we handle all errors properly
    loop {}
}
