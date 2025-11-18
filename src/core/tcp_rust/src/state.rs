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

// ============================================================================
// Component Method Implementations
// ============================================================================

// We need to forward-declare TcpSegment since it's defined in control_path.rs
// This will be resolved when we refactor the modules
use crate::control_path::TcpSegment;

// ============================================================================
// Connection Management State Methods
// ============================================================================

impl ConnectionManagementState {
    // ------------------------------------------------------------------------
    // Connection Setup (Handshake)
    // ------------------------------------------------------------------------

    /// LISTEN → SYN_RCVD: Handle incoming SYN
    /// Store remote endpoint and transition state
    pub fn on_syn_in_listen(
        &mut self,
        remote_ip: ffi::ip_addr_t,
        remote_port: u16,
    ) -> Result<(), &'static str> {
        // Validate we're in LISTEN state
        if self.state != TcpState::Listen {
            return Err("Not in LISTEN state");
        }

        // Store remote endpoint
        self.remote_ip = remote_ip;
        self.remote_port = remote_port;

        // Transition to SYN_RCVD
        self.state = TcpState::SynRcvd;

        Ok(())
    }

    /// SYN_SENT → ESTABLISHED: Handle incoming SYN+ACK (active open)
    /// Transition to ESTABLISHED
    pub fn on_synack_in_synsent(&mut self) -> Result<(), &'static str> {
        // Validate we're in SYN_SENT state
        if self.state != TcpState::SynSent {
            return Err("Not in SYN_SENT state");
        }

        // Transition to ESTABLISHED
        self.state = TcpState::Established;

        Ok(())
    }

    /// SYN_RCVD → ESTABLISHED: Handle ACK of our SYN (passive open)
    /// Transition to ESTABLISHED
    pub fn on_ack_in_synrcvd(&mut self) -> Result<(), &'static str> {
        // Validate we're in SYN_RCVD state
        if self.state != TcpState::SynRcvd {
            return Err("Not in SYN_RCVD state");
        }

        // Transition to ESTABLISHED
        self.state = TcpState::Established;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Connection Teardown (Close)
    // ------------------------------------------------------------------------

    /// ESTABLISHED → FIN_WAIT_1: Application initiates close
    pub fn on_close_in_established(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Established {
            return Err("Not in ESTABLISHED state");
        }

        // Transition to FIN_WAIT_1
        self.state = TcpState::FinWait1;

        Ok(())
    }

    /// CLOSE_WAIT → LAST_ACK: Application closes after receiving peer's FIN
    pub fn on_close_in_closewait(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::CloseWait {
            return Err("Not in CLOSE_WAIT state");
        }

        // Transition to LAST_ACK
        self.state = TcpState::LastAck;

        Ok(())
    }

    /// ESTABLISHED → CLOSE_WAIT: Receive FIN from peer (passive close)
    pub fn on_fin_in_established(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Established {
            return Err("Not in ESTABLISHED state");
        }

        // Transition to CLOSE_WAIT
        self.state = TcpState::CloseWait;

        Ok(())
    }

    /// FIN_WAIT_1 → FIN_WAIT_2: ACK of our FIN received
    pub fn on_ack_in_finwait1(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::FinWait1 {
            return Err("Not in FIN_WAIT_1 state");
        }

        // Transition to FIN_WAIT_2
        self.state = TcpState::FinWait2;

        Ok(())
    }

    /// FIN_WAIT_1 → CLOSING: Receive FIN (simultaneous close)
    pub fn on_fin_in_finwait1(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::FinWait1 {
            return Err("Not in FIN_WAIT_1 state");
        }

        // Transition to CLOSING (simultaneous close)
        self.state = TcpState::Closing;

        Ok(())
    }

    /// FIN_WAIT_2 → TIME_WAIT: Receive FIN
    pub fn on_fin_in_finwait2(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::FinWait2 {
            return Err("Not in FIN_WAIT_2 state");
        }

        // Transition to TIME_WAIT
        self.state = TcpState::TimeWait;

        Ok(())
    }

    /// CLOSING → TIME_WAIT: ACK of our FIN received (simultaneous close)
    pub fn on_ack_in_closing(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Closing {
            return Err("Not in CLOSING state");
        }

        // Transition to TIME_WAIT
        self.state = TcpState::TimeWait;

        Ok(())
    }

    /// LAST_ACK → CLOSED: ACK of our FIN received (passive close complete)
    pub fn on_ack_in_lastack(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::LastAck {
            return Err("Not in LAST_ACK state");
        }

        // Transition to CLOSED
        self.state = TcpState::Closed;

        Ok(())
    }

    /// TIME_WAIT → CLOSED: 2MSL timer expires
    pub fn on_timewait_timeout(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Implement 2MSL timeout handling")
    }

    // ------------------------------------------------------------------------
    // Reset Handling
    // ------------------------------------------------------------------------

    /// ANY → CLOSED: Receive RST or send RST
    pub fn on_rst(&mut self) -> Result<(), &'static str> {
        // Transition to CLOSED
        self.state = TcpState::Closed;
        // TODO: Clean up resources (timers, etc.)

        Ok(())
    }

    /// ANY → CLOSED: Abort connection (send RST)
    pub fn on_abort(&mut self) -> Result<(), &'static str> {
        // Immediately close
        self.state = TcpState::Closed;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // API-Initiated State Changes
    // ------------------------------------------------------------------------

    /// CLOSED → CLOSED: Bind to local address/port
    pub fn on_bind(
        &mut self,
        local_ip: ffi::ip_addr_t,
        local_port: u16,
    ) -> Result<u16, &'static str> {
        unimplemented!("TODO: Migrate from control_path::tcp_bind")
    }

    /// CLOSED → LISTEN: Start listening for connections
    pub fn on_listen(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Migrate from control_path::tcp_listen")
    }

    /// CLOSED → SYN_SENT: Initiate active connection
    pub fn on_connect(
        &mut self,
        remote_ip: ffi::ip_addr_t,
        remote_port: u16,
    ) -> Result<(), &'static str> {
        unimplemented!("TODO: Migrate from control_path::tcp_connect")
    }

    // ------------------------------------------------------------------------
    // No-op handlers (Connection Management doesn't change in these states)
    // ------------------------------------------------------------------------

    /// ESTABLISHED: Handle data/ACK (no state transition)
    pub fn on_data_in_established(&mut self) -> Result<(), &'static str> {
        Ok(()) // No state change for data in ESTABLISHED
    }

    /// CLOSE_WAIT: Handle ACK (no state transition)
    pub fn on_ack_in_closewait(&mut self) -> Result<(), &'static str> {
        Ok(()) // No state change for ACK in CLOSE_WAIT
    }

    /// TIME_WAIT: Handle retransmitted FIN (no state transition)
    pub fn on_fin_in_timewait(&mut self) -> Result<(), &'static str> {
        Ok(()) // Remain in TIME_WAIT, restart 2MSL timer
    }
}

