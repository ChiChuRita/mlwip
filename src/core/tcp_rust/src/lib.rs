// TCP Rust Implementation Library
// Main library entry point for lwip_tcp_rust

#![allow(dead_code)]
#![allow(unused_variables)]

use std::ptr;
use std::ffi::c_void;

pub mod tcp_proto;

#[cfg(not(test))]
#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
pub mod ffi {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[cfg(test)]
#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
pub mod ffi {
    use core::ffi::c_void;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct ip_addr_t {
        pub addr: u32,
    }

    pub type netif = u8;

    pub use crate::tcp_proto::TcpHdr as tcp_hdr;

    #[repr(C)]
    pub struct pbuf {
        pub next: *mut pbuf,
        pub payload: *mut c_void,
        pub tot_len: u16,
        pub len: u16,
        pub type_: u8,
        pub flags: u8,
        pub ref_: u8,
    }

    #[repr(C)]
    pub struct tcp_pcb {
        pub state: u8,
        pub prio: u8,
        pub callback_arg: *mut c_void,
    }

    pub type tcp_recv_fn = Option<unsafe extern "C" fn(*mut c_void, *mut tcp_pcb, *mut pbuf, i8) -> i8>;
    pub type tcp_sent_fn = Option<unsafe extern "C" fn(*mut c_void, *mut tcp_pcb, u16) -> i8>;
    pub type tcp_err_fn = Option<unsafe extern "C" fn(*mut c_void, i8)>;
    pub type tcp_connected_fn = Option<unsafe extern "C" fn(*mut c_void, *mut tcp_pcb, i8) -> i8>;
    pub type tcp_poll_fn = Option<unsafe extern "C" fn(*mut c_void, *mut tcp_pcb) -> i8>;
    pub type tcp_accept_fn = Option<unsafe extern "C" fn(*mut c_void, *mut tcp_pcb, i8) -> i8>;

    pub use crate::tcp_proto::{TCP_FIN, TCP_SYN, TCP_RST, TCP_PSH, TCP_ACK, TCP_URG};

    pub const pbuf_layer_PBUF_TRANSPORT: u32 = 0;
    pub const pbuf_type_PBUF_RAM: u32 = 0;

    pub unsafe fn pbuf_alloc(_layer: u32, _length: u16, _type: u32) -> *mut pbuf {
        panic!("pbuf_alloc should not be called in tests");
    }

