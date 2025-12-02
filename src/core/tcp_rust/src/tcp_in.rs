//! TCP Packet Reception (RX Path)
//!
//! Handles incoming TCP segments and dispatches to appropriate handlers.
//! For handshake: processes SYN, SYN+ACK, and ACK segments.

use crate::state::TcpConnectionState;
use crate::tcp_types::{TcpSegment, TcpFlags};
use crate::ffi;
use crate::tcp_proto;

/// TCP RX Path
///
/// Processes incoming segments and updates state accordingly.
/// During handshake, this primarily invokes control path handlers.
pub struct TcpRx;

impl TcpRx {
    /// Process an incoming TCP segment
    ///
    /// This is the main entry point for packet reception.
    /// It parses the TCP header and dispatches to the appropriate handler.
    pub unsafe fn process_segment(
        state: &mut TcpConnectionState,
        p: *mut ffi::pbuf,
        src_ip: &ffi::ip_addr_t,
        dest_ip: &ffi::ip_addr_t,
    ) -> Result<(), &'static str> {
        // Null check
        if p.is_null() {
            return Err("Null pbuf");
        }

        // Parse TCP header
        let seg = Self::parse_tcp_header(p)?;

        // Debug output
        #[cfg(feature = "debug")]
        {
            // TODO: Add debug logging
        }