// ============================================================================
// Reliable Ordered Delivery State Methods
// ============================================================================

impl ReliableOrderedDeliveryState {
    // ------------------------------------------------------------------------
    // Connection Setup (Handshake)
    // ------------------------------------------------------------------------

    /// LISTEN → SYN_RCVD: Initialize sequence numbers from incoming SYN
    pub fn on_syn_in_listen(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Store peer's initial sequence number
        self.irs = seg.seqno;
        self.rcv_nxt = seg.seqno.wrapping_add(1);

        // Generate our initial sequence number (ISS)
        // TODO: Use proper ISS generation per RFC 6528 (currently simplified)
        self.iss = Self::generate_iss();
        self.snd_nxt = self.iss;
        self.snd_lbb = self.iss;
        self.lastack = self.iss;

        Ok(())
    }

    /// Generate Initial Sequence Number (ISS)
    ///
    /// TODO: Implement proper ISS generation per RFC 6528
    /// For now, use a simple counter
    fn generate_iss() -> u32 {
        unsafe {
            static mut ISS_COUNTER: u32 = 0;
            ISS_COUNTER = ISS_COUNTER.wrapping_add(1);
            ISS_COUNTER
        }
    }

    /// SYN_SENT → ESTABLISHED: Process SYN+ACK, update sequence numbers
    pub fn on_synack_in_synsent(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Validate ACK is for our SYN
        if seg.ackno != self.iss.wrapping_add(1) {
            return Err("Invalid ACK number");
        }

        // Store peer's initial sequence number
        self.irs = seg.seqno;
        self.rcv_nxt = seg.seqno.wrapping_add(1);

        // Update our sequence number (SYN is now ACKed)
        self.snd_nxt = self.iss.wrapping_add(1);
        self.lastack = seg.ackno;

        Ok(())
    }

