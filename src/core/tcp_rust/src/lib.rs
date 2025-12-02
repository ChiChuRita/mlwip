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
    #[derive(Debug, Copy, Clone, Default)]
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
        core::ptr::null_mut()
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
const ERR_VAL: i8 = -6;
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

#[inline]
unsafe fn pcb_to_state<'a>(pcb: *const ffi::tcp_pcb) -> Option<&'a TcpConnectionState> {
    if pcb.is_null() {
        None
    } else {
        Some(&*(pcb as *const TcpConnectionState))
    }
}

#[inline]
unsafe fn pcb_to_state_mut<'a>(pcb: *mut ffi::tcp_pcb) -> Option<&'a mut TcpConnectionState> {
    if pcb.is_null() {
        None
    } else {
        Some(&mut *(pcb as *mut TcpConnectionState))
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_init_rust() {
    tcp_ticks = 0;
    tcp_active_pcbs = ptr::null_mut();
    tcp_tw_pcbs = ptr::null_mut();
    tcp_bound_pcbs = ptr::null_mut();
    tcp_listen_pcbs = ptr::null_mut();
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
    let state = Box::new(TcpConnectionState::new());
    Box::into_raw(state) as *mut ffi::tcp_pcb
}

#[no_mangle]
pub unsafe extern "C" fn tcp_new_ip_type_rust(ip_type: u8) -> *mut ffi::tcp_pcb {
    tcp_new_rust()
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
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ERR_ARG;
    };

    let ip = if ipaddr.is_null() {
        ffi::ip_addr_t { addr: 0 }
    } else {
        *ipaddr
    };

    match tcp_bind(state, ip, port) {
        Ok(_) => ERR_OK,
        Err(_) => ERR_VAL,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_connect_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
    connected: ffi::tcp_connected_fn,
) -> i8 {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ERR_ARG;
    };

    if ipaddr.is_null() {
        return ERR_ARG;
    }

    state.connected_callback = connected.map(|f| {
        core::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *mut c_void, i8) -> i8>(f)
    });

    match tcp_connect(state, *ipaddr, port) {
        Ok(_) => ERR_OK,
        Err(_) => ERR_VAL,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_write_rust(
    pcb: *mut ffi::tcp_pcb,
    dataptr: *const c_void,
    len: u16,
    apiflags: u8,
) -> i8 {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ERR_ARG;
    };

    if dataptr.is_null() && len > 0 {
        return ERR_ARG;
    }

    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_output_rust(pcb: *mut ffi::tcp_pcb) -> i8 {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ERR_ARG;
    };
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_close_rust(pcb: *mut ffi::tcp_pcb) -> i8 {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ERR_ARG;
    };

    match initiate_close(state) {
        Ok(send_fin) => {
            if state.conn_mgmt.state == TcpState::Closed {
                let _ = Box::from_raw(pcb as *mut TcpConnectionState);
            }
            ERR_OK
        }
        Err(_) => ERR_VAL,
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_abort_rust(pcb: *mut ffi::tcp_pcb) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };

    let _ = tcp_abort(state);
    let _ = Box::from_raw(pcb as *mut TcpConnectionState);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_recved_rust(pcb: *mut ffi::tcp_pcb, len: u16) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.flow_ctrl.rcv_wnd = state.flow_ctrl.rcv_wnd.saturating_add(len);
}

#[no_mangle]
pub unsafe extern "C" fn tcp_arg_rust(pcb: *mut ffi::tcp_pcb, arg: *mut c_void) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.callback_arg = arg;
}

#[no_mangle]
pub unsafe extern "C" fn tcp_recv_rust(pcb: *mut ffi::tcp_pcb, recv: ffi::tcp_recv_fn) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.recv_callback = recv.map(|f| {
        core::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, i8) -> i8>(f)
    });
}

#[no_mangle]
pub unsafe extern "C" fn tcp_sent_rust(pcb: *mut ffi::tcp_pcb, sent: ffi::tcp_sent_fn) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.sent_callback = sent.map(|f| {
        core::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *mut c_void, u16) -> i8>(f)
    });
}

