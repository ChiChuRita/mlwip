#![allow(dead_code)]
#![allow(unused_variables)]

mod ffi;
mod tcp;

// use ffi::*; // Implicitly available via mod ffi, but we use ffi::Type explicitly usually
use tcp::{TcpConnection, TcpState}; // Import the safe struct and enum

use std::ffi::c_void;
use std::ptr;

// C-compatible constants required for FFI interaction (e.g. flags)
const TCP_FIN: u8 = 0x01;
const TCP_SYN: u8 = 0x02;
const TCP_RST: u8 = 0x04;
const TCP_PSH: u8 = 0x08;
const TCP_ACK: u8 = 0x10;
const TCP_URG: u8 = 0x20;
const TCP_ECE: u8 = 0x40;
const TCP_CWR: u8 = 0x80;

#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut tcp_ticks: u32 = 0;

#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut tcp_active_pcbs: *mut c_void = ptr::null_mut();

#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut tcp_tw_pcbs: *mut c_void = ptr::null_mut();

#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut tcp_bound_pcbs: *mut c_void = ptr::null_mut();

#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut tcp_listen_pcbs: *mut c_void = ptr::null_mut();

/// Helper to get the Rust handle from the C PCB
unsafe fn get_rust_conn(pcb: *const ffi::tcp_pcb) -> Option<&'static mut TcpConnection> {
    if pcb.is_null() || (*pcb).rust_handle.is_null() {
        None
    } else {
        Some(&mut *((*pcb).rust_handle as *mut TcpConnection))
    }
}

// --- Exported Accessors called by C macros ---

#[no_mangle]
pub unsafe extern "C" fn tcp_get_state_rust(pcb: *const ffi::tcp_pcb) -> u8 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.state as u8
    } else {
        TcpState::Closed as u8
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndbuf_rust(pcb: *const ffi::tcp_pcb) -> u16 {
    if let Some(conn) = get_rust_conn(pcb) {
        std::cmp::min(conn.snd_buf, 0xffff) as u16
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndqueuelen_rust(pcb: *const ffi::tcp_pcb) -> u16 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.snd_queuelen
    } else {
        0
    }
}

// Flag manipulation stubs (C writes)
#[no_mangle]
pub unsafe extern "C" fn tcp_set_flags_rust(pcb: *mut ffi::tcp_pcb, set_flags: u16) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.flags |= set_flags;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_clear_flags_rust(pcb: *mut ffi::tcp_pcb, clr_flags: u16) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.flags &= !clr_flags;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_is_flag_set_rust(pcb: *const ffi::tcp_pcb, flag: u16) -> i32 {
    if let Some(conn) = get_rust_conn(pcb) {
        if (conn.flags & flag) != 0 { 1 } else { 0 }
    } else {
        0
    }
}

// Keep-Alive Setters/Getters
#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_idle_rust(pcb: *mut ffi::tcp_pcb, val: u32) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_idle = val;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_idle_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_idle
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_intvl_rust(pcb: *mut ffi::tcp_pcb, val: u32) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_intvl = val;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_intvl_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_intvl
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_cnt_rust(pcb: *mut ffi::tcp_pcb, val: u32) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_cnt = val;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_cnt_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_cnt
    } else {
        0
    }
}

// Core FFI Exports that call safe TcpConnection methods

#[no_mangle]
pub unsafe extern "C" fn tcp_init_rust() {
    // No specific initialization needed for Rust part yet
}