    /// SYN_RCVD → ESTABLISHED: Process ACK of our SYN
    pub fn on_ack_in_synrcvd(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Validate ACK is for our SYN
        if seg.ackno != self.iss.wrapping_add(1) {
            return Err("Invalid ACK number");
        }

        // Update our sequence number (SYN is now ACKed)
        self.snd_nxt = self.iss.wrapping_add(1);
        self.lastack = seg.ackno;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Connection Teardown (Close)
    // ------------------------------------------------------------------------

    /// ESTABLISHED → FIN_WAIT_1: Prepare to send FIN (no rcv_nxt change)
    pub fn on_close_in_established(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Implement - may need to mark FIN pending")
    }

    /// CLOSE_WAIT → LAST_ACK: Prepare to send FIN
    pub fn on_close_in_closewait(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Implement - may need to mark FIN pending")
    }

    /// ESTABLISHED → CLOSE_WAIT: Process FIN, advance rcv_nxt
    pub fn on_fin_in_established(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Validate sequence number
        if seg.seqno != self.rcv_nxt {
            return Err("Invalid sequence number for FIN");
        }

        // FIN consumes one sequence number
        self.rcv_nxt = self.rcv_nxt.wrapping_add(1);

        Ok(())
    }

    /// FIN_WAIT_1 → FIN_WAIT_2: Process ACK of our FIN
    pub fn on_ack_in_finwait1(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Check if this ACKs our FIN
        // FIN consumes one sequence number, so ACK should be snd_nxt + 1
        let expected_ack = self.snd_nxt.wrapping_add(1);
        if seg.ackno != expected_ack {
            return Err("ACK doesn't acknowledge our FIN");
        }

        self.lastack = seg.ackno;

        Ok(())
    }

    /// FIN_WAIT_1 → CLOSING: Process FIN (simultaneous close)
    pub fn on_fin_in_finwait1(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Validate sequence number
        if seg.seqno != self.rcv_nxt {
            return Err("Invalid sequence number for FIN");
        }

        // FIN consumes one sequence number
        self.rcv_nxt = self.rcv_nxt.wrapping_add(1);

        Ok(())
    }

    /// FIN_WAIT_2 → TIME_WAIT: Process FIN
    pub fn on_fin_in_finwait2(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Validate sequence number
        if seg.seqno != self.rcv_nxt {
            return Err("Invalid sequence number for FIN");
        }

        // FIN consumes one sequence number
        self.rcv_nxt = self.rcv_nxt.wrapping_add(1);

        Ok(())
    }

    /// CLOSING → TIME_WAIT: Process ACK of our FIN
    pub fn on_ack_in_closing(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Check if this ACKs our FIN
        // FIN consumes one sequence number, so ACK should be snd_nxt + 1
        let expected_ack = self.snd_nxt.wrapping_add(1);
        if seg.ackno != expected_ack {
            return Err("ACK doesn't acknowledge our FIN");
        }

        self.lastack = seg.ackno;

        Ok(())
    }

    /// LAST_ACK → CLOSED: Process ACK of our FIN
    pub fn on_ack_in_lastack(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Check if this ACKs our FIN
        // FIN consumes one sequence number, so ACK should be snd_nxt + 1
        let expected_ack = self.snd_nxt.wrapping_add(1);
        if seg.ackno != expected_ack {
            return Err("ACK doesn't acknowledge our FIN");
        }

        self.lastack = seg.ackno;

        Ok(())
    }

    /// TIME_WAIT: Process retransmitted FIN (no sequence change)
    pub fn on_fin_in_timewait(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Implement - validate sequence number")
    }

    // ------------------------------------------------------------------------
    // Reset Handling
    // ------------------------------------------------------------------------

    /// ANY → CLOSED: Reset sequence numbers
    pub fn on_rst(&mut self) -> Result<(), &'static str> {
        // Clear sequence numbers
        self.snd_nxt = 0;
        self.rcv_nxt = 0;
        self.lastack = 0;

        Ok(())
    }

