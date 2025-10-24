//! TCP Packet Transmission (TX Path)
//!
//! Handles outgoing TCP segments for handshake.
//! Generates SYN, SYN+ACK, and ACK packets.

use crate::state::{TcpConnectionState, TcpState};
use crate::ffi;
use crate::tcp_proto;

/// TCP TX Path
///
/// Constructs and sends TCP segments.
/// For handshake: sends SYN, SYN+ACK, and ACK.
pub struct TcpTx;

impl TcpTx {
    /// Send a SYN segment (active open)
    ///
    /// Called when transitioning from CLOSED to SYN_SENT.
    pub unsafe fn send_syn(
        state: &mut TcpConnectionState,
        netif: *mut ffi::netif,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::Closed {
            return Err("Can only send SYN from CLOSED state");
        }

        // Prepare segment with SYN flag
        let flags = tcp_proto::TCP_SYN;

        Self::send_segment(
            state,
            flags,
            state.rod.iss,
            0,  // No ACK number for pure SYN
            0,  // No payload
            netif,
        )?;

        // Transition to SYN_SENT
        state.conn_mgmt.state = TcpState::SynSent;

        Ok(())
    }

    /// Send a SYN+ACK segment (passive open response)
    ///
    /// Called when in SYN_RCVD state after receiving a SYN.
    pub unsafe fn send_synack(
        state: &TcpConnectionState,
        netif: *mut ffi::netif,
    ) -> Result<(), &'static str> {
        if state.conn_mgmt.state != TcpState::SynRcvd {
            return Err("Can only send SYN+ACK from SYN_RCVD state");
        }

        // Prepare segment with SYN+ACK flags
        let flags = tcp_proto::TCP_SYN | tcp_proto::TCP_ACK;

        Self::send_segment(
            state,
            flags,
            state.rod.iss,
            state.rod.rcv_nxt,  // ACK the peer's SYN
            0,  // No payload
            netif,
        )?;

        Ok(())
    }

    /// Send an ACK segment (handshake completion)
    ///
    /// Called when transitioning from SYN_SENT to ESTABLISHED,
    /// or from SYN_RCVD to ESTABLISHED (duplicate, but allowed).
    pub unsafe fn send_ack(
        state: &TcpConnectionState,
        netif: *mut ffi::netif,
    ) -> Result<(), &'static str> {
        // Prepare segment with ACK flag
        let flags = tcp_proto::TCP_ACK;

        Self::send_segment(
            state,
            flags,
            state.rod.snd_nxt,
            state.rod.rcv_nxt,
            0,  // No payload
            netif,
        )?;

        Ok(())
    }

    /// Low-level: Construct and send a TCP segment
    ///
    /// This is the core transmission function.
    unsafe fn send_segment(
        state: &TcpConnectionState,
        flags: u8,
        seqno: u32,
        ackno: u32,
        payload_len: u16,
        netif: *mut ffi::netif,
    ) -> Result<(), &'static str> {
        // Allocate pbuf for TCP header (and payload if needed)
        let tcp_hdr_len = 20u16; // Minimum TCP header size (no options for now)
        let total_len = tcp_hdr_len + payload_len;

        let p = ffi::pbuf_alloc(
            ffi::pbuf_layer_PBUF_TRANSPORT,
            total_len,
            ffi::pbuf_type_PBUF_RAM,
        );

        if p.is_null() {
            return Err("Failed to allocate pbuf");
        }

        // Get pointer to TCP header
        let tcphdr = (*p).payload as *mut tcp_proto::TcpHdr;
        if tcphdr.is_null() {
            ffi::pbuf_free(p);
            return Err("Null TCP header payload");
        }

        // Fill in TCP header
        let hdr = &mut *tcphdr;

        hdr.src = state.conn_mgmt.local_port.to_be();
        hdr.dest = state.conn_mgmt.remote_port.to_be();
        hdr.seqno = seqno.to_be();
        hdr.ackno = ackno.to_be();

        // Set header length (5 = 20 bytes / 4) and flags
        let hdrlen_flags = ((tcp_hdr_len / 4) as u16) << 12 | (flags as u16);
        hdr._hdrlen_rsvd_flags = hdrlen_flags.to_be();

        hdr.wnd = state.flow_ctrl.rcv_ann_wnd.to_be();
        hdr.chksum = 0; // Will be calculated by ip_output
        hdr.urgp = 0;

        // Calculate checksum
        Self::calculate_checksum(hdr, &state.conn_mgmt.local_ip, &state.conn_mgmt.remote_ip, total_len);

        // Send to IP layer
        let result = Self::send_to_ip(
            p,
            &state.conn_mgmt.local_ip,
            &state.conn_mgmt.remote_ip,
            state.conn_mgmt.ttl,
            state.conn_mgmt.tos,
            netif,
        );

        // Free pbuf (ip_output makes a copy)
        ffi::pbuf_free(p);

        result
    }

    /// Calculate TCP checksum
    unsafe fn calculate_checksum(
        tcphdr: *mut tcp_proto::TcpHdr,
        src_ip: &ffi::ip_addr_t,
        dest_ip: &ffi::ip_addr_t,
        len: u16,
    ) {
        // TODO: Implement proper checksum calculation
        // For now, rely on hardware offload or IP layer checksum
        // In lwIP, this is done via inet_chksum_pseudo

        // Placeholder - zero checksum will cause packets to be dropped
        // but this is OK for initial testing
        (*tcphdr).chksum = 0;
    }

    /// Send packet to IP layer
    unsafe fn send_to_ip(
        p: *mut ffi::pbuf,
        src_ip: &ffi::ip_addr_t,
        dest_ip: &ffi::ip_addr_t,
        ttl: u8,
        tos: u8,
        netif: *mut ffi::netif,
    ) -> Result<(), &'static str> {
        // Determine IP version
        #[cfg(feature = "ipv4")]
        {
            // Call IPv4 output
            // TODO: Use proper ffi function when available
            // For now, this is a placeholder
            return Err("IP output not yet implemented");
        }

        #[cfg(not(feature = "ipv4"))]
        {
            return Err("No IP version configured");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_state_validation() {
        let mut state = TcpConnectionState::new();

        // Should fail - can't send SYN from non-CLOSED state
        state.conn_mgmt.state = TcpState::Listen;
        let result = unsafe { TcpTx::send_syn(&mut state, core::ptr::null_mut()) };
        assert!(result.is_err());
    }
}
