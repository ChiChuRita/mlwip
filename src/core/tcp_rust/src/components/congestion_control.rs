//! Congestion Control Component
//!
//! Manages congestion window and slow start threshold.

use crate::components::ConnectionManagementState;
use crate::tcp_types::TcpSegment;

/// Congestion Control State
///
/// Manages congestion window and slow start threshold.
/// Only CC event handlers can write to this state.
pub struct CongestionControlState {
    pub cwnd: u16,       // Congestion Window
    pub ssthresh: u16,   // Slow Start Threshold
}

impl CongestionControlState {
    pub fn new() -> Self {
        Self {
            cwnd: 0,
            ssthresh: 0xFFFF,   // Initial ssthresh is large
        }
    }

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
        // RFC 5681: IW = min(4*MSS, max(2*MSS, 4380 bytes))
        let mss = conn_mgmt.mss as u16;
        self.cwnd = core::cmp::min(4 * mss, core::cmp::max(2 * mss, 4380));
        Ok(())
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
        // Initialize congestion window to 1 MSS for active open
        // (will be expanded after SYN+ACK received per RFC 5681)
        let mss = conn_mgmt.mss as u16;
        self.cwnd = mss;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Data Path (Future - for ESTABLISHED state)
    // ------------------------------------------------------------------------

    /// ESTABLISHED: Update cwnd based on ACK (slow start / congestion avoidance)
    pub fn on_ack_in_established(&mut self, _seg: &TcpSegment, _bytes_acked: u16) -> Result<(), &'static str> {
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
    pub fn on_ack_in_closewait(&mut self, _seg: &TcpSegment, _bytes_acked: u16) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update cwnd")
    }
}