    /// ANY → CLOSED: Abort connection
    pub fn on_abort(&mut self) -> Result<(), &'static str> {
        // Clear sequence numbers
        self.snd_nxt = 0;
        self.rcv_nxt = 0;
        self.lastack = 0;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // API-Initiated State Changes
    // ------------------------------------------------------------------------

    /// CLOSED → SYN_SENT: Generate ISS for active open
    pub fn on_connect(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Migrate from control_path::tcp_connect")
    }

    // ------------------------------------------------------------------------
    // Data Path (Future - for ESTABLISHED state)
    // ------------------------------------------------------------------------

    /// ESTABLISHED: Process incoming data segment
    pub fn on_data_in_established(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update rcv_nxt")
    }

    /// ESTABLISHED: Process ACK of our data
    pub fn on_ack_in_established(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update lastack")
    }

    /// CLOSE_WAIT: Process ACK (connection closing but still receiving)
    pub fn on_ack_in_closewait(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update lastack")
    }

    // ------------------------------------------------------------------------
    // Validation Helpers (Read-only)
    // ------------------------------------------------------------------------

    /// Validate sequence number (RFC 793)
    pub fn validate_sequence_number(
        &self,
        seg: &TcpSegment,
        rcv_wnd: u16,
    ) -> bool {
        unimplemented!("TODO: Migrate from control_path - validation logic")
    }

    /// Validate ACK field (RFC 5961)
    pub fn validate_ack(&self, seg: &TcpSegment) -> crate::control_path::AckValidation {
        unimplemented!("TODO: Migrate from control_path - ACK validation")
    }

    /// Validate RST segment (RFC 5961)
    pub fn validate_rst(&self, seg: &TcpSegment, rcv_wnd: u16) -> crate::control_path::RstValidation {
        unimplemented!("TODO: Migrate from control_path - RST validation")
    }
}

// ============================================================================
// Flow Control State Methods
// ============================================================================

impl FlowControlState {
    // ------------------------------------------------------------------------
    // Connection Setup (Handshake)
    // ------------------------------------------------------------------------

    /// LISTEN → SYN_RCVD: Initialize windows from SYN
    pub fn on_syn_in_listen(
        &mut self,
        seg: &TcpSegment,
        _conn_mgmt: &ConnectionManagementState,
    ) -> Result<(), &'static str> {
        // Store peer's advertised window
        self.snd_wnd = seg.wnd;
        self.snd_wnd_max = seg.wnd;

        // Initialize our receive window
        // TODO: Base this on actual buffer size
        self.rcv_wnd = 4096;
        self.rcv_ann_wnd = self.rcv_wnd;

