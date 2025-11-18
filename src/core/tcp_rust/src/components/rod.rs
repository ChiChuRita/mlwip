//! Reliable Ordered Delivery Component
//!
//! Handles sequence numbers, ACKs, retransmissions, and buffering.

use crate::tcp_types::TcpSegment;

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

impl ReliableOrderedDeliveryState {
    pub fn new() -> Self {
        Self {
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
        }
    }

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
    pub fn on_fin_in_timewait(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
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
    pub fn on_data_in_established(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update rcv_nxt")
    }

    /// ESTABLISHED: Process ACK of our data
    pub fn on_ack_in_established(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update lastack")
    }

    /// CLOSE_WAIT: Process ACK (connection closing but still receiving)
    pub fn on_ack_in_closewait(&mut self, _seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO: Future data path - update lastack")
    }

    // ------------------------------------------------------------------------
    // Validation Helpers (Read-only)
    // ------------------------------------------------------------------------

    /// Validate sequence number (RFC 793)
    pub fn validate_sequence_number(
        &self,
        _seg: &TcpSegment,
        _rcv_wnd: u16,
    ) -> bool {
        unimplemented!("TODO: Migrate from control_path - validation logic")
    }

    /// Validate ACK field (RFC 5961)
    pub fn validate_ack(&self, _seg: &TcpSegment) -> crate::tcp_types::AckValidation {
        unimplemented!("TODO: Migrate from control_path - ACK validation")
    }

    /// Validate RST segment (RFC 5961)
    pub fn validate_rst(&self, _seg: &TcpSegment, _rcv_wnd: u16) -> crate::tcp_types::RstValidation {
        unimplemented!("TODO: Migrate from control_path - RST validation")
    }
}