#[no_mangle]
pub unsafe extern "C" fn tcp_tmr_rust() {
    // Handle global tick update
    tcp_ticks = tcp_ticks.wrapping_add(1);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_new_rust() -> *mut ffi::tcp_pcb {
    let pcb = ffi::tcp_alloc(ffi::TCP_PRIO_NORMAL as u8);
    if !pcb.is_null() {
        // Create Rust object
        let conn = Box::new(TcpConnection::new());
        // Store pointer in C struct
        (*pcb).rust_handle = Box::into_raw(conn) as *mut std::ffi::c_void;

        // Initialize C fields to safe defaults just in case
        (*pcb).snd_buf = 0xffff;
        (*pcb).snd_queuelen = 0;
        (*pcb).rcv_wnd = 0xffff;
        (*pcb).rcv_ann_wnd = 0xffff;
        (*pcb).mss = 536;
        (*pcb).rto = 3000 / 250;
        (*pcb).sv = 3000 / 250;
        (*pcb).sa = 0;
        (*pcb).ttl = 255;
    }
    pcb
}

#[no_mangle]
pub unsafe extern "C" fn tcp_bind_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
) -> i8 {
    let pcb_ref = &mut *pcb;
    if !ipaddr.is_null() {
        pcb_ref.local_ip = *ipaddr;
    }
    pcb_ref.local_port = port;

    // Register in bound list
    pcb_ref.next = tcp_bound_pcbs as *mut ffi::tcp_pcb;
    tcp_bound_pcbs = pcb as *mut std::ffi::c_void;

    0 // ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_connect_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
    connected: ffi::tcp_connected_fn,
) -> i8 {
    let conn = get_rust_conn(pcb).expect("Rust handle missing in tcp_connect");

    let pcb_ref = &mut *pcb;
    if !ipaddr.is_null() {
        pcb_ref.remote_ip = *ipaddr;
    }
    pcb_ref.remote_port = port;
    pcb_ref.connected = connected;

    // Add to active list
    pcb_ref.next = tcp_active_pcbs as *mut ffi::tcp_pcb;
    tcp_active_pcbs = pcb as *mut std::ffi::c_void;

    // Call safe logic
    conn.connect();

    tcp_enqueue_flags(pcb, TCP_SYN);
    tcp_output_rust(pcb);

    0 // ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_write_rust(
    pcb: *mut ffi::tcp_pcb,
    dataptr: *const c_void,
    len: u16,
    apiflags: u8,
) -> i8 {
    let conn = get_rust_conn(pcb).expect("Rust handle missing");

    match conn.write(len as u32) {
        Ok(_) => 0, // ERR_OK
        Err(_) => -1, // ERR_MEM
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_output_rust(pcb: *mut ffi::tcp_pcb) -> i8 {
    if !PENDING_PACKET.is_null() {
        let p = PENDING_PACKET;
        PENDING_PACKET = ptr::null_mut();
        ffi::pbuf_free(p);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_and_err_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
    err: *mut i8,
) -> *mut ffi::tcp_pcb {
    let lpcb = ffi::tcp_alloc(ffi::TCP_PRIO_NORMAL as u8) as *mut ffi::tcp_pcb_listen;

    if lpcb.is_null() {
        if !err.is_null() { *err = -1; } // ERR_MEM
        return ptr::null_mut();
    }

    // Allocate Rust struct for listener
    let conn = Box::new(TcpConnection::new());
    (*lpcb).rust_handle = Box::into_raw(conn) as *mut std::ffi::c_void;
    let conn = get_rust_conn(lpcb as *mut ffi::tcp_pcb).unwrap();

    (*lpcb).local_port = (*pcb).local_port;

    conn.listen();

    // Add to listen list
    (*lpcb).next = tcp_listen_pcbs as *mut ffi::tcp_pcb_listen;
    tcp_listen_pcbs = lpcb as *mut std::ffi::c_void;

    lpcb as *mut ffi::tcp_pcb // Cast back
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
) -> *mut ffi::tcp_pcb {
    tcp_listen_with_backlog_and_err_rust(pcb, backlog, ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn tcp_input_rust(p: *mut ffi::pbuf, inp: *mut ffi::netif) {
    // For now, we still parse raw pointers here because moving packet parsing logic
    // requires more robust pbuf wrappers. This is acceptable for the FFI boundary layer.
    if p.is_null() {
        return;
    }

    let tcphdr = (*p).payload as *mut ffi::tcp_hdr;
    let dest_port = u16::from_be((*tcphdr).dest);

    // ... match connection ...
    // This demux logic should eventually be cleaner, but it involves traversing raw C linked lists.
    // Keeping it here in lib.rs (the FFI layer) is appropriate.

    ffi::pbuf_free(p);
}

// Remaining Stubs
#[no_mangle]
pub unsafe extern "C" fn tcp_free_ooseq(_pcb: *mut c_void) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_close_rust(pcb: *mut ffi::tcp_pcb) -> i8 { 0 }
#[no_mangle]
pub unsafe extern "C" fn tcp_abort_rust(pcb: *mut ffi::tcp_pcb) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_recved_rust(pcb: *mut ffi::tcp_pcb, len: u16) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_arg_rust(pcb: *mut ffi::tcp_pcb, arg: *mut c_void) { (*pcb).callback_arg = arg; }
#[no_mangle]
pub unsafe extern "C" fn tcp_recv_rust(pcb: *mut ffi::tcp_pcb, recv: ffi::tcp_recv_fn) { (*pcb).recv = recv; }
#[no_mangle]
pub unsafe extern "C" fn tcp_sent_rust(pcb: *mut ffi::tcp_pcb, sent: ffi::tcp_sent_fn) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_poll_rust(pcb: *mut ffi::tcp_pcb, poll: ffi::tcp_poll_fn, interval: u8) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_err_rust(pcb: *mut ffi::tcp_pcb, err: ffi::tcp_err_fn) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_accept_rust(pcb: *mut ffi::tcp_pcb, accept: ffi::tcp_accept_fn) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_shutdown_rust(pcb: *mut ffi::tcp_pcb, shut_rx: i32, shut_tx: i32) -> i8 { 0 }
#[no_mangle]
pub unsafe extern "C" fn tcp_bind_netif_rust(pcb: *mut ffi::tcp_pcb, netif: *const ffi::netif) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_new_ip_type_rust(ip_type: u8) -> *mut ffi::tcp_pcb { tcp_new_rust() }
#[no_mangle]
pub unsafe extern "C" fn tcp_setprio_rust(pcb: *mut ffi::tcp_pcb, prio: u8) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_tcp_get_tcp_addrinfo_rust(pcb: *mut ffi::tcp_pcb, local: i32, addr: *mut ffi::ip_addr_t, port: *mut u16) -> i8 { 0 }
#[no_mangle]
pub unsafe extern "C" fn tcp_netif_ip_addr_changed_rust(old_addr: *const ffi::ip_addr_t, new_addr: *const ffi::ip_addr_t) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_delayed_rust(pcb: *mut ffi::tcp_pcb) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_backlog_accepted_rust(pcb: *mut ffi::tcp_pcb) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_alloc_id_rust() -> u8 { 0 }
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_callbacks_rust(pcb: *mut ffi::tcp_pcb, id: u8, callbacks: *const c_void) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_set_rust(pcb: *mut ffi::tcp_pcb, id: u8, arg: *mut c_void) {}
#[no_mangle]
pub unsafe extern "C" fn tcp_ext_arg_get_rust(pcb: *const ffi::tcp_pcb, id: u8) -> *mut c_void { ptr::null_mut() }

// Temporary helpers moved from tcp.rs for compilation
static mut PENDING_PACKET: *mut ffi::pbuf = ptr::null_mut();
unsafe fn tcp_enqueue_flags(pcb: *mut ffi::tcp_pcb, flags: u8) -> i8 {
    let p = ffi::pbuf_alloc(1, 20, 0);
    if p.is_null() { return -1; }
    PENDING_PACKET = p;
    0
}
