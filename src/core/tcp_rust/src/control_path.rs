//! TCP Control Path
//!
//! Handles connection setup, teardown, and state transitions.
//! This is the ONLY component allowed to write to all state.

use crate::state::{TcpConnectionState, TcpState};
use crate::ffi;
use crate::tcp_proto;

/// TCP Flags from the header
#[derive(Debug, Clone, Copy)]
pub struct TcpFlags {
    pub fin: bool,
    pub syn: bool,
    pub rst: bool,
    pub psh: bool,
    pub ack: bool,
    pub urg: bool,
}

impl TcpFlags {
    pub fn from_tcphdr(flags: u8) -> Self {
        Self {
            fin: (flags & tcp_proto::TCP_FIN) != 0,
            syn: (flags & tcp_proto::TCP_SYN) != 0,
            rst: (flags & tcp_proto::TCP_RST) != 0,
            psh: (flags & tcp_proto::TCP_PSH) != 0,
            ack: (flags & tcp_proto::TCP_ACK) != 0,
            urg: (flags & tcp_proto::TCP_URG) != 0,
        }
    }
}

/// Parsed TCP segment information
pub struct TcpSegment {
    pub seqno: u32,
    pub ackno: u32,
    pub flags: TcpFlags,
    pub wnd: u16,
    pub tcphdr_len: u16,
    pub payload_len: u16,
}

/// RST validation result (RFC 5961)
#[derive(Debug, PartialEq)]
pub enum RstValidation {
    Valid,
    Invalid,
    Challenge,  // Tests use Challenge instead of ChallengeAck
}

/// ACK validation result (RFC 5961)
#[derive(Debug, PartialEq)]
pub enum AckValidation {
    Valid,
    Invalid,
    Duplicate,
    Future,  // ACK for data not yet sent
    Old,     // ACK for already acknowledged data
}

/// Action to take after processing input
#[derive(Debug, PartialEq)]
pub enum InputAction {
    Accept,
    Drop,
    SendAck,
    SendSynAck,  // For handshake
    SendChallengeAck,
    SendRst,
    Abort,  // For aborting connection
}

/// Control Path: Handles handshake state transitions
///
/// This implements the TCP 3-way handshake:
/// - Passive open: CLOSED -> LISTEN -> SYN_RCVD -> ESTABLISHED
/// - Active open: CLOSED -> SYN_SENT -> ESTABLISHED
pub struct ControlPath;

impl ControlPath {
    /// Process a SYN segment in LISTEN state
    ///
    /// Transition: LISTEN -> SYN_RCVD
    pub fn process_syn_in_listen(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
        remote_ip: ffi::ip_addr_t,
        remote_port: u16,
    ) -> Result<(), &'static str> {
        // Validate we're in LISTEN state
        if state.conn_mgmt.state != TcpState::Listen {
            return Err("Not in LISTEN state");
        }

        // Store remote endpoint
        state.conn_mgmt.remote_ip = remote_ip;
        state.conn_mgmt.remote_port = remote_port;

        // Store peer's initial sequence number
        state.rod.irs = seg.seqno;
        state.rod.rcv_nxt = seg.seqno.wrapping_add(1);

        // Generate our initial sequence number (ISS)
        // TODO: Use proper ISS generation (currently simplified)
        state.rod.iss = unsafe { Self::generate_iss() };
        state.rod.snd_nxt = state.rod.iss;
        state.rod.snd_lbb = state.rod.iss;
        state.rod.lastack = state.rod.iss;

        // Store peer's window
        state.flow_ctrl.snd_wnd = seg.wnd;
        state.flow_ctrl.snd_wnd_max = seg.wnd;

        // Initialize our receive window
        // TODO: Base this on actual buffer size
        state.flow_ctrl.rcv_wnd = 4096;
        state.flow_ctrl.rcv_ann_wnd = state.flow_ctrl.rcv_wnd;

        // Initialize congestion control
        // RFC 5681: IW = min(4*MSS, max(2*MSS, 4380 bytes))
        let mss = state.conn_mgmt.mss as u16;
        state.cong_ctrl.cwnd = core::cmp::min(4 * mss, core::cmp::max(2 * mss, 4380));

        // Transition to SYN_RCVD
        state.conn_mgmt.state = TcpState::SynRcvd;

