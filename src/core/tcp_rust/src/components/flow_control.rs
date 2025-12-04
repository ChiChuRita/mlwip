//! Flow Control Component
//!
//! Manages receive and send windows.

use crate::components::ConnectionManagementState;
use crate::tcp_types::TcpSegment;

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

impl FlowControlState {
    pub fn new() -> Self {
        Self {
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
        }
    }

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
        // Initialize our receive window
        self.rcv_wnd = 4096;
        self.rcv_ann_wnd = self.rcv_wnd;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Data Path (Future - for ESTABLISHED state)
    // ------------------------------------------------------------------------

    /// ESTABLISHED: Update windows based on incoming segment
    pub fn on_data_in_established(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update snd_wnd, rcv_wnd")
    }

    /// ESTABLISHED: Update send window from ACK
    pub fn on_ack_in_established(&mut self, _seg: &TcpSegment, _bytes_acked: u16) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update snd_wnd")
    }

    /// CLOSE_WAIT: Update send window from ACK
    pub fn on_ack_in_closewait(&mut self, _seg: &TcpSegment, _bytes_acked: u16) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update snd_wnd")
    }
}