    pub unsafe fn pbuf_free(_p: *mut pbuf) {
    }
}

pub mod components;
pub mod state;
pub mod tcp_types;
pub mod tcp_api;
pub mod tcp_in;
pub mod tcp_out;

pub use state::{TcpState, TcpConnectionState};
pub use tcp_types::{
    TcpFlags, TcpSegment,
    RstValidation, AckValidation, InputAction
};
pub use tcp_api::{
    tcp_bind, tcp_listen, tcp_connect, tcp_abort, initiate_close
};
pub use tcp_api::tcp_input;

const ERR_OK: i8 = 0;
const ERR_MEM: i8 = -1;
const ERR_ARG: i8 = -16;

#[no_mangle]
pub static mut tcp_ticks: u32 = 0;

#[no_mangle]
pub static mut tcp_active_pcbs: *mut c_void = ptr::null_mut();

#[no_mangle]
pub static mut tcp_tw_pcbs: *mut c_void = ptr::null_mut();

#[no_mangle]
pub static mut tcp_bound_pcbs: *mut c_void = ptr::null_mut();

#[no_mangle]
pub static mut tcp_listen_pcbs: *mut c_void = ptr::null_mut();

#[no_mangle]
pub unsafe extern "C" fn tcp_init_rust() {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_input_rust(
    p: *mut ffi::pbuf,
    inp: *mut ffi::netif,
) {
    if p.is_null() {
        return;
    }
    ffi::pbuf_free(p);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_new_rust() -> *mut ffi::tcp_pcb {
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn tcp_new_ip_type_rust(ip_type: u8) -> *mut ffi::tcp_pcb {
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn tcp_tmr_rust() {
    tcp_ticks = tcp_ticks.wrapping_add(1);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_bind_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
) -> i8 {
    if pcb.is_null() {
        return ERR_ARG;
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_connect_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
    connected: ffi::tcp_connected_fn,
) -> i8 {
    if pcb.is_null() {
        return ERR_ARG;
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_write_rust(
    pcb: *mut ffi::tcp_pcb,
    dataptr: *const c_void,
    len: u16,
    apiflags: u8,
) -> i8 {
    if pcb.is_null() || dataptr.is_null() {
        return ERR_ARG;
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_output_rust(pcb: *mut ffi::tcp_pcb) -> i8 {
    if pcb.is_null() {
        return ERR_ARG;
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_close_rust(pcb: *mut ffi::tcp_pcb) -> i8 {
    if pcb.is_null() {
        return ERR_ARG;
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_abort_rust(pcb: *mut ffi::tcp_pcb) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_recved_rust(pcb: *mut ffi::tcp_pcb, len: u16) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_arg_rust(pcb: *mut ffi::tcp_pcb, arg: *mut c_void) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_recv_rust(pcb: *mut ffi::tcp_pcb, recv: ffi::tcp_recv_fn) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_sent_rust(pcb: *mut ffi::tcp_pcb, sent: ffi::tcp_sent_fn) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_poll_rust(
    pcb: *mut ffi::tcp_pcb,
    poll: ffi::tcp_poll_fn,
    interval: u8,
) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_err_rust(pcb: *mut ffi::tcp_pcb, err: ffi::tcp_err_fn) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_accept_rust(pcb: *mut ffi::tcp_pcb, accept: ffi::tcp_accept_fn) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_shutdown_rust(pcb: *mut ffi::tcp_pcb, shut_rx: i32, shut_tx: i32) -> i8 {
    if pcb.is_null() {
        return ERR_ARG;
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_bind_netif_rust(pcb: *mut ffi::tcp_pcb, netif: *const ffi::netif) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
) -> *mut ffi::tcp_pcb {
    pcb
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_and_err_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
    err: *mut i8,
) -> *mut ffi::tcp_pcb {
    if !err.is_null() {
        *err = ERR_OK;
    }
    pcb
}

#[no_mangle]
pub unsafe extern "C" fn tcp_setprio_rust(pcb: *mut ffi::tcp_pcb, prio: u8) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_tcp_get_tcp_addrinfo_rust(
    pcb: *mut ffi::tcp_pcb,
    local: i32,
    addr: *mut ffi::ip_addr_t,
    port: *mut u16,
) -> i8 {
    if pcb.is_null() {
        return ERR_ARG;
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_netif_ip_addr_changed_rust(
    old_addr: *const ffi::ip_addr_t,
    new_addr: *const ffi::ip_addr_t,
) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_delayed_rust(pcb: *mut ffi::tcp_pcb) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_accepted_rust(pcb: *mut ffi::tcp_pcb) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_alloc_id_rust() -> u8 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_callbacks_rust(
    pcb: *mut ffi::tcp_pcb,
    id: u8,
    callbacks: *const c_void,
) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_rust(
    pcb: *mut ffi::tcp_pcb,
    id: u8,
    arg: *mut c_void,
) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_get_rust(
    pcb: *const ffi::tcp_pcb,
    id: u8,
) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_state_rust(pcb: *const ffi::tcp_pcb) -> u8 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndbuf_rust(pcb: *const ffi::tcp_pcb) -> u16 {
    0xffff
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndqueuelen_rust(pcb: *const ffi::tcp_pcb) -> u16 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_flags_rust(pcb: *mut ffi::tcp_pcb, set_flags: u16) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_clear_flags_rust(pcb: *mut ffi::tcp_pcb, clr_flags: u16) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_is_flag_set_rust(pcb: *const ffi::tcp_pcb, flag: u16) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn tcp_rst(
    pcb: *mut ffi::tcp_pcb,
    seqno: u32,
    ackno: u32,
    local_ip: *const ffi::ip_addr_t,
    remote_ip: *const ffi::ip_addr_t,
    local_port: u16,
    remote_port: u16,
) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_next_iss(pcb: *mut ffi::tcp_pcb) -> u32 {
    static mut ISS: u32 = 6510;
    ISS = ISS.wrapping_add(1);
    ISS
}

#[no_mangle]
pub unsafe extern "C" fn tcp_fasttmr() {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_slowtmr() {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_free_ooseq(pcb: *mut ffi::tcp_pcb) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_idle_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    7200000
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_idle_rust(pcb: *mut ffi::tcp_pcb, idle: u32) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_intvl_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    75000
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_intvl_rust(pcb: *mut ffi::tcp_pcb, intvl: u32) {
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_cnt_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    9
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_cnt_rust(pcb: *mut ffi::tcp_pcb, cnt: u32) {
}