        Ok(())
    }

    /// Process a SYN+ACK segment in SYN_SENT state
    ///
    /// Transition: SYN_SENT -> ESTABLISHED
    pub fn process_synack_in_synsent(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Validate we're in SYN_SENT state
        if state.conn_mgmt.state != TcpState::SynSent {
            return Err("Not in SYN_SENT state");
        }

        // Validate ACK is for our SYN
        if seg.ackno != state.rod.iss.wrapping_add(1) {
            return Err("Invalid ACK number");
        }

        // Store peer's initial sequence number
        state.rod.irs = seg.seqno;
        state.rod.rcv_nxt = seg.seqno.wrapping_add(1);

        // Update our sequence number (SYN is now ACKed)
        state.rod.snd_nxt = state.rod.iss.wrapping_add(1);
        state.rod.lastack = seg.ackno;

        // Store peer's window
        state.flow_ctrl.snd_wnd = seg.wnd;
        state.flow_ctrl.snd_wnd_max = seg.wnd;

        // Transition to ESTABLISHED
        state.conn_mgmt.state = TcpState::Established;

        Ok(())
    }

    /// Process an ACK segment in SYN_RCVD state
    ///
    /// Transition: SYN_RCVD -> ESTABLISHED
    pub fn process_ack_in_synrcvd(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Validate we're in SYN_RCVD state
        if state.conn_mgmt.state != TcpState::SynRcvd {
            return Err("Not in SYN_RCVD state");
        }

        // Validate ACK is for our SYN
        if seg.ackno != state.rod.iss.wrapping_add(1) {
            return Err("Invalid ACK number");
        }

        // Update our sequence number (SYN is now ACKed)
        state.rod.snd_nxt = state.rod.iss.wrapping_add(1);
        state.rod.lastack = seg.ackno;

        // Update peer's window
        state.flow_ctrl.snd_wnd = seg.wnd;

        // Transition to ESTABLISHED
        state.conn_mgmt.state = TcpState::Established;

        Ok(())
    }

    /// Handle RST (connection reset)
    ///
    /// Transition: ANY -> CLOSED
    pub fn process_rst(state: &mut TcpConnectionState) {
        state.conn_mgmt.state = TcpState::Closed;
        // TODO: Clean up resources
    }

    // ========================================================================
    // Validation Functions (RFC 5961 Security)
    // ========================================================================

    /// Validate sequence number for incoming segment
    ///
    /// Implements RFC 793 and RFC 5961 sequence number validation
    pub fn validate_sequence_number(
        state: &TcpConnectionState,
        seg: &TcpSegment,
    ) -> bool {
        let seqno = seg.seqno;
        let rcv_nxt = state.rod.rcv_nxt;
        let rcv_wnd = state.flow_ctrl.rcv_wnd;

        // Special case: zero window
        if rcv_wnd == 0 {
            return seqno == rcv_nxt;
        }

        // Check if sequence number is within receive window
        // Valid if: RCV.NXT <= SEG.SEQ < RCV.NXT + RCV.WND
        let seg_end = seqno.wrapping_add(seg.payload_len as u32);

        // Check if segment overlaps with receive window
        let seq_acceptable = Self::seq_in_window(seqno, rcv_nxt, rcv_wnd) ||
                            (seg.payload_len > 0 && Self::seq_in_window(seg_end.wrapping_sub(1), rcv_nxt, rcv_wnd));

        seq_acceptable
    }

    /// Check if a sequence number is within the window
    fn seq_in_window(seq: u32, rcv_nxt: u32, rcv_wnd: u16) -> bool {
        let diff = seq.wrapping_sub(rcv_nxt);
        diff < rcv_wnd as u32
    }

    /// Validate RST segment (RFC 5961)
    ///
    /// Returns whether RST should be accepted, rejected, or trigger challenge ACK
    pub fn validate_rst(
        state: &TcpConnectionState,
        seg: &TcpSegment,
    ) -> RstValidation {
        let seqno = seg.seqno;
        let rcv_nxt = state.rod.rcv_nxt;
        
        // Check if sequence number is in window
        if Self::validate_sequence_number(state, seg) {
            // In window - accept the RST
            RstValidation::Valid
        } else {
            // Out of window - send challenge ACK per RFC 5961
            RstValidation::Challenge
        }
    }

    /// Validate ACK field (RFC 5961)
    ///
    /// Returns whether ACK is acceptable
    pub fn validate_ack(
        state: &TcpConnectionState,
        seg: &TcpSegment,
    ) -> AckValidation {
        let ackno = seg.ackno;
        let snd_una = state.rod.lastack;
        let snd_nxt = state.rod.snd_nxt;

        // ACK must be in range: SND.UNA < SEG.ACK <= SND.NXT
        if ackno == snd_una {
            AckValidation::Duplicate
        } else if Self::seq_lt(snd_una, ackno) && Self::seq_leq(ackno, snd_nxt) {
            AckValidation::Valid
        } else if Self::seq_gt(ackno, snd_nxt) {
            // RFC 5961: ACK of unsent data
            AckValidation::Future
        } else {
            // ACK for already acknowledged data
            AckValidation::Old
        }
    }

    // ========================================================================
    // Connection Teardown (FIN Handling)
    // ========================================================================

    /// Initiate active close
    ///
    /// Transition: ESTABLISHED -> FIN_WAIT_1
    ///             CLOSE_WAIT -> LAST_ACK
    /// Returns: Ok(true) if FIN should be sent, Ok(false) otherwise
    /// 
    /// Note: This function does NOT increment snd_nxt. The FIN transmission
    /// (handled by output layer) will increment snd_nxt when the FIN is actually sent.
    pub fn initiate_close(state: &mut TcpConnectionState) -> Result<bool, &'static str> {
        match state.conn_mgmt.state {
            TcpState::Established => {
                // Transition to FIN_WAIT_1
                state.conn_mgmt.state = TcpState::FinWait1;
                Ok(true) // Send FIN
            }
            TcpState::CloseWait => {
                // Transition to LAST_ACK
                state.conn_mgmt.state = TcpState::LastAck;
                Ok(true) // Send FIN
            }
            _ => Err("Cannot close from this state"),
        }
    }

    /// Process FIN in ESTABLISHED state
    ///
    /// Transition: ESTABLISHED -> CLOSE_WAIT
    pub fn process_fin_in_established(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::Established {
            return Err("Not in ESTABLISHED state");
        }

        // Validate sequence number
        if seg.seqno != state.rod.rcv_nxt {
            return Err("Invalid sequence number for FIN");
        }

        // FIN consumes one sequence number
        state.rod.rcv_nxt = state.rod.rcv_nxt.wrapping_add(1);

        // Transition to CLOSE_WAIT
        state.conn_mgmt.state = TcpState::CloseWait;

        Ok(())
    }

    /// Process ACK in FIN_WAIT_1 state
    ///
    /// Transition: FIN_WAIT_1 -> FIN_WAIT_2 (if ACKing our FIN)
    pub fn process_ack_in_finwait1(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::FinWait1 {
            return Err("Not in FIN_WAIT_1 state");
        }

        // Check if this ACKs our FIN
        // FIN consumes one sequence number, so ACK should be snd_nxt + 1
        let expected_ack = state.rod.snd_nxt.wrapping_add(1);
        if seg.ackno == expected_ack {
            state.rod.lastack = seg.ackno;
            state.conn_mgmt.state = TcpState::FinWait2;
            Ok(())
        } else {
            Err("ACK doesn't acknowledge our FIN")
        }
    }

    /// Process FIN in FIN_WAIT_1 state (simultaneous close)
    ///
    /// Transition: FIN_WAIT_1 -> CLOSING
    pub fn process_fin_in_finwait1(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::FinWait1 {
            return Err("Not in FIN_WAIT_1 state");
        }

        // Validate sequence number
        if seg.seqno != state.rod.rcv_nxt {
            return Err("Invalid sequence number for FIN");
        }

        // FIN consumes one sequence number
        state.rod.rcv_nxt = state.rod.rcv_nxt.wrapping_add(1);

        // Transition to CLOSING (simultaneous close)
        state.conn_mgmt.state = TcpState::Closing;

        Ok(())
    }

    /// Process FIN in FIN_WAIT_2 state
    ///
    /// Transition: FIN_WAIT_2 -> TIME_WAIT
    pub fn process_fin_in_finwait2(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::FinWait2 {
            return Err("Not in FIN_WAIT_2 state");
        }

        // Validate sequence number
        if seg.seqno != state.rod.rcv_nxt {
            return Err("Invalid sequence number for FIN");
        }

        // FIN consumes one sequence number
        state.rod.rcv_nxt = state.rod.rcv_nxt.wrapping_add(1);

        // Transition to TIME_WAIT
        state.conn_mgmt.state = TcpState::TimeWait;

        Ok(())
    }

    /// Process ACK in CLOSING state
    ///
    /// Transition: CLOSING -> TIME_WAIT
    pub fn process_ack_in_closing(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::Closing {
            return Err("Not in CLOSING state");
        }

        // Check if this ACKs our FIN
        // FIN consumes one sequence number, so ACK should be snd_nxt + 1
        let expected_ack = state.rod.snd_nxt.wrapping_add(1);
        if seg.ackno == expected_ack {
            state.rod.lastack = seg.ackno;
            state.conn_mgmt.state = TcpState::TimeWait;
            Ok(())
        } else {
            Err("ACK doesn't acknowledge our FIN")
        }
    }

    /// Process ACK in LAST_ACK state
    ///
    /// Transition: LAST_ACK -> CLOSED
    pub fn process_ack_in_lastack(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::LastAck {
            return Err("Not in LAST_ACK state");
        }

        // Check if this ACKs our FIN
        // FIN consumes one sequence number, so ACK should be snd_nxt + 1
        let expected_ack = state.rod.snd_nxt.wrapping_add(1);
        if seg.ackno == expected_ack {
            state.rod.lastack = seg.ackno;
            state.conn_mgmt.state = TcpState::Closed;
            Ok(())
        } else {
            Err("ACK doesn't acknowledge our FIN")
        }
    }

    // ========================================================================
    // API Functions
    // ========================================================================

    /// Bind to a local IP and port
    ///
    /// Transition: CLOSED -> CLOSED (with IP and port assigned)
    /// Returns: Ok(port) on success
    pub fn tcp_bind(
        state: &mut TcpConnectionState,
        local_ip: ffi::ip_addr_t,
        local_port: u16,
    ) -> Result<u16, &'static str> {
        if state.conn_mgmt.state != TcpState::Closed {
            return Err("Can only bind in CLOSED state");
        }

        if local_port == 0 {
            return Err("Port 0 not yet supported - provide explicit port");
        }

        state.conn_mgmt.local_ip = local_ip;
        state.conn_mgmt.local_port = local_port;
        Ok(local_port)
    }

    /// Start listening for connections
    ///
    /// Transition: CLOSED -> LISTEN
    pub fn tcp_listen(state: &mut TcpConnectionState) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::Closed {
            return Err("Can only listen from CLOSED state");
        }

        // In lwIP, tcp_listen can be called without explicit bind if local_port is set
        // The port must be set either via tcp_bind or by the PCB creation
        if state.conn_mgmt.local_port == 0 {
            return Err("Must bind to port before listening");
        }

        state.conn_mgmt.state = TcpState::Listen;
        Ok(())
    }

    /// Initiate active connection
    ///
    /// Transition: CLOSED -> SYN_SENT
    /// Note: SYN will be sent by output layer, which increments snd_nxt
    pub fn tcp_connect(
        state: &mut TcpConnectionState,
        remote_ip: ffi::ip_addr_t,
        remote_port: u16,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::Closed {
            return Err("Can only connect from CLOSED state");
        }

        // Store remote endpoint
        state.conn_mgmt.remote_ip = remote_ip;
        state.conn_mgmt.remote_port = remote_port;

        // Generate our ISS
        state.rod.iss = unsafe { Self::generate_iss() };
        state.rod.snd_nxt = state.rod.iss;
        state.rod.snd_lbb = state.rod.iss.wrapping_sub(1);
        state.rod.lastack = state.rod.iss.wrapping_sub(1);

        // Initialize our receive window
        state.flow_ctrl.rcv_wnd = 4096;
        state.flow_ctrl.rcv_ann_wnd = state.flow_ctrl.rcv_wnd;
        
        // Initialize congestion window
        let mss = state.conn_mgmt.mss as u16;
        state.cong_ctrl.cwnd = mss;

        // Transition to SYN_SENT
        state.conn_mgmt.state = TcpState::SynSent;

        Ok(())
    }

    /// Abort connection (send RST)
    ///
    /// Transition: ANY -> CLOSED
    /// Returns: Ok(true) if RST should be sent, Ok(false) otherwise
    pub fn tcp_abort(state: &mut TcpConnectionState) -> Result<bool, &'static str> {
        let should_send_rst = match state.conn_mgmt.state {
            TcpState::Closed | TcpState::Listen => false,
            _ => true,
        };
        
        // Close immediately
        state.conn_mgmt.state = TcpState::Closed;
        Ok(should_send_rst)
    }

    // ========================================================================
    // Input Dispatcher
    // ========================================================================

    /// Main input processing dispatcher
    ///
    /// Routes segments to appropriate state-specific handlers
    pub fn tcp_input(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
        remote_ip: ffi::ip_addr_t,
        remote_port: u16,
    ) -> Result<InputAction, &'static str> {
        // Handle RST first (in any state)
        if seg.flags.rst {
            return Self::handle_rst(state, seg);
        }

        // Dispatch based on current state
        match state.conn_mgmt.state {
            TcpState::Closed => Self::input_closed(state, seg),
            TcpState::Listen => Self::input_listen(state, seg, remote_ip, remote_port),
            TcpState::SynSent => Self::input_synsent(state, seg),
            TcpState::SynRcvd => Self::input_synrcvd(state, seg),
            TcpState::Established => Self::input_established(state, seg),
            TcpState::FinWait1 => Self::input_finwait1(state, seg),
            TcpState::FinWait2 => Self::input_finwait2(state, seg),
            TcpState::CloseWait => Self::input_closewait(state, seg),
            TcpState::Closing => Self::input_closing(state, seg),
            TcpState::LastAck => Self::input_lastack(state, seg),
            TcpState::TimeWait => Self::input_timewait(state, seg),
        }
    }

    /// Handle RST in any state
    fn handle_rst(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        match Self::validate_rst(state, seg) {
            RstValidation::Valid => {
                Self::process_rst(state);
                Ok(InputAction::Abort)
            }
            RstValidation::Challenge => Ok(InputAction::SendChallengeAck),
            RstValidation::Invalid => Ok(InputAction::Drop),
        }
    }

    /// Input processing for CLOSED state
    fn input_closed(
        _state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // RFC 793: All segments are rejected in CLOSED state
        // Send RST if not already RST
        if !seg.flags.rst {
            Ok(InputAction::SendRst)
        } else {
            Ok(InputAction::Drop)
        }
    }

    /// Input processing for LISTEN state
    fn input_listen(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
        remote_ip: ffi::ip_addr_t,
        remote_port: u16,
    ) -> Result<InputAction, &'static str> {
        // Only accept SYN in LISTEN state
        if seg.flags.syn && !seg.flags.ack {
            // Process the SYN and transition to SYN_RCVD
            Self::process_syn_in_listen(state, seg, remote_ip, remote_port)?;
            Ok(InputAction::SendSynAck)
        } else {
            Ok(InputAction::SendRst)
        }
    }

    /// Input processing for SYN_SENT state
    fn input_synsent(
        _state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Expecting SYN+ACK
        if seg.flags.syn && seg.flags.ack {
            Ok(InputAction::Accept)
        } else if seg.flags.syn {
            // Simultaneous open (SYN without ACK)
            Ok(InputAction::Accept)
        } else {
            Ok(InputAction::Drop)
        }
    }

    /// Input processing for SYN_RCVD state
    fn input_synrcvd(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Expecting ACK of our SYN
        if seg.flags.ack {
            Ok(InputAction::Accept)
        } else {
            Ok(InputAction::Drop)
        }
    }

    /// Input processing for ESTABLISHED state
    fn input_established(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Validate ACK if present
        if seg.flags.ack {
            match Self::validate_ack(state, seg) {
                AckValidation::Valid | AckValidation::Duplicate => {
                    // Process normally
                }
                AckValidation::Future => {
                    // RFC 5961: ACK of unsent data - send challenge ACK
                    return Ok(InputAction::SendChallengeAck);
                }
                AckValidation::Old | AckValidation::Invalid => {
                    return Ok(InputAction::Drop);
                }
            }
        }

        // Check for FIN
        if seg.flags.fin {
            // Process FIN and transition to CLOSE_WAIT
            Self::process_fin_in_established(state, seg)?;
            // FIN received - must send ACK
            Ok(InputAction::SendAck)
        } else {
            Ok(InputAction::Accept)
        }
    }

    /// Input processing for FIN_WAIT_1 state
    fn input_finwait1(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Can receive ACK of our FIN or peer's FIN (simultaneous close)
        if seg.flags.ack || seg.flags.fin {
            Ok(InputAction::Accept)
        } else {
            Ok(InputAction::Drop)
        }
    }

    /// Input processing for FIN_WAIT_2 state
    fn input_finwait2(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Expecting FIN from peer
        if seg.flags.fin {
            Ok(InputAction::Accept)
        } else {
            Ok(InputAction::Accept) // Can still receive data
        }
    }

    /// Input processing for CLOSE_WAIT state
    fn input_closewait(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Can receive ACKs for data we sent
        Ok(InputAction::Accept)
    }

    /// Input processing for CLOSING state
    fn input_closing(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Expecting ACK of our FIN
        if seg.flags.ack {
            Ok(InputAction::Accept)
        } else {
            Ok(InputAction::Drop)
        }
    }

    /// Input processing for LAST_ACK state
    fn input_lastack(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Expecting ACK of our FIN
        if seg.flags.ack {
            Ok(InputAction::Accept)
        } else {
            Ok(InputAction::Drop)
        }
    }

    /// Input processing for TIME_WAIT state
    fn input_timewait(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !Self::validate_sequence_number(state, seg) {
            return Ok(InputAction::Drop);
        }

        // Can receive retransmitted FIN
        if seg.flags.fin {
            Ok(InputAction::SendAck) // ACK the FIN again
        } else {
            Ok(InputAction::Accept)
        }
    }

    // ========================================================================
    // Sequence Number Comparison (RFC 793)
    // ========================================================================

    /// Sequence number less than (handles wraparound)
    fn seq_lt(a: u32, b: u32) -> bool {
        (a.wrapping_sub(b) as i32) < 0
    }

    /// Sequence number less than or equal (handles wraparound)
    fn seq_leq(a: u32, b: u32) -> bool {
        (a.wrapping_sub(b) as i32) <= 0
    }

    /// Sequence number greater than (handles wraparound)
    fn seq_gt(a: u32, b: u32) -> bool {
        (a.wrapping_sub(b) as i32) > 0
    }

    /// Generate Initial Sequence Number (ISS)
    ///
    /// TODO: Implement proper ISS generation per RFC 6528
    /// For now, use a simple counter
    unsafe fn generate_iss() -> u32 {
        static mut ISS_COUNTER: u32 = 0;
        ISS_COUNTER = ISS_COUNTER.wrapping_add(1);
        ISS_COUNTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syn_in_listen() {
        let mut state = TcpConnectionState::new();
        state.conn_mgmt.state = TcpState::Listen;
        state.conn_mgmt.mss = 1460;

        let seg = TcpSegment {
            seqno: 1000,
            ackno: 0,
            flags: TcpFlags {
                syn: true,
                ack: false,
                fin: false,
                rst: false,
                psh: false,
                urg: false,
            },
            wnd: 8192,
            tcphdr_len: 20,
            payload_len: 0,
        };

        let remote_ip = unsafe { core::mem::zeroed() };
        let result = ControlPath::process_syn_in_listen(&mut state, &seg, remote_ip, 12345);

        assert!(result.is_ok());
        assert_eq!(state.conn_mgmt.state, TcpState::SynRcvd);
        assert_eq!(state.rod.irs, 1000);
        assert_eq!(state.rod.rcv_nxt, 1001);
        assert_eq!(state.flow_ctrl.snd_wnd, 8192);
    }

    #[test]
    fn test_ack_in_synrcvd() {
        let mut state = TcpConnectionState::new();
        state.conn_mgmt.state = TcpState::SynRcvd;
        state.rod.iss = 5000;

        let seg = TcpSegment {
            seqno: 1001,
            ackno: 5001,
            flags: TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: false,
                urg: false,
            },
            wnd: 8192,
            tcphdr_len: 20,
            payload_len: 0,
        };

        let result = ControlPath::process_ack_in_synrcvd(&mut state, &seg);

        assert!(result.is_ok());
        assert_eq!(state.conn_mgmt.state, TcpState::Established);
        assert_eq!(state.rod.lastack, 5001);
    }
}
