//! Connection Management Component
//!
//! This component owns the TCP state machine and all connection lifecycle data.

use crate::ffi;
use crate::state::TcpState;

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

impl ConnectionManagementState {
    pub fn new() -> Self {
        Self {
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
        }
    }

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
        if self.state != TcpState::Closed {
            return Err("Can only bind in CLOSED state");
        }

        if local_port == 0 {
            return Err("Port 0 not yet supported - provide explicit port");
        }

        self.local_ip = local_ip;
        self.local_port = local_port;
        Ok(local_port)
    }

    /// CLOSED → LISTEN: Start listening for connections
    pub fn on_listen(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Closed {
            return Err("Can only listen from CLOSED state");
        }

        if self.local_port == 0 {
            return Err("Must bind to port before listening");
        }

        self.state = TcpState::Listen;
        Ok(())
    }

    /// CLOSED → SYN_SENT: Initiate active connection
    pub fn on_connect(
        &mut self,
        remote_ip: ffi::ip_addr_t,
        remote_port: u16,
    ) -> Result<(), &'static str> {
        if self.state != TcpState::Closed {
            return Err("Can only connect from CLOSED state");
        }

        // Store remote endpoint
        self.remote_ip = remote_ip;
        self.remote_port = remote_port;

        // Transition to SYN_SENT
        self.state = TcpState::SynSent;

        Ok(())
    }

    /// Initiate graceful close from various states
    /// Returns: Ok(true) if FIN should be sent, Ok(false) if already closing/closed
    pub fn on_close(&mut self) -> Result<bool, &'static str> {
        match self.state {
            TcpState::Closed => Ok(false),
            TcpState::Listen => {
                self.state = TcpState::Closed;
                Ok(false)
            }
            TcpState::SynSent | TcpState::SynRcvd => {
                self.state = TcpState::Closed;
                Ok(false)
            }
            TcpState::Established => {
                self.state = TcpState::FinWait1;
                Ok(true)
            }
            TcpState::CloseWait => {
                self.state = TcpState::LastAck;
                Ok(true)
            }
            _ => {
                // Already closing (FinWait1, FinWait2, Closing, LastAck, TimeWait)
                Ok(false)
            }
        }
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
