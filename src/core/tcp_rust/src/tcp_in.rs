//! TCP Packet Reception (RX Path)
//!
//! Handles incoming TCP segments and dispatches to appropriate handlers.
//! For handshake: processes SYN, SYN+ACK, and ACK segments.

use crate::state::TcpConnectionState;
use crate::control_path::{ControlPath, TcpSegment, TcpFlags};
use crate::ffi;

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
            crate::state::TcpState::Closed => {
                Err("Connection is closed")
            }
            _ => {
                // TODO: Implement other states (FIN_WAIT, CLOSE_WAIT, etc.)
                Err("State not yet implemented")
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
        let tcphdr = pbuf.payload as *const ffi::tcp_hdr;
        if tcphdr.is_null() {
            return Err("Null TCP header");
        }

        let hdr = &*tcphdr;

        // Extract fields (convert from network byte order)
        let seqno = u32::from_be(hdr.seqno);
        let ackno = u32::from_be(hdr.ackno);
        let wnd = u16::from_be(hdr.wnd);

        // Extract flags from the combined field
        let flags_byte = (u16::from_be(hdr._hdrlen_rsvd_flags) & 0x3F) as u8;
        let flags = TcpFlags::from_tcphdr(flags_byte);

        // Calculate header length (in bytes)
        let hdrlen_raw = u16::from_be(hdr._hdrlen_rsvd_flags) >> 12;
        let tcphdr_len = (hdrlen_raw as u16) * 4;

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
            ControlPath::process_syn_in_listen(state, seg, remote_ip, remote_port)?;

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
            ControlPath::process_rst(state);
            return Err("Connection reset");
        }

        // Check for SYN+ACK
        if seg.flags.syn && seg.flags.ack {
            ControlPath::process_synack_in_synsent(state, seg)?;

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
            ControlPath::process_rst(state);
            return Err("Connection reset");
        }

        // Check for ACK to complete handshake
        if seg.flags.ack && !seg.flags.syn {
            ControlPath::process_ack_in_synrcvd(state, seg)?;
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
        // For now, just basic validation
        // Full data path will be implemented later

        if seg.flags.rst {
            ControlPath::process_rst(state);
            return Err("Connection reset");
        }

        // TODO: Process data and ACKs
        // This is where the data path components will come in

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