        Ok(())
    }

    /// SYN_SENT → ESTABLISHED: Store peer's advertised window
    pub fn on_synack_in_synsent(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Store peer's advertised window
        self.snd_wnd = seg.wnd;
        self.snd_wnd_max = seg.wnd;

        Ok(())
    }

    /// SYN_RCVD → ESTABLISHED: Update peer's window
    pub fn on_ack_in_synrcvd(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        // Update peer's advertised window
        self.snd_wnd = seg.wnd;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Connection Teardown (No-ops - FC doesn't change on close)
    // ------------------------------------------------------------------------

    /// ESTABLISHED → FIN_WAIT_1: No flow control change
    pub fn on_close_in_established(&mut self) -> Result<(), &'static str> {
        Ok(()) // No window change on FIN
    }

    /// CLOSE_WAIT → LAST_ACK: No flow control change
    pub fn on_close_in_closewait(&mut self) -> Result<(), &'static str> {
        Ok(()) // No window change on FIN
    }

    /// ESTABLISHED → CLOSE_WAIT: No flow control change
    pub fn on_fin_in_established(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No window change on receiving FIN
    }

    /// FIN_WAIT_1 → FIN_WAIT_2: No flow control change
    pub fn on_ack_in_finwait1(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No window change
    }

    /// FIN_WAIT_1 → CLOSING: No flow control change
    pub fn on_fin_in_finwait1(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No window change
    }

    /// FIN_WAIT_2 → TIME_WAIT: No flow control change
    pub fn on_fin_in_finwait2(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No window change
    }

    /// CLOSING → TIME_WAIT: No flow control change
    pub fn on_ack_in_closing(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No window change
    }

    /// LAST_ACK → CLOSED: No flow control change
    pub fn on_ack_in_lastack(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No window change
    }

    /// TIME_WAIT: No flow control change
    pub fn on_fin_in_timewait(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No window change
    }

    // ------------------------------------------------------------------------
    // Reset Handling
    // ------------------------------------------------------------------------

    /// ANY → CLOSED: Clear window state
    pub fn on_rst(&mut self) -> Result<(), &'static str> {
        // Clear window state
        self.snd_wnd = 0;
        self.rcv_wnd = 0;

        Ok(())
    }

    /// ANY → CLOSED: Clear window state
    pub fn on_abort(&mut self) -> Result<(), &'static str> {
        // Clear window state
        self.snd_wnd = 0;
        self.rcv_wnd = 0;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // API-Initiated State Changes
    // ------------------------------------------------------------------------

    /// CLOSED → SYN_SENT: Initialize our receive window for active open
    pub fn on_connect(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Migrate from control_path::tcp_connect")
    }

    // ------------------------------------------------------------------------
    // Data Path (Future - for ESTABLISHED state)
    // ------------------------------------------------------------------------

    /// ESTABLISHED: Update windows based on incoming segment
    pub fn on_data_in_established(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update snd_wnd, rcv_wnd")
    }

    /// ESTABLISHED: Update send window from ACK
    pub fn on_ack_in_established(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update snd_wnd")
    }

    /// CLOSE_WAIT: Update send window from ACK
    pub fn on_ack_in_closewait(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update snd_wnd")
    }
}

// ============================================================================
// Congestion Control State Methods
// ============================================================================

impl CongestionControlState {
    // ------------------------------------------------------------------------
    // Connection Setup (Handshake)
    // ------------------------------------------------------------------------

    /// LISTEN → SYN_RCVD: Initialize cwnd (passive open)
    pub fn on_syn_in_listen(
        &mut self,
        conn_mgmt: &ConnectionManagementState,
    ) -> Result<(), &'static str> {
        // Initialize congestion control
        // RFC 5681: IW = min(4*MSS, max(2*MSS, 4380 bytes))
        let mss = conn_mgmt.mss as u16;
        self.cwnd = core::cmp::min(4 * mss, core::cmp::max(2 * mss, 4380));

        // ssthresh is already initialized to 0xFFFF in TcpConnectionState::new()