#[no_mangle]
pub unsafe extern "C" fn tcp_poll_rust(
    pcb: *mut ffi::tcp_pcb,
    poll: ffi::tcp_poll_fn,
    interval: u8,
) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.poll_callback = poll.map(|f| {
        core::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *mut c_void) -> i8>(f)
    });
    state.poll_interval = interval;
}

#[no_mangle]
pub unsafe extern "C" fn tcp_err_rust(pcb: *mut ffi::tcp_pcb, err: ffi::tcp_err_fn) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.err_callback = err.map(|f| {
        core::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, i8)>(f)
    });
}

#[no_mangle]
pub unsafe extern "C" fn tcp_accept_rust(pcb: *mut ffi::tcp_pcb, accept: ffi::tcp_accept_fn) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.accept_callback = accept.map(|f| {
        core::mem::transmute::<_, unsafe extern "C" fn(*mut c_void, *mut c_void, i8) -> i8>(f)
    });
}

#[no_mangle]
pub unsafe extern "C" fn tcp_shutdown_rust(pcb: *mut ffi::tcp_pcb, shut_rx: i32, shut_tx: i32) -> i8 {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ERR_ARG;
    };

    if shut_tx != 0 {
        let _ = initiate_close(state);
    }
    ERR_OK
}

#[no_mangle]
pub unsafe extern "C" fn tcp_bind_netif_rust(pcb: *mut ffi::tcp_pcb, _netif: *const ffi::netif) {
    // netif binding tracked but not currently used
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
) -> *mut ffi::tcp_pcb {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ptr::null_mut();
    };

    match tcp_listen(state) {
        Ok(_) => pcb,
        Err(_) => ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_listen_with_backlog_and_err_rust(
    pcb: *mut ffi::tcp_pcb,
    backlog: u8,
    err: *mut i8,
) -> *mut ffi::tcp_pcb {
    let Some(state) = pcb_to_state_mut(pcb) else {
        if !err.is_null() {
            *err = ERR_ARG;
        }
        return ptr::null_mut();
    };

    match tcp_listen(state) {
        Ok(_) => {
            if !err.is_null() {
                *err = ERR_OK;
            }
            pcb
        }
        Err(_) => {
            if !err.is_null() {
                *err = ERR_VAL;
            }
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_setprio_rust(pcb: *mut ffi::tcp_pcb, prio: u8) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.conn_mgmt.prio = prio;
}

#[no_mangle]
pub unsafe extern "C" fn tcp_tcp_get_tcp_addrinfo_rust(
    pcb: *mut ffi::tcp_pcb,
    local: i32,
    addr: *mut ffi::ip_addr_t,
    port: *mut u16,
) -> i8 {
    let Some(state) = pcb_to_state(pcb) else {
        return ERR_ARG;
    };

    if local != 0 {
        if !addr.is_null() {
            *addr = state.conn_mgmt.local_ip;
        }
        if !port.is_null() {
            *port = state.conn_mgmt.local_port;
        }
    } else {
        if !addr.is_null() {
            *addr = state.conn_mgmt.remote_ip;
        }
        if !port.is_null() {
            *port = state.conn_mgmt.remote_port;
        }
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
    static mut EXT_ARG_ID: u8 = 0;
    let id = EXT_ARG_ID;
    EXT_ARG_ID = EXT_ARG_ID.wrapping_add(1);
    id
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
    let Some(state) = pcb_to_state(pcb) else {
        return 0;
    };
    state.conn_mgmt.state as u8
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndbuf_rust(pcb: *const ffi::tcp_pcb) -> u16 {
    let Some(state) = pcb_to_state(pcb) else {
        return 0;
    };
    state.rod.snd_buf
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndqueuelen_rust(pcb: *const ffi::tcp_pcb) -> u16 {
    let Some(state) = pcb_to_state(pcb) else {
        return 0;
    };
    state.rod.snd_queuelen
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_flags_rust(pcb: *mut ffi::tcp_pcb, set_flags: u16) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.conn_mgmt.flags |= set_flags;
}

#[no_mangle]
pub unsafe extern "C" fn tcp_clear_flags_rust(pcb: *mut ffi::tcp_pcb, clr_flags: u16) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.conn_mgmt.flags &= !clr_flags;
}

#[no_mangle]
pub unsafe extern "C" fn tcp_is_flag_set_rust(pcb: *const ffi::tcp_pcb, flag: u16) -> i32 {
    let Some(state) = pcb_to_state(pcb) else {
        return 0;
    };
    if (state.conn_mgmt.flags & flag) != 0 { 1 } else { 0 }
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
    ISS = ISS.wrapping_add(64000);
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
    let Some(state) = pcb_to_state(pcb) else {
        return 7200000;
    };
    state.conn_mgmt.keep_idle
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_idle_rust(pcb: *mut ffi::tcp_pcb, idle: u32) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.conn_mgmt.keep_idle = idle;
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_intvl_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    let Some(state) = pcb_to_state(pcb) else {
        return 75000;
    };
    state.conn_mgmt.keep_intvl
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_intvl_rust(pcb: *mut ffi::tcp_pcb, intvl: u32) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.conn_mgmt.keep_intvl = intvl;
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_cnt_rust(pcb: *const ffi::tcp_pcb) -> u32 {
    let Some(state) = pcb_to_state(pcb) else {
        return 9;
    };
    state.conn_mgmt.keep_cnt
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_cnt_rust(pcb: *mut ffi::tcp_pcb, cnt: u32) {
    let Some(state) = pcb_to_state_mut(pcb) else {
        return;
    };
    state.conn_mgmt.keep_cnt = cnt;
}

#[cfg(test)]
mod ffi_tests {
    use super::*;

    #[test]
    fn test_tcp_new_allocates_state() {
        unsafe {
            let pcb = tcp_new_rust();
            assert!(!pcb.is_null());

            let state = pcb_to_state(pcb).unwrap();
            assert_eq!(state.conn_mgmt.state, TcpState::Closed);

            tcp_abort_rust(pcb);
        }
    }

    #[test]
    fn test_tcp_bind_sets_address() {
        unsafe {
            let pcb = tcp_new_rust();
            assert!(!pcb.is_null());

            let addr = ffi::ip_addr_t { addr: 0x0100007f }; // 127.0.0.1
            let result = tcp_bind_rust(pcb, &addr, 8080);
            assert_eq!(result, ERR_OK);

            let state = pcb_to_state(pcb).unwrap();
            assert_eq!(state.conn_mgmt.local_port, 8080);
            assert_eq!(state.conn_mgmt.local_ip.addr, 0x0100007f);

            tcp_abort_rust(pcb);
        }
    }

    #[test]
    fn test_tcp_listen_transitions_state() {
        unsafe {
            let pcb = tcp_new_rust();

            let addr = ffi::ip_addr_t { addr: 0 };
            tcp_bind_rust(pcb, &addr, 8080);

            let listen_pcb = tcp_listen_with_backlog_rust(pcb, 5);
            assert!(!listen_pcb.is_null());

            assert_eq!(tcp_get_state_rust(listen_pcb), TcpState::Listen as u8);

            tcp_abort_rust(listen_pcb);
        }
    }

    #[test]
    fn test_tcp_connect_transitions_to_syn_sent() {
        unsafe {
            let pcb = tcp_new_rust();

            let local_addr = ffi::ip_addr_t { addr: 0 };
            tcp_bind_rust(pcb, &local_addr, 0);

            let remote_addr = ffi::ip_addr_t { addr: 0x0100007f };
            let result = tcp_connect_rust(pcb, &remote_addr, 80, None);
            assert_eq!(result, ERR_OK);

            assert_eq!(tcp_get_state_rust(pcb), TcpState::SynSent as u8);

            let state = pcb_to_state(pcb).unwrap();
            assert_eq!(state.conn_mgmt.remote_port, 80);
            assert!(state.rod.iss > 0);

            tcp_abort_rust(pcb);
        }
    }

    #[test]
    fn test_tcp_getters_return_correct_values() {
        unsafe {
            let pcb = tcp_new_rust();

            tcp_set_keep_idle_rust(pcb, 60000);
            assert_eq!(tcp_get_keep_idle_rust(pcb), 60000);

            tcp_set_keep_intvl_rust(pcb, 10000);
            assert_eq!(tcp_get_keep_intvl_rust(pcb), 10000);

            tcp_set_keep_cnt_rust(pcb, 5);
            assert_eq!(tcp_get_keep_cnt_rust(pcb), 5);

            tcp_setprio_rust(pcb, 100);
            let state = pcb_to_state(pcb).unwrap();
            assert_eq!(state.conn_mgmt.prio, 100);

            tcp_abort_rust(pcb);
        }
    }

    #[test]
    fn test_tcp_flags_operations() {
        unsafe {
            let pcb = tcp_new_rust();

            tcp_set_flags_rust(pcb, 0x01);
            assert_eq!(tcp_is_flag_set_rust(pcb, 0x01), 1);
            assert_eq!(tcp_is_flag_set_rust(pcb, 0x02), 0);

            tcp_set_flags_rust(pcb, 0x02);
            assert_eq!(tcp_is_flag_set_rust(pcb, 0x01), 1);
            assert_eq!(tcp_is_flag_set_rust(pcb, 0x02), 1);

            tcp_clear_flags_rust(pcb, 0x01);
            assert_eq!(tcp_is_flag_set_rust(pcb, 0x01), 0);
            assert_eq!(tcp_is_flag_set_rust(pcb, 0x02), 1);

            tcp_abort_rust(pcb);
        }
    }

    #[test]
    fn test_tcp_callback_arg() {
        unsafe {
            let pcb = tcp_new_rust();

            let mut data: u32 = 42;
            let data_ptr = &mut data as *mut u32 as *mut c_void;

            tcp_arg_rust(pcb, data_ptr);

            let state = pcb_to_state(pcb).unwrap();
            assert_eq!(state.callback_arg, data_ptr);

            tcp_abort_rust(pcb);
        }
    }

    #[test]
    fn test_tcp_addrinfo() {
        unsafe {
            let pcb = tcp_new_rust();

            let local_addr = ffi::ip_addr_t { addr: 0x0100007f };
            tcp_bind_rust(pcb, &local_addr, 8080);

            let remote_addr = ffi::ip_addr_t { addr: 0x0200007f };
            tcp_connect_rust(pcb, &remote_addr, 80, None);

            let mut addr = ffi::ip_addr_t { addr: 0 };
            let mut port: u16 = 0;

            tcp_tcp_get_tcp_addrinfo_rust(pcb, 1, &mut addr, &mut port);
            assert_eq!(addr.addr, 0x0100007f);
            assert_eq!(port, 8080);

            tcp_tcp_get_tcp_addrinfo_rust(pcb, 0, &mut addr, &mut port);
            assert_eq!(addr.addr, 0x0200007f);
            assert_eq!(port, 80);

            tcp_abort_rust(pcb);
        }
    }

    #[test]
    fn test_tcp_close_deallocates() {
        unsafe {
            let pcb = tcp_new_rust();

            let result = tcp_close_rust(pcb);
            assert_eq!(result, ERR_OK);
        }
    }

    #[test]
    fn test_null_pcb_handling() {
        unsafe {
            assert_eq!(tcp_bind_rust(ptr::null_mut(), ptr::null(), 80), ERR_ARG);
            assert_eq!(tcp_connect_rust(ptr::null_mut(), ptr::null(), 80, None), ERR_ARG);
            assert_eq!(tcp_close_rust(ptr::null_mut()), ERR_ARG);
            assert_eq!(tcp_get_state_rust(ptr::null()), 0);
            assert_eq!(tcp_get_sndbuf_rust(ptr::null()), 0);
        }
    }
}
