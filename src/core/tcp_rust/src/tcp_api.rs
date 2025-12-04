//! TCP API Functions
//!
//! High-level API functions for TCP connections (bind, listen, connect, etc.)
//! These orchestrate component methods - they do NOT directly modify component state.

use crate::state::{TcpConnectionState, TcpState};
use crate::ffi;

/// Bind to a local IP and port
///
/// Transition: CLOSED -> CLOSED (with IP and port assigned)
/// Returns: Ok(port) on success
pub fn tcp_bind(
    state: &mut TcpConnectionState,
    local_ip: ffi::ip_addr_t,
    local_port: u16,
) -> Result<u16, &'static str> {
    // Delegate to connection management component
    state.conn_mgmt.on_bind(local_ip, local_port)
}

/// Start listening for connections
///
/// Transition: CLOSED -> LISTEN
pub fn tcp_listen(state: &mut TcpConnectionState) -> Result<(), &'static str> {
    // Delegate to connection management component
    state.conn_mgmt.on_listen()
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
    // Validate state first (before calling any component methods)
    if state.conn_mgmt.state != TcpState::Closed {
        return Err("Can only connect from CLOSED state");
    }

    // Each component handles its own initialization
    // Order: data components first, then state transition last
    state.rod.on_connect()?;
    state.flow_ctrl.on_connect()?;
    state.cong_ctrl.on_connect(&state.conn_mgmt)?;
    state.conn_mgmt.on_connect(remote_ip, remote_port)?;

    Ok(())
}

/// Initiate graceful close
///
/// Handles closing from various states
/// Returns: Ok(true) if FIN should be sent, Ok(false) if already closing/closed
pub fn initiate_close(state: &mut TcpConnectionState) -> Result<bool, &'static str> {
    // Delegate to connection management component
    state.conn_mgmt.on_close()
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

    // Each component resets its own state
    state.rod.on_abort()?;
    state.flow_ctrl.on_abort()?;
    state.cong_ctrl.on_abort()?;
    state.conn_mgmt.on_abort()?;

    Ok(should_send_rst)
}

/// Process an incoming TCP segment represented as a parsed `TcpSegment`.
///
/// This is a test-friendly dispatcher that mirrors the old `ControlPath::tcp_input` behavior.
pub fn tcp_input(
    state: &mut TcpConnectionState,
    seg: &crate::tcp_types::TcpSegment,
    remote_ip: ffi::ip_addr_t,
    remote_port: u16,
) -> Result<crate::tcp_types::InputAction, &'static str> {
    use crate::tcp_types::{InputAction};

    // Handle RST first (in any state)
    if seg.flags.rst {
        match state.rod.validate_rst(seg, state.flow_ctrl.rcv_wnd) {
            crate::tcp_types::RstValidation::Valid => {
                // Close connection
                state.conn_mgmt.on_rst()?;
                return Ok(InputAction::Abort);
            }
            crate::tcp_types::RstValidation::Challenge => return Ok(InputAction::SendChallengeAck),
            crate::tcp_types::RstValidation::Invalid => return Ok(InputAction::Drop),
        }
    }

    // Dispatch based on current state
    match state.conn_mgmt.state {
        TcpState::Closed => {
            // RFC 793: All segments are rejected in CLOSED state
            // Send RST if not already RST
            if !seg.flags.rst {
                Ok(InputAction::SendRst)
            } else {
                Ok(InputAction::Drop)
            }
        }
        TcpState::Listen => {
            // Only accept SYN in LISTEN state
            if seg.flags.syn && !seg.flags.ack {
                // Process the SYN using component methods
                state.rod.on_syn_in_listen(seg)?;
                state.flow_ctrl.on_syn_in_listen(seg, &state.conn_mgmt)?;
                state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;
                state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;
                Ok(InputAction::SendSynAck)
            } else {
                Ok(InputAction::SendRst)
            }
        }
        TcpState::SynSent => {
            // Expecting SYN+ACK
            if seg.flags.syn && seg.flags.ack {
                // Let components process SYN+ACK
                state.rod.on_synack_in_synsent(seg)?;
                state.flow_ctrl.on_synack_in_synsent(seg)?;
                state.cong_ctrl.on_synack_in_synsent(&state.conn_mgmt)?;
                state.conn_mgmt.on_synack_in_synsent()?;
                Ok(InputAction::Accept)
            } else if seg.flags.syn {
                // Simultaneous open (SYN without ACK)
                Ok(InputAction::Accept)
            } else {
                Ok(InputAction::Drop)
            }
        }
        TcpState::SynRcvd => {
            // Validate sequence number
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }

            // Expecting ACK of our SYN
            if seg.flags.ack {
                // Let components handle ACK in SYN_RCVD
                state.rod.on_ack_in_synrcvd(seg)?;
                state.flow_ctrl.on_ack_in_synrcvd(seg)?;
                state.cong_ctrl.on_ack_in_synrcvd()?;
                state.conn_mgmt.on_ack_in_synrcvd()?;
                Ok(InputAction::Accept)
            } else {
                Ok(InputAction::Drop)
            }
        }
        TcpState::Established => {
            // Validate sequence number
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }

            // Validate ACK if present
            if seg.flags.ack {
                match state.rod.validate_ack(seg) {
                    crate::tcp_types::AckValidation::Valid | crate::tcp_types::AckValidation::Duplicate => {
                        // Process normally via components (ACK handling)
                        // For now, no-op at API level
                    }
                    crate::tcp_types::AckValidation::Future => {
                        // RFC 5961: ACK of unsent data - send challenge ACK
                        return Ok(InputAction::SendChallengeAck);
                    }
                    crate::tcp_types::AckValidation::Old | crate::tcp_types::AckValidation::Invalid => {
                        return Ok(InputAction::Drop);
                    }
                }
            }

            // Check for FIN
            if seg.flags.fin {
                // Process FIN and transition to CLOSE_WAIT
                state.rod.on_fin_in_established(seg)?;
                state.flow_ctrl.on_fin_in_established(seg)?;
                state.cong_ctrl.on_fin_in_established(seg)?;
                state.conn_mgmt.on_fin_in_established()?;
                Ok(InputAction::SendAck)
            } else {
                Ok(InputAction::Accept)
            }
        }
        TcpState::FinWait1 => {
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }

            if seg.flags.ack || seg.flags.fin {
                Ok(InputAction::Accept)
            } else {
                Ok(InputAction::Drop)
            }
        }
        TcpState::FinWait2 => {
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }

            if seg.flags.fin {
                Ok(InputAction::Accept)
            } else {
                Ok(InputAction::Accept)
            }
        }
        TcpState::CloseWait => {
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }
            Ok(InputAction::Accept)
        }
        TcpState::Closing => {
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }

            if seg.flags.ack {
                Ok(InputAction::Accept)
            } else {
                Ok(InputAction::Drop)
            }
        }
        TcpState::LastAck => {
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }

            if seg.flags.ack {
                Ok(InputAction::Accept)
            } else {
                Ok(InputAction::Drop)
            }
        }
        TcpState::TimeWait => {
            if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
                return Ok(InputAction::Drop);
            }

            if seg.flags.fin {
                Ok(InputAction::SendAck)
            } else {
                Ok(InputAction::Accept)
            }
        }
    }
}
