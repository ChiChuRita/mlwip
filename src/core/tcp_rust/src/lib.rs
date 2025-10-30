//! Rust implementation of lwIP TCP layer
//!
//! This crate provides a memory-safe Rust implementation of TCP that integrates
//! with the existing lwIP C codebase via FFI (Foreign Function Interface).

// TEMPORARY: Enable std for development debugging
// TODO: Switch back to #![no_std] for production
// #![no_std]
#![allow(dead_code)]
#![allow(unused_variables)]

mod ffi;

// Re-export for testing
pub use ffi::*;

use std::ffi::c_void;
use std::ptr::null_mut;

/// Internal timer state - NOT exposed to C
static mut TCP_TIMER_ACTIVE: bool = false;
static mut TCP_TIMER_COUNTER: u8 = 0;

/// Global PCB lists - exposed for C code compatibility
/// These are NULL for stub implementation
#[no_mangle]
pub static mut tcp_active_pcbs: *mut c_void = null_mut();

#[no_mangle]
pub static mut tcp_tw_pcbs: *mut c_void = null_mut();

#[no_mangle]
pub static mut tcp_bound_pcbs: *mut c_void = null_mut();

/// Free out-of-sequence queue - stub for C compatibility
#[no_mangle]
pub unsafe extern "C" fn tcp_free_ooseq(_pcb: *mut c_void) {
    // Stub - no-op for now
}

/// Internal timer callback registered with sys_timeout()
/// This is called by lwIP's timer system
unsafe extern "C" fn tcp_internal_timer(_arg: *mut c_void) {
    println!("[RUST] tcp_internal_timer fired - calling tcp_tmr_rust");
    tcp_tmr_rust();

    ffi::sys_timeout(ffi::TCP_TMR_INTERVAL, Some(tcp_internal_timer), null_mut());
}

/// Internal fast timer function (not FFI)
fn tcp_fasttmr() {
    println!("[RUST] tcp_fasttmr called - fast timer tick (250ms)");
}

/// Internal slow timer function (not FFI)
fn tcp_slowtmr() {
    println!("[RUST] tcp_slowtmr called - slow timer tick (500ms)");
}

/// TCP input function called from C IP layer
///
/// # Safety
/// This function is unsafe because it:
/// - Receives raw pointers from C
/// - Dereferences opaque C structures
/// - Calls back into C code
#[no_mangle]
pub unsafe extern "C" fn tcp_input_rust(p: *mut ffi::pbuf, inp: *mut ffi::netif) {
    println!("[RUST] tcp_input_rust called: pbuf={:p}, netif={:p}", p, inp);

    if p.is_null() {
        println!("[RUST] tcp_input_rust: null pbuf, returning");
        return;
    }

    ffi::pbuf_free(p);
    println!("[RUST] tcp_input_rust: pbuf freed");
}

/// Initialize TCP module - called from lwip_init()
///
/// # Safety
/// Must be called only once at startup
#[no_mangle]
pub unsafe extern "C" fn tcp_init_rust() {
    println!("[RUST] tcp_init_rust called - initializing TCP module");
    println!("[RUST] tcp_init_rust: registering timer with sys_timeout");

    ffi::sys_timeout(ffi::TCP_TMR_INTERVAL, Some(tcp_internal_timer), null_mut());
    TCP_TIMER_ACTIVE = true;

    println!("[RUST] tcp_init_rust: timer registered successfully");
}

/// Create a new TCP PCB (Protocol Control Block)
///
/// # Safety
/// Returns a raw pointer that must be freed by the caller
#[no_mangle]
pub unsafe extern "C" fn tcp_new_rust() -> *mut c_void {
    println!("[RUST] tcp_new_rust called");
    null_mut()
}

/// TCP timer function called periodically from C or internal timer
///
/// # Safety
/// Must be called from the same thread as other TCP functions
#[no_mangle]
pub unsafe extern "C" fn tcp_tmr_rust() {
    tcp_fasttmr();

    TCP_TIMER_COUNTER = TCP_TIMER_COUNTER.wrapping_add(1);
    if TCP_TIMER_COUNTER & 1 != 0 {
        tcp_slowtmr();
    }
}

/// Bind a TCP PCB to a local address and port
///
/// # Safety
/// pcb and ipaddr must be valid pointers
#[no_mangle]
pub unsafe extern "C" fn tcp_bind_rust(
    _pcb: *mut c_void,
    _ipaddr: *const ffi::ip_addr_t,
    _port: u16,
) -> i8 {
    println!("[RUST] tcp_bind_rust called: pcb={:p}, port={}", _pcb, _port);
    ffi::ErrT::Ok.to_c()
}