        Ok(())
    }

    /// SYN_SENT → ESTABLISHED: Initialize cwnd (active open)
    pub fn on_synack_in_synsent(
        &mut self,
        conn_mgmt: &ConnectionManagementState,
    ) -> Result<(), &'static str> {
        unimplemented!("TODO: Migrate from control_path::process_synack_in_synsent")
    }

    /// SYN_RCVD → ESTABLISHED: No congestion control change
    pub fn on_ack_in_synrcvd(&mut self) -> Result<(), &'static str> {
        Ok(()) // cwnd already initialized in on_syn_in_listen
    }

    // ------------------------------------------------------------------------
    // Connection Teardown (No-ops - CC doesn't change on close)
    // ------------------------------------------------------------------------

    /// ESTABLISHED → FIN_WAIT_1: No congestion control change
    pub fn on_close_in_established(&mut self) -> Result<(), &'static str> {
        Ok(()) // No cwnd change on FIN
    }

    /// CLOSE_WAIT → LAST_ACK: No congestion control change
    pub fn on_close_in_closewait(&mut self) -> Result<(), &'static str> {
        Ok(()) // No cwnd change on FIN
    }

    /// ESTABLISHED → CLOSE_WAIT: No congestion control change
    pub fn on_fin_in_established(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No cwnd change on receiving FIN
    }

    /// FIN_WAIT_1 → FIN_WAIT_2: No congestion control change
    pub fn on_ack_in_finwait1(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No cwnd change
    }

    /// FIN_WAIT_1 → CLOSING: No congestion control change
    pub fn on_fin_in_finwait1(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No cwnd change
    }

    /// FIN_WAIT_2 → TIME_WAIT: No congestion control change
    pub fn on_fin_in_finwait2(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No cwnd change
    }

    /// CLOSING → TIME_WAIT: No congestion control change
    pub fn on_ack_in_closing(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No cwnd change
    }

    /// LAST_ACK → CLOSED: No congestion control change
    pub fn on_ack_in_lastack(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No cwnd change
    }

    /// TIME_WAIT: No congestion control change
    pub fn on_fin_in_timewait(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        Ok(()) // No cwnd change
    }

    // ------------------------------------------------------------------------
    // Reset Handling
    // ------------------------------------------------------------------------

    /// ANY → CLOSED: Reset congestion control state
    pub fn on_rst(&mut self) -> Result<(), &'static str> {
        // Reset congestion control state
        self.cwnd = 0;

        Ok(())
    }

    /// ANY → CLOSED: Reset congestion control state
    pub fn on_abort(&mut self) -> Result<(), &'static str> {
        // Reset congestion control state
        self.cwnd = 0;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // API-Initiated State Changes
    // ------------------------------------------------------------------------

    /// CLOSED → SYN_SENT: Initialize cwnd for active open
    pub fn on_connect(
        &mut self,
        conn_mgmt: &ConnectionManagementState,
    ) -> Result<(), &'static str> {
        unimplemented!("TODO: Migrate from control_path::tcp_connect")
    }

    // ------------------------------------------------------------------------
    // Data Path (Future - for ESTABLISHED state)
    // ------------------------------------------------------------------------

    /// ESTABLISHED: Update cwnd based on ACK (slow start / congestion avoidance)
    pub fn on_ack_in_established(&mut self, seg: &TcpSegment, bytes_acked: u16) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update cwnd based on ACK")
    }

    /// ESTABLISHED: Handle duplicate ACK (fast retransmit)
    pub fn on_dupack_in_established(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - fast retransmit logic")
    }

    /// ESTABLISHED: Handle timeout (congestion event)
    pub fn on_timeout_in_established(&mut self) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - reduce cwnd on timeout")
    }

    /// CLOSE_WAIT: Update cwnd based on ACK
    pub fn on_ack_in_closewait(&mut self, seg: &TcpSegment, bytes_acked: u16) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update cwnd")
    }
}

// ============================================================================
// Demultiplexing State Methods (Stateless)
// ============================================================================

impl DemuxState {
    // Demultiplexing is stateless - uses 4-tuple from ConnectionManagementState
    // No methods needed
}