        // Dispatch based on current state
        match state.conn_mgmt.state {
            crate::state::TcpState::Listen => {
                Self::process_listen(state, &seg, *src_ip)
            }
            crate::state::TcpState::SynSent => {
                Self::process_synsent(state, &seg)
            }
            crate::state::TcpState::SynRcvd => {
                Self::process_synrcvd(state, &seg)
            }
            crate::state::TcpState::Established => {
                Self::process_established(state, &seg)
            }
            crate::state::TcpState::FinWait1 => {
                Self::process_finwait1(state, &seg)
            }
            crate::state::TcpState::FinWait2 => {
                Self::process_finwait2(state, &seg)
            }
            crate::state::TcpState::CloseWait => {
                Self::process_closewait(state, &seg)
            }
            crate::state::TcpState::Closing => {
                Self::process_closing(state, &seg)
            }
            crate::state::TcpState::LastAck => {
                Self::process_lastack(state, &seg)
            }
            crate::state::TcpState::TimeWait => {
                Self::process_timewait(state, &seg)
            }
            crate::state::TcpState::Closed => {
                Err("Connection is closed")
            }
        }
    }

    /// Parse TCP header from pbuf
    unsafe fn parse_tcp_header(p: *mut ffi::pbuf) -> Result<TcpSegment, &'static str> {
        let pbuf = &*p;

        // Ensure we have at least a TCP header (20 bytes minimum)
        if pbuf.len < 20 {
            return Err("Packet too short for TCP header");
        }

        // Cast payload to TCP header
        let tcphdr = pbuf.payload as *const tcp_proto::TcpHdr;
        if tcphdr.is_null() {
            return Err("Null TCP header");
        }

        let hdr = &*tcphdr;

        // Extract fields (convert from network byte order)
        let seqno = hdr.sequence_number();
        let ackno = hdr.ack_number();
        let wnd = hdr.window();

        // Extract flags from the combined field
        let flags_byte = hdr.flags();
        let flags = TcpFlags::from_tcphdr(flags_byte);

        // Calculate header length (in bytes)
        let tcphdr_len = hdr.hdrlen_bytes() as u16;

        // Calculate payload length
        let payload_len = if pbuf.tot_len > tcphdr_len {
            pbuf.tot_len - tcphdr_len
        } else {
            0
        };

        Ok(TcpSegment {
            seqno,
            ackno,
            flags,
            wnd,
            tcphdr_len,
            payload_len,
        })
    }

    /// Process segment in LISTEN state
    fn process_listen(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
        remote_ip: ffi::ip_addr_t,
    ) -> Result<(), &'static str> {
        // In LISTEN state, we only care about SYN
        if seg.flags.rst {
            // Ignore RST in LISTEN
            return Ok(());
        }

        if seg.flags.ack {
            // ACK without SYN in LISTEN is invalid
            // TODO: Send RST
            return Err("Unexpected ACK in LISTEN");
        }

        if seg.flags.syn {
            // Valid SYN - initiate passive open
            // TODO: Extract remote port from actual packet
            let remote_port = state.conn_mgmt.remote_port; // Placeholder

            // NEW APPROACH: Call component methods instead of control path
            // Each component handles its own state updates

            // 1. ROD: Initialize sequence numbers
            state.rod.on_syn_in_listen(seg)?;

            // 2. Flow Control: Initialize windows
            state.flow_ctrl.on_syn_in_listen(seg, &state.conn_mgmt)?;

            // 3. Congestion Control: Initialize cwnd
            state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;

            // 4. Connection Management: Store endpoint and transition state
            state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;

            // Now we need to send SYN+ACK
            // This will be handled by the TX path
            return Ok(());
        }

        // No SYN, no ACK, nothing useful
        Err("Invalid segment in LISTEN")
    }

    /// Process segment in SYN_SENT state
    fn process_synsent(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            // Call component methods for RST
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // Check for SYN+ACK
        if seg.flags.syn && seg.flags.ack {
            // Call component methods for SYN+ACK in SYN_SENT
            state.rod.on_synack_in_synsent(seg)?;
            state.flow_ctrl.on_synack_in_synsent(seg)?;
            state.cong_ctrl.on_synack_in_synsent(&state.conn_mgmt)?;
            state.conn_mgmt.on_synack_in_synsent()?;

            // Now we need to send ACK
            // This will be handled by the TX path
            return Ok(());
        }

        // SYN without ACK (simultaneous open - rare)
        if seg.flags.syn && !seg.flags.ack {
            // TODO: Handle simultaneous open
            return Err("Simultaneous open not yet implemented");
        }

        Err("Invalid segment in SYN_SENT")
    }

    /// Process segment in SYN_RCVD state
    fn process_synrcvd(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            // Call component methods for RST
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // Check for ACK to complete handshake
        if seg.flags.ack && !seg.flags.syn {
            // Call component methods for ACK in SYN_RCVD
            state.rod.on_ack_in_synrcvd(seg)?;
            state.flow_ctrl.on_ack_in_synrcvd(seg)?;
            state.cong_ctrl.on_ack_in_synrcvd()?;
            state.conn_mgmt.on_ack_in_synrcvd()?;
            return Ok(());
        }

        // Retransmitted SYN?
        if seg.flags.syn {
            // TODO: Handle retransmitted SYN
            return Err("Retransmitted SYN not yet implemented");
        }

        Err("Invalid segment in SYN_RCVD")
    }

    /// Process segment in ESTABLISHED state
    fn process_established(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            // Call component methods for RST
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // Check for FIN (peer closing connection)
        if seg.flags.fin {
            // Call component methods for FIN in ESTABLISHED
            state.rod.on_fin_in_established(seg)?;
            state.flow_ctrl.on_fin_in_established(seg)?;
            state.cong_ctrl.on_fin_in_established(seg)?;
            state.conn_mgmt.on_fin_in_established()?;
            // Should send ACK
            return Ok(());
        }

        // TODO: Process data and ACKs
        // This is where the data path components will come in

        Ok(())
    }

    /// Process segment in FIN_WAIT_1 state
    fn process_finwait1(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // Check for ACK (which may also have FIN set)
        if seg.flags.ack {
            state.rod.on_ack_in_finwait1(seg)?;
            state.flow_ctrl.on_ack_in_finwait1(seg)?;
            state.cong_ctrl.on_ack_in_finwait1(seg)?;
            state.conn_mgmt.on_ack_in_finwait1()?;

            // If FIN is also set, handle it
            if seg.flags.fin {
                state.rod.on_fin_in_finwait1(seg)?;
                state.flow_ctrl.on_fin_in_finwait1(seg)?;
                state.cong_ctrl.on_fin_in_finwait1(seg)?;
                state.conn_mgmt.on_fin_in_finwait1()?;
            }
            return Ok(());
        }

        // Check for FIN only (without ACK - unusual)
        if seg.flags.fin {
            state.rod.on_fin_in_finwait1(seg)?;
            state.flow_ctrl.on_fin_in_finwait1(seg)?;
            state.cong_ctrl.on_fin_in_finwait1(seg)?;
            state.conn_mgmt.on_fin_in_finwait1()?;
            return Ok(());
        }

        Ok(())
    }

    /// Process segment in FIN_WAIT_2 state
    fn process_finwait2(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // Check for FIN
        if seg.flags.fin {
            state.rod.on_fin_in_finwait2(seg)?;
            state.flow_ctrl.on_fin_in_finwait2(seg)?;
            state.cong_ctrl.on_fin_in_finwait2(seg)?;
            state.conn_mgmt.on_fin_in_finwait2()?;
            return Ok(());
        }

        Ok(())
    }

    /// Process segment in CLOSE_WAIT state
    fn process_closewait(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // In CLOSE_WAIT, we're waiting for the application to close
        // Just process any data/ACKs
        // TODO: Handle data processing
        Ok(())
    }

    /// Process segment in CLOSING state
    fn process_closing(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // Check for ACK
        if seg.flags.ack {
            state.rod.on_ack_in_closing(seg)?;
            state.flow_ctrl.on_ack_in_closing(seg)?;
            state.cong_ctrl.on_ack_in_closing(seg)?;
            state.conn_mgmt.on_ack_in_closing()?;
            return Ok(());
        }

        Ok(())
    }

    /// Process segment in LAST_ACK state
    fn process_lastack(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Check for RST
        if seg.flags.rst {
            state.rod.on_rst()?;
            state.flow_ctrl.on_rst()?;
            state.cong_ctrl.on_rst()?;
            state.conn_mgmt.on_rst()?;
            return Err("Connection reset");
        }

        // Check for ACK
        if seg.flags.ack {
            state.rod.on_ack_in_lastack(seg)?;
            state.flow_ctrl.on_ack_in_lastack(seg)?;
            state.cong_ctrl.on_ack_in_lastack(seg)?;
            state.conn_mgmt.on_ack_in_lastack()?;
            return Ok(());
        }

        Ok(())
    }

    /// Process segment in TIME_WAIT state
    fn process_timewait(
        _state: &mut TcpConnectionState,
        _seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // In TIME_WAIT, we just absorb packets
        // The timer will eventually close the connection
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_flags() {
        let flags = TcpFlags {
            syn: true,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        };

        assert!(flags.syn);
        assert!(flags.ack);
        assert!(!flags.fin);
    }
}