/// Connect to a remote TCP endpoint
///
/// # Safety
/// All pointers must be valid
#[no_mangle]
pub unsafe extern "C" fn tcp_connect_rust(
    _pcb: *mut c_void,
    _ipaddr: *const ffi::ip_addr_t,
    _port: u16,
    _connected: Option<extern "C" fn(*mut c_void, *mut c_void, i8) -> i8>,
) -> i8 {
    println!("[RUST] tcp_connect_rust called: pcb={:p}, port={}", _pcb, _port);
    ffi::ErrT::Ok.to_c()
}

/// Write data to a TCP connection
///
/// # Safety
/// All pointers must be valid and dataptr must point to at least len bytes
#[no_mangle]
pub unsafe extern "C" fn tcp_write_rust(
    _pcb: *mut c_void,
    _dataptr: *const c_void,
    _len: u16,
    _apiflags: u8,
) -> i8 {
    println!("[RUST] tcp_write_rust called: pcb={:p}, len={}, flags={}", _pcb, _len, _apiflags);
    ffi::ErrT::Ok.to_c()
}

/// Trigger TCP output (send buffered data)
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_output_rust(_pcb: *mut c_void) -> i8 {
    println!("[RUST] tcp_output_rust called: pcb={:p}", _pcb);
    ffi::ErrT::Ok.to_c()
}

/// Close a TCP connection
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_close_rust(_pcb: *mut c_void) -> i8 {
    println!("[RUST] tcp_close_rust called: pcb={:p}", _pcb);
    ffi::ErrT::Ok.to_c()
}

/// Abort a TCP connection (immediate termination)
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_abort_rust(_pcb: *mut c_void) {
    println!("[RUST] tcp_abort_rust called: pcb={:p}", _pcb);
}

/// Indicate that application has processed received data
///
/// # Safety
/// pcb must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn tcp_recved_rust(_pcb: *mut c_void, _len: u16) {
    println!("[RUST] tcp_recved_rust called: pcb={:p}, len={}", _pcb, _len);
}

/// Set the argument passed to callbacks
#[no_mangle]
pub unsafe extern "C" fn tcp_arg_rust(_pcb: *mut c_void, _arg: *mut c_void) {
    println!("[RUST] tcp_arg_rust called: pcb={:p}, arg={:p}", _pcb, _arg);
}

/// Set the receive callback
#[no_mangle]
pub unsafe extern "C" fn tcp_recv_rust(
    _pcb: *mut c_void,
    _recv: Option<extern "C" fn(*mut c_void, *mut c_void, *mut ffi::pbuf, i8) -> i8>,
) {
    println!("[RUST] tcp_recv_rust called: pcb={:p}", _pcb);
}

/// Set the sent callback
#[no_mangle]
pub unsafe extern "C" fn tcp_sent_rust(
    _pcb: *mut c_void,
    _sent: Option<extern "C" fn(*mut c_void, *mut c_void, u16) -> i8>,
) {
    println!("[RUST] tcp_sent_rust called: pcb={:p}", _pcb);
}

/// Set the poll callback
#[no_mangle]
pub unsafe extern "C" fn tcp_poll_rust(
    _pcb: *mut c_void,
    _poll: Option<extern "C" fn(*mut c_void, *mut c_void) -> i8>,
    _interval: u8,
) {
    println!("[RUST] tcp_poll_rust called: pcb={:p}, interval={}", _pcb, _interval);
}

/// Set the error callback
#[no_mangle]
pub unsafe extern "C" fn tcp_err_rust(
    _pcb: *mut c_void,
    _err: Option<extern "C" fn(*mut c_void, i8)>,
) {
    println!("[RUST] tcp_err_rust called: pcb={:p}", _pcb);
}

/// Set the accept callback
#[no_mangle]
pub unsafe extern "C" fn tcp_accept_rust(
    _pcb: *mut c_void,
    _accept: Option<extern "C" fn(*mut c_void, *mut c_void, i8) -> i8>,
) {
    println!("[RUST] tcp_accept_rust called: pcb={:p}", _pcb);
}

/// Shutdown a TCP connection
#[no_mangle]
pub unsafe extern "C" fn tcp_shutdown_rust(_pcb: *mut c_void, _shut_rx: i32, _shut_tx: i32) -> i8 {
    println!("[RUST] tcp_shutdown_rust called: pcb={:p}, rx={}, tx={}", _pcb, _shut_rx, _shut_tx);
    ffi::ErrT::Ok.to_c()
}

