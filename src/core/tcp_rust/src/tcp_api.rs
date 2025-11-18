//! TCP API Functions
//!
//! High-level API functions for TCP connections (bind, listen, connect, etc.)
//! These will eventually be migrated to component methods.

use crate::state::{TcpConnectionState, TcpState};
use crate::ffi;

/// Helper to generate Initial Sequence Number (ISS)
///
/// TODO: Implement proper ISS generation per RFC 6528
/// For now, use a simple counter
unsafe fn generate_iss() -> u32 {
    static mut ISS_COUNTER: u32 = 0;
    ISS_COUNTER = ISS_COUNTER.wrapping_add(1);
    ISS_COUNTER
}

/// Bind to a local IP and port
///
/// Transition: CLOSED -> CLOSED (with IP and port assigned)
/// Returns: Ok(port) on success
pub fn tcp_bind(
    state: &mut TcpConnectionState,
    local_ip: ffi::ip_addr_t,
    local_port: u16,
) -> Result<u16, &'static str> {
    if state.conn_mgmt.state != TcpState::Closed {
        return Err("Can only bind in CLOSED state");
    }

    if local_port == 0 {
        return Err("Port 0 not yet supported - provide explicit port");
    }

    state.conn_mgmt.local_ip = local_ip;
    state.conn_mgmt.local_port = local_port;
    Ok(local_port)
}

/// Start listening for connections
///
/// Transition: CLOSED -> LISTEN
pub fn tcp_listen(state: &mut TcpConnectionState) -> Result<(), &'static str> {
    if state.conn_mgmt.state != TcpState::Closed {
        return Err("Can only listen from CLOSED state");
    }

    // In lwIP, tcp_listen can be called without explicit bind if local_port is set
    // The port must be set either via tcp_bind or by the PCB creation
    if state.conn_mgmt.local_port == 0 {
        return Err("Must bind to port before listening");
    }

    state.conn_mgmt.state = TcpState::Listen;
    Ok(())
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
    if state.conn_mgmt.state != TcpState::Closed {
        return Err("Can only connect from CLOSED state");
    }

    // Store remote endpoint
    state.conn_mgmt.remote_ip = remote_ip;
    state.conn_mgmt.remote_port = remote_port;

    // Generate our ISS
    state.rod.iss = unsafe { generate_iss() };
    state.rod.snd_nxt = state.rod.iss;
    state.rod.snd_lbb = state.rod.iss.wrapping_sub(1);
    state.rod.lastack = state.rod.iss.wrapping_sub(1);

    // Initialize our receive window
    state.flow_ctrl.rcv_wnd = 4096;
    state.flow_ctrl.rcv_ann_wnd = state.flow_ctrl.rcv_wnd;

    // Initialize congestion window
    let mss = state.conn_mgmt.mss as u16;
    state.cong_ctrl.cwnd = mss;

    // Transition to SYN_SENT
    state.conn_mgmt.state = TcpState::SynSent;

    Ok(())
}

/// Initiate graceful close
///
/// Handles closing from various states
/// Returns: Ok(true) if FIN should be sent, Ok(false) if already closing/closed
pub fn initiate_close(state: &mut TcpConnectionState) -> Result<bool, &'static str> {
    match state.conn_mgmt.state {
        TcpState::Closed => Ok(false),
        TcpState::Listen => {
            state.conn_mgmt.state = TcpState::Closed;
            Ok(false)
        }
        TcpState::SynSent | TcpState::SynRcvd => {
            state.conn_mgmt.state = TcpState::Closed;
            Ok(false)
        }
        TcpState::Established => {
            state.conn_mgmt.state = TcpState::FinWait1;
            Ok(true)
        }
        TcpState::CloseWait => {
            state.conn_mgmt.state = TcpState::LastAck;
            Ok(true)
        }
        _ => {
            // Already closing (FinWait1, FinWait2, Closing, LastAck, TimeWait)
            Ok(false)
        }
    }
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

    // Close immediately
    state.conn_mgmt.state = TcpState::Closed;
    Ok(should_send_rst)
}
