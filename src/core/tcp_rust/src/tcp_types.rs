//! TCP Common Types
//!
//! Shared types used across TCP implementation modules.

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
    Challenge,  // Challenge ACK should be sent
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