/// Bind to a specific network interface
#[no_mangle]
pub unsafe extern "C" fn tcp_bind_netif_rust(_pcb: *mut c_void, _netif: *const ffi::netif) {
    println!("[RUST] tcp_bind_netif_rust called: pcb={:p}, netif={:p}", _pcb, _netif);
}

/// Listen for incoming connections
#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_and_err_rust(
    _pcb: *mut c_void,
    _backlog: u8,
    _err: *mut i8,
) -> *mut c_void {
    println!("[RUST] tcp_listen_with_backlog_and_err_rust called: pcb={:p}, backlog={}", _pcb, _backlog);
    if !_err.is_null() {
        *_err = ffi::ErrT::Ok.to_c();
    }
    _pcb
}

/// Listen for incoming connections (simplified version)
#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_rust(
    _pcb: *mut c_void,
    _backlog: u8,
) -> *mut c_void {
    println!("[RUST] tcp_listen_with_backlog_rust called: pcb={:p}, backlog={}", _pcb, _backlog);
    _pcb
}

/// Create a new TCP PCB with specific IP type
#[no_mangle]
pub unsafe extern "C" fn tcp_new_ip_type_rust(_type: u8) -> *mut c_void {
    println!("[RUST] tcp_new_ip_type_rust called: type={}", _type);
    null_mut()
}

/// Set TCP connection priority
#[no_mangle]
pub unsafe extern "C" fn tcp_setprio_rust(_pcb: *mut c_void, _prio: u8) {
    println!("[RUST] tcp_setprio_rust called: pcb={:p}, prio={}", _pcb, _prio);
}

/// Get TCP address info
#[no_mangle]
pub unsafe extern "C" fn tcp_tcp_get_tcp_addrinfo_rust(
    _pcb: *mut c_void,
    _local: i32,
    _addr: *mut ffi::ip_addr_t,
    _port: *mut u16,
) -> i8 {
    println!("[RUST] tcp_tcp_get_tcp_addrinfo_rust called: pcb={:p}, local={}", _pcb, _local);
    ffi::ErrT::Ok.to_c()
}

/// Handle network interface IP address changes
#[no_mangle]
pub unsafe extern "C" fn tcp_netif_ip_addr_changed_rust(
    _old_addr: *const ffi::ip_addr_t,
    _new_addr: *const ffi::ip_addr_t,
) {
    println!("[RUST] tcp_netif_ip_addr_changed_rust called: old_addr={:p}, new_addr={:p}", _old_addr, _new_addr);
}

/// TCP backlog delayed (if TCP_LISTEN_BACKLOG enabled)
#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_delayed_rust(_pcb: *mut c_void) {
    println!("[RUST] tcp_backlog_delayed_rust called: pcb={:p}", _pcb);
}

/// TCP backlog accepted (if TCP_LISTEN_BACKLOG enabled)
#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_accepted_rust(_pcb: *mut c_void) {
    println!("[RUST] tcp_backlog_accepted_rust called: pcb={:p}", _pcb);
}

/// Allocate extension argument ID (if LWIP_TCP_PCB_NUM_EXT_ARGS enabled)
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_alloc_id_rust() -> u8 {
    println!("[RUST] tcp_ext_arg_alloc_id_rust called");
    0
}

/// Set extension argument callbacks (if LWIP_TCP_PCB_NUM_EXT_ARGS enabled)
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_callbacks_rust(
    _pcb: *mut c_void,
    _id: u8,
    _callbacks: *const c_void,
) {
    println!("[RUST] tcp_ext_arg_set_callbacks_rust called: pcb={:p}, id={}", _pcb, _id);
}

/// Set extension argument (if LWIP_TCP_PCB_NUM_EXT_ARGS enabled)
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_rust(
    _pcb: *mut c_void,
    _id: u8,
    _arg: *mut c_void,
) {
    println!("[RUST] tcp_ext_arg_set_rust called: pcb={:p}, id={}, arg={:p}", _pcb, _id, _arg);
}

/// Get extension argument (if LWIP_TCP_PCB_NUM_EXT_ARGS enabled)
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_get_rust(
    _pcb: *const c_void,
    _id: u8,
) -> *mut c_void {
    println!("[RUST] tcp_ext_arg_get_rust called: pcb={:p}, id={}", _pcb, _id);
    null_mut()
}

// TEMPORARY: Panic handler disabled while using std
// TODO: Re-enable when switching back to no_std
// #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
