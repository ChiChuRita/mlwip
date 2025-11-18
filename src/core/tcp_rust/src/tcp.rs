use crate::ffi;
use std::ptr;

// TCP Flags
const TCP_FIN: u8 = 0x01;
const TCP_SYN: u8 = 0x02;
const TCP_RST: u8 = 0x04;
const TCP_PSH: u8 = 0x08;
const TCP_ACK: u8 = 0x10;
const TCP_URG: u8 = 0x20;
const TCP_ECE: u8 = 0x40;
const TCP_CWR: u8 = 0x80;

// TCP States
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum TcpState {
    Closed = 0,
    Listen = 1,
    SynSent = 2,
    SynRcvd = 3,
    Established = 4,
    FinWait1 = 5,
    FinWait2 = 6,
    CloseWait = 7,
    Closing = 8,
    LastAck = 9,
    TimeWait = 10,
}

impl TcpState {
    pub fn to_u32(self) -> u32 {
        self as u8 as u32
    }

    pub fn from_u32(val: u32) -> Self {
        match val {
            0 => TcpState::Closed,
            1 => TcpState::Listen,
            2 => TcpState::SynSent,
            3 => TcpState::SynRcvd,
            4 => TcpState::Established,
            5 => TcpState::FinWait1,
            6 => TcpState::FinWait2,
            7 => TcpState::CloseWait,
            8 => TcpState::Closing,
            9 => TcpState::LastAck,
            10 => TcpState::TimeWait,
            _ => TcpState::Closed, // Default/Error
        }
    }
}

/// Pure Rust TCP Connection structure.
/// This is completely separate from the C layout.
pub struct TcpConnection {
    pub state: TcpState,
    pub snd_buf: u32,
    pub snd_queuelen: u16,
    pub flags: u16,

    // Keep Alive configuration
    pub keep_idle: u32,
    pub keep_intvl: u32,
    pub keep_cnt: u32,
}

