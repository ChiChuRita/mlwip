#![allow(dead_code)]
#![allow(unused_variables)]

mod ffi;
mod tcp;

pub use ffi::*;

use std::ffi::c_void;
use std::ptr::null_mut;

#[no_mangle]
pub static mut tcp_active_pcbs: *mut c_void = null_mut();

#[no_mangle]
pub static mut tcp_tw_pcbs: *mut c_void = null_mut();

#[no_mangle]
pub static mut tcp_bound_pcbs: *mut c_void = null_mut();

#[no_mangle]
pub unsafe extern "C" fn tcp_free_ooseq(_pcb: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn tcp_input_rust(p: *mut ffi::pbuf, inp: *mut ffi::netif) {
    tcp::tcp_input(p, inp);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_init_rust() {
    tcp::tcp_init();
}

#[no_mangle]
pub unsafe extern "C" fn tcp_new_rust() -> *mut ffi::tcp_pcb {
    tcp::tcp_new()
}

#[no_mangle]
pub unsafe extern "C" fn tcp_tmr_rust() {
    tcp::tcp_tmr();
}

#[no_mangle]
pub unsafe extern "C" fn tcp_bind_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
) -> i8 {
    tcp::tcp_bind(pcb, ipaddr, port)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_connect_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
    connected: ffi::tcp_connected_fn,
) -> i8 {
    tcp::tcp_connect(pcb, ipaddr, port, connected)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_write_rust(
    pcb: *mut ffi::tcp_pcb,
    dataptr: *const c_void,
    len: u16,
    apiflags: u8,
) -> i8 {
    tcp::tcp_write(pcb, dataptr, len, apiflags)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_output_rust(pcb: *mut ffi::tcp_pcb) -> i8 {
    tcp::tcp_output(pcb)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_close_rust(pcb: *mut ffi::tcp_pcb) -> i8 {
    tcp::tcp_close(pcb)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_abort_rust(pcb: *mut ffi::tcp_pcb) {
    tcp::tcp_abort(pcb);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_recved_rust(pcb: *mut ffi::tcp_pcb, len: u16) {
    tcp::tcp_recved(pcb, len);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_arg_rust(pcb: *mut ffi::tcp_pcb, arg: *mut c_void) {
    tcp::tcp_arg(pcb, arg);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_recv_rust(
    pcb: *mut ffi::tcp_pcb,
    recv: ffi::tcp_recv_fn,
) {
    tcp::tcp_recv(pcb, recv);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_sent_rust(
    pcb: *mut ffi::tcp_pcb,
    sent: ffi::tcp_sent_fn,
) {
    tcp::tcp_sent(pcb, sent);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_poll_rust(
    pcb: *mut ffi::tcp_pcb,
    poll: ffi::tcp_poll_fn,
    interval: u8,
) {
    tcp::tcp_poll(pcb, poll, interval);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_err_rust(pcb: *mut ffi::tcp_pcb, err: ffi::tcp_err_fn) {
    tcp::tcp_err(pcb, err);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_accept_rust(
    pcb: *mut ffi::tcp_pcb,
    accept: ffi::tcp_accept_fn,
) {
    tcp::tcp_accept(pcb, accept);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_shutdown_rust(
    pcb: *mut ffi::tcp_pcb,
    shut_rx: i32,
    shut_tx: i32,
) -> i8 {
    tcp::tcp_shutdown(pcb, shut_rx, shut_tx)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_bind_netif_rust(
    pcb: *mut ffi::tcp_pcb,
    netif: *const ffi::netif,
) {
    tcp::tcp_bind_netif(pcb, netif);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_and_err_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
    err: *mut i8,
) -> *mut ffi::tcp_pcb {
    tcp::tcp_listen_with_backlog_and_err(pcb, backlog, err)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
) -> *mut ffi::tcp_pcb {
    tcp::tcp_listen_with_backlog(pcb, backlog)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_new_ip_type_rust(ip_type: u8) -> *mut ffi::tcp_pcb {
    tcp::tcp_new_ip_type(ip_type)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_setprio_rust(pcb: *mut ffi::tcp_pcb, prio: u8) {
    tcp::tcp_setprio(pcb, prio);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_tcp_get_tcp_addrinfo_rust(
    pcb: *mut ffi::tcp_pcb,
    local: i32,
    addr: *mut ffi::ip_addr_t,
    port: *mut u16,
) -> i8 {
    tcp::tcp_tcp_get_tcp_addrinfo(pcb, local, addr, port)
}

#[no_mangle]
pub unsafe extern "C" fn tcp_netif_ip_addr_changed_rust(
    old_addr: *const ffi::ip_addr_t,
    new_addr: *const ffi::ip_addr_t,
) {
    tcp::tcp_netif_ip_addr_changed(old_addr, new_addr);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_delayed_rust(pcb: *mut ffi::tcp_pcb) {
    tcp::tcp_backlog_delayed(pcb);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_accepted_rust(pcb: *mut ffi::tcp_pcb) {
    tcp::tcp_backlog_accepted(pcb);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_alloc_id_rust() -> u8 {
    tcp::tcp_ext_arg_alloc_id()
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_callbacks_rust(
    pcb: *mut ffi::tcp_pcb,
    id: u8,
    callbacks: *const c_void,
) {
    tcp::tcp_ext_arg_set_callbacks(pcb, id, callbacks);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_rust(
    pcb: *mut ffi::tcp_pcb,
    id: u8,
    arg: *mut c_void,
) {
    tcp::tcp_ext_arg_set(pcb, id, arg);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_get_rust(
    pcb: *const ffi::tcp_pcb,
    id: u8,
) -> *mut c_void {
    tcp::tcp_ext_arg_get(pcb, id)
}
