//! TCP Connection State Components
//!
//! This module defines the five disjoint state components according to the
//! modularization design:
//! 1. Connection Management
//! 2. Reliable Ordered Delivery
//! 3. Flow Control
//! 4. Congestion Control
//! 5. Demultiplexing
use crate::ffi;

/// TCP State Machine States
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(TcpState::Closed),
            1 => Some(TcpState::Listen),
            2 => Some(TcpState::SynSent),
            3 => Some(TcpState::SynRcvd),
            4 => Some(TcpState::Established),
            5 => Some(TcpState::FinWait1),
            6 => Some(TcpState::FinWait2),
            7 => Some(TcpState::CloseWait),
            8 => Some(TcpState::Closing),
            9 => Some(TcpState::LastAck),
            10 => Some(TcpState::TimeWait),
            _ => None,
        }
    }

    pub fn is_closing(&self) -> bool {
        *self >= TcpState::FinWait1
    }
}

/// Connection Management State
///
/// This component owns the TCP state machine and all connection lifecycle data.
/// Only the control path can write to this state.
pub struct ConnectionManagementState {
    /* Connection Identifier (Tuple) */
    pub local_ip: ffi::ip_addr_t,
    pub remote_ip: ffi::ip_addr_t,
    pub local_port: u16,
    pub remote_port: u16,

    /* Lifecycle State */
    pub state: TcpState,

    /* Timers & Keep-Alive */
    pub tmr: u32,
    pub polltmr: u8,
    pub pollinterval: u8,
    pub keep_idle: u32,
    pub keep_intvl: u32,
    pub keep_cnt: u32,
    pub keep_cnt_sent: u8,

    /* Static Connection Parameters & Options */
    pub mss: u16,
    pub so_options: u8,
    pub tos: u8,
    pub ttl: u8,
    pub prio: u8,
    pub flags: u16, // tcpflags_t

    /* Network Interface */
    pub netif_idx: u8,
}

/// Reliable Ordered Delivery State
///
/// Handles sequence numbers, ACKs, retransmissions, and buffering.
/// Only ROD event handlers can write to this state.
pub struct ReliableOrderedDeliveryState {
    /* Local & Remote Sequence Numbers */
    pub snd_nxt: u32,      // Next sequence number we will send
    pub rcv_nxt: u32,      // Next sequence number we expect from peer
    pub lastack: u32,      // Last cumulative ACK we received

    /* Initial Sequence Numbers (for handshake) */
    pub iss: u32,          // Our initial send sequence number
    pub irs: u32,          // Peer's initial receive sequence number

    /* Send Buffer Management */
    pub snd_lbb: u32,      // Sequence number of next byte to be buffered
    pub snd_buf: u16,      // Available space in send buffer (simplified for now)
    pub snd_queuelen: u16, // Number of pbufs in send queues
    pub bytes_acked: u16,  // Bytes acknowledged in current round

    /* Retransmission Timer & RTT Estimation */
    pub rtime: i16,        // Retransmission timer countdown
    pub rttest: u32,       // RTT measurement start time
    pub rtseq: u32,        // Sequence number being timed for RTT
    pub sa: i16,           // Smoothed RTT
    pub sv: i16,           // RTT variance
    pub rto: i16,          // Retransmission Timeout value
    pub nrtx: u8,          // Number of retransmissions

    /* Fast Retransmit / Recovery State */
    pub dupacks: u8,       // Duplicate ACK counter
    pub rto_end: u32,      // End of RTO recovery

    /* TCP Timestamps */
    pub ts_lastacksent: u32,
    pub ts_recent: u32,
}

/// Flow Control State
///
/// Manages receive and send windows.
/// Only FC event handlers can write to this state.
pub struct FlowControlState {
    /* Peer's Receive Window */
    pub snd_wnd: u16,          // Window the remote peer advertised
    pub snd_wnd_max: u16,      // Maximum window we've seen from peer
    pub snd_wl1: u32,          // For validating window updates
    pub snd_wl2: u32,          // For validating window updates

    /* Our Receive Window */
    pub rcv_wnd: u16,          // Our available receive buffer space
    pub rcv_ann_wnd: u16,      // Window we will advertise
    pub rcv_ann_right_edge: u32, // Right edge of advertised window

    /* Window Scaling */
    pub snd_scale: u8,         // Scale factor for our advertisements
    pub rcv_scale: u8,         // Scale factor for peer's advertisements

    /* Zero Window Probing */
    pub persist_cnt: u8,
    pub persist_backoff: u8,
    pub persist_probe: u8,
}

/// Congestion Control State
///
/// Manages congestion window and slow start threshold.
/// Only CC event handlers can write to this state.
pub struct CongestionControlState {
    pub cwnd: u16,       // Congestion Window
    pub ssthresh: u16,   // Slow Start Threshold
}

/// Demultiplexing State
///
/// Currently empty - demuxing uses the 4-tuple from ConnectionManagementState.
/// Included for completeness as per design document.
pub struct DemuxState {
    // Empty by design
}

/// Complete TCP Connection State
///
/// Aggregates all five state components.
/// This structure enforces the separation of concerns.
pub struct TcpConnectionState {
    pub conn_mgmt: ConnectionManagementState,
    pub rod: ReliableOrderedDeliveryState,
    pub flow_ctrl: FlowControlState,
    pub cong_ctrl: CongestionControlState,
    pub demux: DemuxState,
}

impl TcpConnectionState {
    /// Create a new TCP connection state with default values
    pub fn new() -> Self {
        Self {
            conn_mgmt: ConnectionManagementState {
                local_ip: unsafe { core::mem::zeroed() },
                remote_ip: unsafe { core::mem::zeroed() },
                local_port: 0,
                remote_port: 0,
                state: TcpState::Closed,
                tmr: 0,
                polltmr: 0,
                pollinterval: 0,
                keep_idle: 7200000, // TCP_KEEPIDLE_DEFAULT
                keep_intvl: 75000,  // TCP_KEEPINTVL_DEFAULT
                keep_cnt: 9,        // TCP_KEEPCNT_DEFAULT
                keep_cnt_sent: 0,
                mss: 536,           // Default MSS
                so_options: 0,
                tos: 0,
                ttl: 255,
                prio: 64,           // TCP_PRIO_NORMAL
                flags: 0,
                netif_idx: 0,
            },
            rod: ReliableOrderedDeliveryState {
                snd_nxt: 0,
                rcv_nxt: 0,
                lastack: 0,
                iss: 0,
                irs: 0,
                snd_lbb: 0,
                snd_buf: 0,
                snd_queuelen: 0,
                bytes_acked: 0,
                rtime: 0,
                rttest: 0,
                rtseq: 0,
                sa: 0,
                sv: 0,
                rto: 3000,          // Default RTO: 3 seconds
                nrtx: 0,
                dupacks: 0,
                rto_end: 0,
                ts_lastacksent: 0,
                ts_recent: 0,
            },
            flow_ctrl: FlowControlState {
                snd_wnd: 0,
                snd_wnd_max: 0,
                snd_wl1: 0,
                snd_wl2: 0,
                rcv_wnd: 0,
                rcv_ann_wnd: 0,
                rcv_ann_right_edge: 0,
                snd_scale: 0,
                rcv_scale: 0,
                persist_cnt: 0,
                persist_backoff: 0,
                persist_probe: 0,
            },
            cong_ctrl: CongestionControlState {
                cwnd: 0,
                ssthresh: 0xFFFF,   // Initial ssthresh is large
            },
            demux: DemuxState {},
        }
    }
}
