//! TCP Control Path
//!
//! Handles connection setup, teardown, and state transitions.
//! This is the ONLY component allowed to write to all state.

use crate::state::{TcpConnectionState, TcpState};
use crate::ffi;
use crate::tcp_proto;

/// TCP Flags from the header
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