impl TcpConnection {
    pub fn new() -> Self {
        Self {
            state: TcpState::Closed,
            snd_buf: 0xffff,
            snd_queuelen: 0,
            flags: 0,
            keep_idle: 7200000, // Default 2 hours ms
            keep_intvl: 75000,  // Default 75 sec ms
            keep_cnt: 9,        // Default 9 probes
        }
    }
}

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
pub unsafe extern "C" fn tcp_get_state(pcb: *const ffi::tcp_pcb) -> u8 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.state as u8
    } else {
        TcpState::Closed as u8
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndbuf(pcb: *const ffi::tcp_pcb) -> u16 {
    if let Some(conn) = get_rust_conn(pcb) {
        // Cap at u16 max because lwIP API expects u16 usually, though macro might handle larger
        std::cmp::min(conn.snd_buf, 0xffff) as u16
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_sndqueuelen(pcb: *const ffi::tcp_pcb) -> u16 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.snd_queuelen
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_flags(pcb: *mut ffi::tcp_pcb, set_flags: u16) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.flags |= set_flags;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_clear_flags(pcb: *mut ffi::tcp_pcb, clr_flags: u16) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.flags &= !clr_flags;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_is_flag_set(pcb: *const ffi::tcp_pcb, flag: u16) -> i32 {
    if let Some(conn) = get_rust_conn(pcb) {
        if (conn.flags & flag) != 0 { 1 } else { 0 }
    } else {
        0
    }
}

// Keep-Alive Setters/Getters
#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_idle(pcb: *mut ffi::tcp_pcb, val: u32) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_idle = val;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_idle(pcb: *const ffi::tcp_pcb) -> u32 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_idle
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_intvl(pcb: *mut ffi::tcp_pcb, val: u32) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_intvl = val;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_intvl(pcb: *const ffi::tcp_pcb) -> u32 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_intvl
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_set_keep_cnt(pcb: *mut ffi::tcp_pcb, val: u32) {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_cnt = val;
    }
}

#[no_mangle]
pub unsafe extern "C" fn tcp_get_keep_cnt(pcb: *const ffi::tcp_pcb) -> u32 {
    if let Some(conn) = get_rust_conn(pcb) {
        conn.keep_cnt
    } else {
        0
    }
}

pub unsafe fn tcp_init() {
    // No specific initialization needed for Rust part yet
}

pub unsafe fn tcp_tmr() {
    crate::tcp_ticks = crate::tcp_ticks.wrapping_add(1);
}

pub unsafe fn tcp_new() -> *mut ffi::tcp_pcb {
    let pcb = ffi::tcp_alloc(ffi::TCP_PRIO_NORMAL as u8);
    if !pcb.is_null() {
        // Create Rust object
        let conn = Box::new(TcpConnection::new());
        // Store pointer in C struct
        (*pcb).rust_handle = Box::into_raw(conn) as *mut std::ffi::c_void;

        // Initialize C fields that might still be accessed directly or need defaults
        // Although we try to move to accessors, some might remain.
        // For safety, we keep them somewhat consistent for now, but truth is in rust_handle.
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

pub unsafe fn tcp_bind(pcb: *mut ffi::tcp_pcb, ipaddr: *const ffi::ip_addr_t, port: u16) -> i8 {
    let pcb_ref = &mut *pcb;
    if !ipaddr.is_null() {
        pcb_ref.local_ip = *ipaddr;
    }
    pcb_ref.local_port = port;

    // Register in bound list
    pcb_ref.next = crate::tcp_bound_pcbs as *mut ffi::tcp_pcb;
    crate::tcp_bound_pcbs = pcb as *mut std::ffi::c_void;

    0 // ERR_OK
}

pub unsafe fn tcp_connect(
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
    pcb_ref.next = crate::tcp_active_pcbs as *mut ffi::tcp_pcb;
    crate::tcp_active_pcbs = pcb as *mut std::ffi::c_void;

    // Update RUST state
    conn.state = TcpState::SynSent;

    // Keep C state in sync just in case (optional if accessors work 100%)
    // But for now, we rely on accessors.
    // (*pcb).state = conn.state.to_u32() as _;

    tcp_enqueue_flags(pcb, TCP_SYN);
    tcp_output(pcb);

    0 // ERR_OK
}

pub unsafe fn tcp_input(p: *mut ffi::pbuf, inp: *mut ffi::netif) {
    if p.is_null() {
        return;
    }

    let tcphdr = (*p).payload as *mut ffi::tcp_hdr;
    let hdrlen_rsvd_flags = (*tcphdr)._hdrlen_rsvd_flags;
    let flags = u16::from_be(hdrlen_rsvd_flags) & 0x3F;
    let dest_port = u16::from_be((*tcphdr).dest);

    // Check active PCBs
    let mut pcb = crate::tcp_active_pcbs as *mut ffi::tcp_pcb;
    while !pcb.is_null() {
        if (*pcb).local_port == dest_port {
            tcp_process(pcb, tcphdr, p);
            return;
        }
        pcb = (*pcb).next;
    }

    // Check listen PCBs
    let mut lpcb = crate::tcp_listen_pcbs as *mut ffi::tcp_pcb_listen;
    while !lpcb.is_null() {
        if (*lpcb).local_port == dest_port {
            tcp_listen_input(lpcb, tcphdr, p);
            return;
        }
        lpcb = (*lpcb).next;
    }

    ffi::pbuf_free(p);
}

unsafe fn tcp_listen_input(
    pcb: *mut ffi::tcp_pcb_listen,
    tcphdr: *mut ffi::tcp_hdr,
    p: *mut ffi::pbuf,
) {
    let hdrlen_rsvd_flags = (*tcphdr)._hdrlen_rsvd_flags;
    let flags = (u16::from_be(hdrlen_rsvd_flags) & 0x3F) as u8;

    if (flags & TCP_SYN) != 0 {
        // Create new PCB
        let npcb = ffi::tcp_alloc(ffi::TCP_PRIO_NORMAL as u8);
        if npcb.is_null() {
            ffi::pbuf_free(p);
            return;
        }

        // Initialize Rust struct for new PCB
        let conn = Box::new(TcpConnection::new());
        (*npcb).rust_handle = Box::into_raw(conn) as *mut std::ffi::c_void;
        let conn = get_rust_conn(npcb).unwrap();

        // Set up new PCB
        (*npcb).local_port = (*pcb).local_port;
        (*npcb).remote_port = u16::from_be((*tcphdr).src);

        // Update Rust State
        conn.state = TcpState::SynRcvd;

        (*npcb).rcv_nxt = u32::from_be((*tcphdr).seqno) + 1;
        (*npcb).snd_wnd = u16::from_be((*tcphdr).wnd) as u32;
        (*npcb).mss = 536;
        (*npcb).rcv_wnd = 0xffff;
        (*npcb).snd_wl1 = u32::from_be((*tcphdr).seqno).wrapping_sub(1);

        // Register active
        (*npcb).next = crate::tcp_active_pcbs as *mut ffi::tcp_pcb;
        crate::tcp_active_pcbs = npcb as *mut std::ffi::c_void;

        tcp_enqueue_flags(npcb, TCP_SYN | TCP_ACK);
        tcp_output(npcb);
    }

    ffi::pbuf_free(p);
}

unsafe fn tcp_process(
    pcb: *mut ffi::tcp_pcb,
    tcphdr: *mut ffi::tcp_hdr,
    p: *mut ffi::pbuf,
) -> i8 {
    let conn = get_rust_conn(pcb).expect("Rust handle missing in tcp_process");

    let hdrlen_rsvd_flags = (*tcphdr)._hdrlen_rsvd_flags;
    let flags = (u16::from_be(hdrlen_rsvd_flags) & 0x3F) as u8;
    let seqno = u32::from_be((*tcphdr).seqno);
    let ackno = u32::from_be((*tcphdr).ackno);

    (*pcb).rcv_nxt = seqno.wrapping_add((*p).tot_len as u32);
    if (flags & (TCP_FIN | TCP_SYN)) != 0 {
        (*pcb).rcv_nxt = (*pcb).rcv_nxt.wrapping_add(1);
    }

    match conn.state {
        TcpState::SynSent => {
            if (flags & (TCP_SYN | TCP_ACK)) == (TCP_SYN | TCP_ACK) {
                if ackno == (*pcb).snd_nxt {
                    conn.state = TcpState::Established;
                    (*pcb).rcv_nxt = seqno.wrapping_add(1);
                    (*pcb).snd_wnd = u16::from_be((*tcphdr).wnd) as u32;
                    (*pcb).snd_wl1 = seqno.wrapping_sub(1);

                    tcp_send_empty_ack(pcb);

                    if let Some(connected) = (*pcb).connected {
                        connected((*pcb).callback_arg, pcb, 0);
                    }
                }
            }
        }
        TcpState::SynRcvd => {
            if (flags & TCP_ACK) != 0 {
                if ackno == (*pcb).snd_nxt {
                    conn.state = TcpState::Established;
                }
            }
        }
        TcpState::Established => {
            if (flags & TCP_ACK) != 0 {
                // Handle ACK
            }
            if (flags & TCP_FIN) != 0 {
                conn.state = TcpState::CloseWait;
                tcp_send_empty_ack(pcb);
            }
        }
        _ => {}
    }

    ffi::pbuf_free(p);
    0 // ERR_OK
}

// Stub helpers
pub unsafe fn tcp_enqueue_flags(pcb: *mut ffi::tcp_pcb, flags: u8) -> i8 {
    let p = ffi::pbuf_alloc(1, 20, 0);
    if p.is_null() { return -1; }

    let tcphdr = (*p).payload as *mut ffi::tcp_hdr;
    ptr::write_bytes(tcphdr, 0, 1);

    (*tcphdr).src = u16::to_be((*pcb).local_port);
    (*tcphdr).dest = u16::to_be((*pcb).remote_port);
    (*tcphdr).seqno = u32::to_be((*pcb).snd_nxt);
    (*tcphdr).ackno = u32::to_be((*pcb).rcv_nxt);
    (*tcphdr)._hdrlen_rsvd_flags = u16::to_be(0x5000 | (flags as u16));
    (*tcphdr).wnd = u16::to_be((*pcb).rcv_ann_wnd);

    if (flags & (TCP_SYN | TCP_FIN)) != 0 {
        (*pcb).snd_nxt = (*pcb).snd_nxt.wrapping_add(1);
    }

    PENDING_PACKET = p;
    0
}

static mut PENDING_PACKET: *mut ffi::pbuf = ptr::null_mut();

pub unsafe fn tcp_output(pcb: *mut ffi::tcp_pcb) -> i8 {
    if !PENDING_PACKET.is_null() {
        let p = PENDING_PACKET;
        PENDING_PACKET = ptr::null_mut();

        /*
        ffi::ip_output_if(
            p,
            &(*pcb).local_ip,
            &(*pcb).remote_ip,
            (*pcb).ttl,
            0, // tos
            6, // IP_PROTO_TCP
            ptr::null_mut(), // netif (optional usually, or find route)
        );
        */

        ffi::pbuf_free(p);
    }
    0
}

pub unsafe fn tcp_send_empty_ack(pcb: *mut ffi::tcp_pcb) -> i8 {
    tcp_enqueue_flags(pcb, TCP_ACK);
    tcp_output(pcb);
    0
}

// Stubs
pub unsafe fn tcp_write(pcb: *mut ffi::tcp_pcb, dataptr: *const std::ffi::c_void, len: u16, apiflags: u8) -> i8 {
    let conn = get_rust_conn(pcb).expect("Rust handle missing");

    if (len as u32) > conn.snd_buf {
        return -1; // ERR_MEM
    }

    // Update RUST state
    conn.snd_buf -= len as u32;
    conn.snd_queuelen += 1;

    0
}
pub unsafe fn tcp_close(pcb: *mut ffi::tcp_pcb) -> i8 {
    // Free Rust object when closing
    // Note: real close logic is complex (FIN wait etc)
    // For now, just stub
    0
}
pub unsafe fn tcp_abort(pcb: *mut ffi::tcp_pcb) {}
pub unsafe fn tcp_recved(pcb: *mut ffi::tcp_pcb, len: u16) {}
pub unsafe fn tcp_arg(pcb: *mut ffi::tcp_pcb, arg: *mut std::ffi::c_void) {
    (*pcb).callback_arg = arg;
}
pub unsafe fn tcp_recv(pcb: *mut ffi::tcp_pcb, recv: ffi::tcp_recv_fn) {
    (*pcb).recv = recv;
}
pub unsafe fn tcp_sent(pcb: *mut ffi::tcp_pcb, sent: ffi::tcp_sent_fn) {}
pub unsafe fn tcp_poll(pcb: *mut ffi::tcp_pcb, poll: ffi::tcp_poll_fn, interval: u8) {}
pub unsafe fn tcp_err(pcb: *mut ffi::tcp_pcb, err: ffi::tcp_err_fn) {}
pub unsafe fn tcp_accept(pcb: *mut ffi::tcp_pcb, accept: ffi::tcp_accept_fn) {}
pub unsafe fn tcp_shutdown(pcb: *mut ffi::tcp_pcb, shut_rx: i32, shut_tx: i32) -> i8 { 0 }
pub unsafe fn tcp_bind_netif(pcb: *mut ffi::tcp_pcb, netif: *const ffi::netif) {}
pub unsafe fn tcp_listen_with_backlog_and_err(pcb: *mut ffi::tcp_pcb, backlog: u8, err: *mut i8) -> *mut ffi::tcp_pcb {
    let lpcb = ffi::tcp_alloc(ffi::TCP_PRIO_NORMAL as u8) as *mut ffi::tcp_pcb_listen;

    if lpcb.is_null() {
        if !err.is_null() { *err = -1; } // ERR_MEM
        return ptr::null_mut();
    }

    // Allocate Rust struct
    let conn = Box::new(TcpConnection::new());
    (*lpcb).rust_handle = Box::into_raw(conn) as *mut std::ffi::c_void;
    let conn = get_rust_conn(lpcb as *mut ffi::tcp_pcb).unwrap();

    (*lpcb).local_port = (*pcb).local_port;

    // Set Rust State
    conn.state = TcpState::Listen;

    // Add to listen list
    (*lpcb).next = crate::tcp_listen_pcbs as *mut ffi::tcp_pcb_listen;
    crate::tcp_listen_pcbs = lpcb as *mut std::ffi::c_void;

    // In real lwIP, original pcb is freed.
    // ffi::memp_free(ffi::memp_t::MEMP_TCP_PCB, pcb as *mut std::ffi::c_void);

    lpcb as *mut ffi::tcp_pcb // Cast back
}
pub unsafe fn tcp_new_ip_type(ip_type: u8) -> *mut ffi::tcp_pcb { tcp_new() }
pub unsafe fn tcp_setprio(pcb: *mut ffi::tcp_pcb, prio: u8) {}
pub unsafe fn tcp_tcp_get_tcp_addrinfo(pcb: *mut ffi::tcp_pcb, local: i32, addr: *mut ffi::ip_addr_t, port: *mut u16) -> i8 { 0 }
pub unsafe fn tcp_netif_ip_addr_changed(old_addr: *const ffi::ip_addr_t, new_addr: *const ffi::ip_addr_t) {}
pub unsafe fn tcp_backlog_delayed(pcb: *mut ffi::tcp_pcb) {}
pub unsafe fn tcp_backlog_accepted(pcb: *mut ffi::tcp_pcb) {}
pub unsafe fn tcp_ext_arg_alloc_id() -> u8 { 0 }
pub unsafe fn tcp_ext_arg_set_callbacks(pcb: *mut ffi::tcp_pcb, id: u8, callbacks: *const std::ffi::c_void) {}
pub unsafe fn tcp_ext_arg_set(pcb: *mut ffi::tcp_pcb, id: u8, arg: *mut std::ffi::c_void) {}
pub unsafe fn tcp_ext_arg_get(pcb: *const ffi::tcp_pcb, id: u8) -> *mut std::ffi::c_void { ptr::null_mut() }
