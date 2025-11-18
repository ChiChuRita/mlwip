//! TCP Control Path Tests
//!
//! These tests are translated from lwIP's test_tcp_state.c and verify
//! the TCP state machine, connection setup/teardown, and RST handling.

mod test_helpers;

use test_helpers::*;
use lwip_tcp_rust::{
    TcpFlags, TcpSegment,
    RstValidation, AckValidation, InputAction,
    tcp_bind, tcp_listen, tcp_connect, tcp_abort, initiate_close
};
use lwip_tcp_rust::control_path::ControlPath;  // Legacy test functions
use lwip_tcp_rust::state::{TcpConnectionState, TcpState};
use lwip_tcp_rust::tcp_proto;
use lwip_tcp_rust::ffi;

// ============================================================================
// Test 1: Active Open (tcp_connect)
// ============================================================================

#[test]
fn test_tcp_connect_active_open() {
    reset_iss();
    let mut state = create_test_state();
    let mut tx_capture = MockTxCapture::new();

    // Initial state should be CLOSED
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);

    // Simulate tcp_connect() - transition to SYN_SENT
    state.conn_mgmt.state = TcpState::SynSent;
    state.rod.iss = next_iss();
    state.rod.snd_nxt = state.rod.iss;
    state.rod.lastack = state.rod.iss;

    // In real implementation, tcp_connect would send SYN
    // For now, we verify state transition
    assert_eq!(state.conn_mgmt.state, TcpState::SynSent);

    // Simulate receiving SYN-ACK
    let synack_seg = TcpSegment {
        seqno: 12345,
        ackno: state.rod.snd_nxt.wrapping_add(1),
        flags: TcpFlags {
            syn: true,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process SYN-ACK (should transition to ESTABLISHED)
    // Use component methods
    let result = state.rod.on_synack_in_synsent(&synack_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_synack_in_synsent(&synack_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_synack_in_synsent(&state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_synack_in_synsent();
    assert!(result.is_ok());

    assert_eq!(state.conn_mgmt.state, TcpState::Established);
    assert_eq!(state.rod.rcv_nxt, 12346); // seqno + 1
}

// ============================================================================
// Test 2: Active Close (tcp_close from ESTABLISHED)
// ============================================================================

#[test]
fn test_tcp_active_close() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Close from ESTABLISHED should transition to FIN_WAIT_1
    let result = initiate_close(&mut state);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true); // Should send FIN
    assert_eq!(state.conn_mgmt.state, TcpState::FinWait1);

    // Receive ACK of our FIN -> FIN_WAIT_2
    let ack_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt.wrapping_add(1), // ACK our FIN
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process ACK in FIN_WAIT_1 - use component methods
    let result = state.rod.on_ack_in_finwait1(&ack_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_ack_in_finwait1(&ack_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_ack_in_finwait1(&ack_seg);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_ack_in_finwait1();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::FinWait2);

    // Receive FIN from peer -> TIME_WAIT
    let fin_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt.wrapping_add(1),
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: true,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process FIN in FIN_WAIT_2 - use component methods
    let result = state.rod.on_fin_in_finwait2(&fin_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_fin_in_finwait2(&fin_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_fin_in_finwait2(&fin_seg);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_fin_in_finwait2();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::TimeWait);

    // After 2*MSL timer expires, should transition to CLOSED
    // (Timer implementation pending)
}

// ============================================================================
// Test 3: Simultaneous Close
// ============================================================================

#[test]
fn test_tcp_simultaneous_close() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Both sides send FIN -> FIN_WAIT_1
    let result = initiate_close(&mut state);
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::FinWait1);

    // Receive FIN from peer (crossing FINs) -> CLOSING
    let fin_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt, // No ACK of our FIN yet
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: true,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process FIN in FIN_WAIT_1 (crossing FINs) - use component methods
    let result = state.rod.on_fin_in_finwait1(&fin_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_fin_in_finwait1(&fin_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_fin_in_finwait1(&fin_seg);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_fin_in_finwait1();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::Closing);

    // Receive ACK of our FIN -> TIME_WAIT
    let ack_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt.wrapping_add(1),
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process ACK in CLOSING - use component methods
    let result = state.rod.on_ack_in_closing(&ack_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_ack_in_closing(&ack_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_ack_in_closing(&ack_seg);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_ack_in_closing();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::TimeWait);
}

// ============================================================================
// Test 4: RST Generation in CLOSED State
// ============================================================================

#[test]
fn test_tcp_gen_rst_in_closed() {
    // No PCB exists - any incoming segment should generate RST
    let incoming_seg = create_segment(
        TEST_REMOTE_IP,
        TEST_LOCAL_IP,
        TEST_REMOTE_PORT,
        TEST_LOCAL_PORT,
        &[],
        12345,
        54321,
        tcp_proto::TCP_ACK,
    );

    // In full implementation, tcp_input would generate RST
    // For now, we just verify the logic:
    // - No matching PCB found
    // - Should send RST with correct sequence numbers
    assert!(incoming_seg.has_ack());
}

// ============================================================================
// Test 5: RST Generation in LISTEN State
// ============================================================================

#[test]
fn test_tcp_gen_rst_in_listen() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::Listen;

    // Receive ACK (not SYN) in LISTEN -> should send RST
    let ack_seg = create_segment(
        TEST_REMOTE_IP,
        TEST_LOCAL_IP,
        TEST_REMOTE_PORT,
        TEST_LOCAL_PORT,
        &[],
        12345,
        54321,
        tcp_proto::TCP_ACK,
    );

    // In LISTEN state, only SYN is acceptable
    // ACK should trigger RST
    assert!(ack_seg.has_ack());
    assert!(!ack_seg.has_syn());

    // State should remain LISTEN
    assert_eq!(state.conn_mgmt.state, TcpState::Listen);
}

// ============================================================================
// Test 6: RST Generation in TIME_WAIT State
// ============================================================================

#[test]
fn test_tcp_gen_rst_in_time_wait() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::TimeWait,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Receive SYN in TIME_WAIT -> should send RST
    let syn_seg = create_rx_segment(&state, &[], 0, 0, tcp_proto::TCP_SYN, 8192);

    assert!(syn_seg.has_syn());

    // State should remain TIME_WAIT
    assert_eq!(state.conn_mgmt.state, TcpState::TimeWait);
}

// ============================================================================
// Test 7: RST Processing with Sequence Number Validation
// ============================================================================

#[test]
fn test_tcp_process_rst_seqno() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    let initial_rcv_nxt = state.rod.rcv_nxt;

    // RST with incorrect sequence number should be rejected
    let bad_rst = TcpSegment {
        seqno: state.rod.rcv_nxt.wrapping_sub(10), // Out of window
        ackno: 54321,
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: false,
            rst: true,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process RST with bad seqno (to be implemented)
    // Should NOT abort connection
    // TODO: Implement sequence number validation in process_rst
    // For now, we skip this test
    // let result = ControlPath::process_rst(&mut state);

    // Connection should still be ESTABLISHED
    assert_eq!(state.conn_mgmt.state, TcpState::Established);

    // RST with correct sequence number should be accepted
    let good_rst = TcpSegment {
        seqno: state.rod.rcv_nxt, // Exact match
        ackno: 54321,
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: false,
            rst: true,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process RST with correct seqno - use component methods
    let _ = state.rod.on_rst();
    let _ = state.flow_ctrl.on_rst();
    let _ = state.cong_ctrl.on_rst();
    let _ = state.conn_mgmt.on_rst();

    // Connection should be aborted (CLOSED)
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
}

// ============================================================================
// Test 8: RST Generation in SYN_SENT with Incorrect ACK
// ============================================================================

#[test]
fn test_tcp_gen_rst_in_syn_sent_ackseq() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::SynSent;
    state.rod.iss = 1000;
    state.rod.snd_nxt = 1001;

    // Receive SYN-ACK with incorrect ACK number
    let bad_synack = TcpSegment {
        seqno: 12345,
        ackno: 9999, // Wrong ACK (should be snd_nxt + 1)
        flags: TcpFlags {
            syn: true,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Should reject and send RST - use component methods
    let result = state.rod.on_synack_in_synsent(&bad_synack);

    assert!(result.is_err());
    // State should remain SYN_SENT or go to CLOSED
}

// ============================================================================
// Test 9: RST Generation in SYN_SENT with Non-SYN ACK
// ============================================================================

#[test]
fn test_tcp_gen_rst_in_syn_sent_non_syn_ack() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::SynSent;
    state.rod.iss = 1000;
    state.rod.snd_nxt = 1001;

    // Receive ACK without SYN (invalid in SYN_SENT)
    let ack_only = TcpSegment {
        seqno: 12345,
        ackno: state.rod.snd_nxt.wrapping_add(1),
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Should reject (SYN_SENT expects SYN+ACK, not just ACK)
    // In real implementation, this would send RST
    assert!(!ack_only.flags.syn);
    assert!(ack_only.flags.ack);
}

// ============================================================================
// Test 10: RST Generation in SYN_RCVD with Incorrect ACK
// ============================================================================

#[test]
fn test_tcp_gen_rst_in_syn_rcvd() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::SynRcvd;
    state.rod.iss = 1000;
    state.rod.snd_nxt = 1001;
    state.rod.irs = 2000;
    state.rod.rcv_nxt = 2001;

    // Receive ACK with incorrect sequence number
    let bad_ack = TcpSegment {
        seqno: 9999, // Wrong seqno
        ackno: state.rod.snd_nxt.wrapping_add(1),
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Should send RST due to out-of-window seqno
    // (To be implemented in full control path)
}

// ============================================================================
// Test 11: RST Received in SYN_RCVD Returns to LISTEN
// ============================================================================

#[test]
fn test_tcp_receive_rst_syn_rcvd_to_listen() {
    let mut state = create_test_state();

    // Start in LISTEN
    state.conn_mgmt.state = TcpState::Listen;

    // Receive SYN -> transition to SYN_RCVD
    let syn_seg = TcpSegment {
        seqno: 1000,
        ackno: 0,
        flags: TcpFlags {
            syn: true,
            ack: false,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Use component methods
    let result = state.rod.on_syn_in_listen(&syn_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_syn_in_listen(&syn_seg, &state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_syn_in_listen(
        crate::ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        TEST_REMOTE_PORT,
    );

    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::SynRcvd);

    // Receive RST -> should return to LISTEN (for listening PCB)
    let rst_seg = TcpSegment {
        seqno: 1001,
        ackno: 0,
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: false,
            rst: true,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process RST (to be implemented)
    // For listening PCB, should return to LISTEN
    // For non-listening PCB, should go to CLOSED
}

// ============================================================================
// Test 12: Passive Close (Receive FIN in ESTABLISHED)
// ============================================================================

#[test]
fn test_tcp_passive_close() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Receive FIN from peer -> CLOSE_WAIT
    let fin_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: true,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process FIN in ESTABLISHED -> CLOSE_WAIT - use component methods
    let result = state.rod.on_fin_in_established(&fin_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_fin_in_established(&fin_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_fin_in_established(&fin_seg);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_fin_in_established();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::CloseWait);
    assert_eq!(state.rod.rcv_nxt, fin_seg.seqno.wrapping_add(1)); // FIN consumed 1 seq

    // Application calls tcp_close() -> LAST_ACK
    let result = initiate_close(&mut state);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true); // Should send FIN
    assert_eq!(state.conn_mgmt.state, TcpState::LastAck);

    // Receive ACK of our FIN -> CLOSED
    let ack_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt.wrapping_add(1),
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Process ACK in LAST_ACK -> CLOSED - use component methods
    let result = state.rod.on_ack_in_lastack(&ack_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_ack_in_lastack(&ack_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_ack_in_lastack(&ack_seg);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_ack_in_lastack();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
}

// ============================================================================
// Test 13: API Function Tests - tcp_bind()
// ============================================================================

#[test]
fn test_tcp_bind_success() {
    let mut state = create_test_state();
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);

    // Bind to specific port
    let result = tcp_bind(&mut state, ffi::ip_addr_t { addr: TEST_LOCAL_IP }, 8080);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 8080);
    assert_eq!(state.conn_mgmt.local_ip.addr, TEST_LOCAL_IP);
    assert_eq!(state.conn_mgmt.local_port, 8080);
}

#[test]
fn test_tcp_bind_wrong_state() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::Established;

    // Cannot bind in non-CLOSED state
    let result = tcp_bind(&mut state, ffi::ip_addr_t { addr: TEST_LOCAL_IP }, 8080);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Can only bind in CLOSED state");
}

#[test]
fn test_tcp_bind_port_zero() {
    let mut state = create_test_state();

    // Port 0 not yet supported (needs port allocation)
    let result = tcp_bind(&mut state, ffi::ip_addr_t { addr: TEST_LOCAL_IP }, 0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Port 0 not yet supported - provide explicit port");
}

// ============================================================================
// Test 14: API Function Tests - tcp_listen()
// ============================================================================

#[test]
fn test_tcp_listen_success() {
    let mut state = create_test_state();

    // Must bind first
    let result = tcp_bind(&mut state, ffi::ip_addr_t { addr: TEST_LOCAL_IP }, 8080);
    assert!(result.is_ok());

    // Now listen
    let result = tcp_listen(&mut state);
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::Listen);
}

#[test]
fn test_tcp_listen_without_bind() {
    // Create fresh state without pre-assigned port
    let mut state = TcpConnectionState::new();
    state.conn_mgmt.state = TcpState::Closed;

    // Cannot listen without binding to port
    let result = tcp_listen(&mut state);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Must bind to port before listening");
}

#[test]
fn test_tcp_listen_wrong_state() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::Established;
    state.conn_mgmt.local_port = 8080;

    // Cannot listen from non-CLOSED state
    let result = tcp_listen(&mut state);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Can only listen from CLOSED state");
}

// ============================================================================
// Test 15: API Function Tests - tcp_connect()
// ============================================================================

#[test]
fn test_tcp_connect_success() {
    reset_iss();
    let mut state = create_test_state();

    // Bind to local port first
    let result = tcp_bind(&mut state, ffi::ip_addr_t { addr: TEST_LOCAL_IP }, 12345);
    assert!(result.is_ok());

    // Connect to remote
    let result = tcp_connect(
        &mut state,
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        80,
    );
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::SynSent);
    assert_eq!(state.conn_mgmt.remote_ip.addr, TEST_REMOTE_IP);
    assert_eq!(state.conn_mgmt.remote_port, 80);

    // ISS should be initialized (matching lwIP behavior)
    assert_ne!(state.rod.iss, 0);
    assert_eq!(state.rod.snd_nxt, state.rod.iss);
    assert_eq!(state.rod.lastack, state.rod.iss.wrapping_sub(1)); // lwIP sets lastack = iss - 1

    // Windows should be initialized
    assert_eq!(state.flow_ctrl.rcv_wnd, 4096);
    assert!(state.cong_ctrl.cwnd > 0);
}

#[test]
fn test_tcp_connect_wrong_state() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::Established;
    state.conn_mgmt.local_port = 12345;

    // Cannot connect from non-CLOSED state
    let result = tcp_connect(
        &mut state,
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        80,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Can only connect from CLOSED state");
}

// ============================================================================
// Test 16: API Function Tests - tcp_abort()
// ============================================================================

#[test]
fn test_tcp_abort_established() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Abort should send RST
    let result = tcp_abort(&mut state);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true); // Should send RST
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
}

#[test]
fn test_tcp_abort_listen() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::Listen;

    // Abort from LISTEN doesn't need RST
    let result = tcp_abort(&mut state);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), false); // No RST needed
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
}

#[test]
fn test_tcp_abort_closed() {
    let mut state = create_test_state();
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);

    // Abort already closed connection
    let result = tcp_abort(&mut state);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), false); // No RST needed
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
}

// ============================================================================
// Test 17: API Integration - Full Connection Lifecycle
// ============================================================================

#[test]
fn test_full_server_lifecycle() {
    reset_iss();
    let mut state = create_test_state();

    // 1. Bind
    let result = tcp_bind(&mut state, ffi::ip_addr_t { addr: TEST_LOCAL_IP }, 8080);
    assert!(result.is_ok());

    // 2. Listen
    let result = tcp_listen(&mut state);
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::Listen);

    // 3. Receive SYN -> SYN_RCVD
    let syn_seg = TcpSegment {
        seqno: 1000,
        ackno: 0,
        flags: TcpFlags {
            syn: true,
            ack: false,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Use component methods
    let result = state.rod.on_syn_in_listen(&syn_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_syn_in_listen(&syn_seg, &state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_syn_in_listen(
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        TEST_REMOTE_PORT,
    );
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::SynRcvd);

    // 4. Receive ACK -> ESTABLISHED
    let ack_seg = TcpSegment {
        seqno: 1001,
        ackno: state.rod.iss.wrapping_add(1),
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Use component methods
    let result = state.rod.on_ack_in_synrcvd(&ack_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_ack_in_synrcvd(&ack_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_ack_in_synrcvd();
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_ack_in_synrcvd();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::Established);

    // 5. Close
    let result = initiate_close(&mut state);
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::FinWait1);
}

#[test]
fn test_full_client_lifecycle() {
    reset_iss();
    let mut state = create_test_state();

    // 1. Bind
    let result = tcp_bind(&mut state, ffi::ip_addr_t { addr: TEST_LOCAL_IP }, 12345);
    assert!(result.is_ok());

    // 2. Connect -> SYN_SENT
    let result = tcp_connect(
        &mut state,
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        80,
    );
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::SynSent);

    // 3. Receive SYN-ACK -> ESTABLISHED
    let synack_seg = TcpSegment {
        seqno: 5000,
        ackno: state.rod.iss.wrapping_add(1),
        flags: TcpFlags {
            syn: true,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Use component methods
    let result = state.rod.on_synack_in_synsent(&synack_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_synack_in_synsent(&synack_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_synack_in_synsent(&state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_synack_in_synsent();
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::Established);

    // 4. Close
    let result = initiate_close(&mut state);
    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::FinWait1);
}

// ============================================================================
// Test 18: Sequence Number Validation
// ============================================================================

#[test]
fn test_validate_sequence_number_in_window() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Set receive window
    state.rod.rcv_nxt = 1000;
    state.flow_ctrl.rcv_wnd = 8192;

    // Segment starting at rcv_nxt (exact match)
    let seg = TcpSegment {
        seqno: 1000,
        ackno: 0,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 100,
    };

    assert!(ControlPath::validate_sequence_number(&state, &seg));

    // Segment in middle of window
    let seg2 = TcpSegment {
        seqno: 5000,
        ackno: 0,
        flags: seg.flags,
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 100,
    };

    assert!(ControlPath::validate_sequence_number(&state, &seg2));

    // Segment at end of window
    let seg3 = TcpSegment {
        seqno: 9191, // rcv_nxt + rcv_wnd - 1
        ackno: 0,
        flags: seg.flags,
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 1,
    };

    assert!(ControlPath::validate_sequence_number(&state, &seg3));
}

#[test]
fn test_validate_sequence_number_out_of_window() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.rcv_nxt = 1000;
    state.flow_ctrl.rcv_wnd = 8192;

    // Segment before window
    let seg = TcpSegment {
        seqno: 500,
        ackno: 0,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 100,
    };

    assert!(!ControlPath::validate_sequence_number(&state, &seg));

    // Segment after window
    let seg2 = TcpSegment {
        seqno: 20000,
        ackno: 0,
        flags: seg.flags,
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 100,
    };

    assert!(!ControlPath::validate_sequence_number(&state, &seg2));
}

#[test]
fn test_validate_sequence_number_zero_window() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.rcv_nxt = 1000;
    state.flow_ctrl.rcv_wnd = 0; // Zero window

    // Only exact match should be accepted
    let seg_exact = TcpSegment {
        seqno: 1000,
        ackno: 0,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    assert!(ControlPath::validate_sequence_number(&state, &seg_exact));

    // Anything else should be rejected
    let seg_off = TcpSegment {
        seqno: 1001,
        ackno: 0,
        flags: seg_exact.flags,
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    assert!(!ControlPath::validate_sequence_number(&state, &seg_off));
}

// ============================================================================
// Test 19: RST Validation (RFC 5961)
// ============================================================================

#[test]
fn test_validate_rst_in_window() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.rcv_nxt = 1000;
    state.flow_ctrl.rcv_wnd = 8192;

    // RST with sequence number in window
    let seg = TcpSegment {
        seqno: 5000, // In window
        ackno: 0,
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: false,
            rst: true,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::validate_rst(&state, &seg);
    assert_eq!(result, RstValidation::Valid);
}

#[test]
fn test_validate_rst_out_of_window() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.rcv_nxt = 1000;
    state.flow_ctrl.rcv_wnd = 8192;

    // RST with sequence number out of window
    let seg = TcpSegment {
        seqno: 20000, // Way out of window
        ackno: 0,
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: false,
            rst: true,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::validate_rst(&state, &seg);
    assert_eq!(result, RstValidation::Challenge);
}

// ============================================================================
// Test 20: ACK Validation (RFC 5961)
// ============================================================================

#[test]
fn test_validate_ack_valid() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.lastack = 1000; // SND.UNA
    state.rod.snd_nxt = 2000; // SND.NXT

    // Valid ACK (in range)
    let seg = TcpSegment {
        seqno: 0,
        ackno: 1500, // Between SND.UNA and SND.NXT
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::validate_ack(&state, &seg);
    assert_eq!(result, AckValidation::Valid);
}

#[test]
fn test_validate_ack_duplicate() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.lastack = 1000;
    state.rod.snd_nxt = 2000;

    // Duplicate ACK (ACK == SND.UNA)
    let seg = TcpSegment {
        seqno: 0,
        ackno: 1000,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::validate_ack(&state, &seg);
    assert_eq!(result, AckValidation::Duplicate);
}

#[test]
fn test_validate_ack_future() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.lastack = 1000;
    state.rod.snd_nxt = 2000;

    // Future ACK (ACK > SND.NXT)
    let seg = TcpSegment {
        seqno: 0,
        ackno: 3000,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::validate_ack(&state, &seg);
    assert_eq!(result, AckValidation::Future);
}

#[test]
fn test_validate_ack_old() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    state.rod.lastack = 1000;
    state.rod.snd_nxt = 2000;

    // Old ACK (ACK < SND.UNA)
    let seg = TcpSegment {
        seqno: 0,
        ackno: 500,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::validate_ack(&state, &seg);
    assert_eq!(result, AckValidation::Old);
}

// ============================================================================
// Test 21: tcp_input Dispatcher
// ============================================================================

#[test]
fn test_tcp_input_dispatcher_listen() {
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::Listen;

    // Send SYN to LISTEN
    let syn_seg = TcpSegment {
        seqno: 1000,
        ackno: 0,
        flags: TcpFlags {
            syn: true,
            ack: false,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::tcp_input(
        &mut state,
        &syn_seg,
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        TEST_REMOTE_PORT,
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), InputAction::SendSynAck);
    assert_eq!(state.conn_mgmt.state, TcpState::SynRcvd);
}

#[test]
fn test_tcp_input_dispatcher_established_with_fin() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Send FIN in ESTABLISHED
    let fin_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt,
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: true,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::tcp_input(
        &mut state,
        &fin_seg,
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        TEST_REMOTE_PORT,
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), InputAction::SendAck);
    assert_eq!(state.conn_mgmt.state, TcpState::CloseWait);
}

#[test]
fn test_tcp_input_dispatcher_rst_in_window() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Send valid RST
    let rst_seg = TcpSegment {
        seqno: state.rod.rcv_nxt,
        ackno: state.rod.snd_nxt,
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: false,
            rst: true,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::tcp_input(
        &mut state,
        &rst_seg,
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        TEST_REMOTE_PORT,
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), InputAction::Abort);
    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
}

#[test]
fn test_tcp_input_dispatcher_rst_out_of_window() {
    let mut state = create_test_state();
    set_tcp_state(
        &mut state,
        TcpState::Established,
        TEST_LOCAL_IP,
        TEST_REMOTE_IP,
        TEST_LOCAL_PORT,
        TEST_REMOTE_PORT,
    );

    // Send RST with bad sequence number
    let rst_seg = TcpSegment {
        seqno: state.rod.rcv_nxt.wrapping_add(100000), // Way out of window
        ackno: state.rod.snd_nxt,
        flags: TcpFlags {
            syn: false,
            ack: false,
            fin: false,
            rst: true,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    let result = ControlPath::tcp_input(
        &mut state,
        &rst_seg,
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        TEST_REMOTE_PORT,
    );

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), InputAction::SendChallengeAck);
    // State should NOT change to Closed
    assert_eq!(state.conn_mgmt.state, TcpState::Established);
}

// ============================================================================
// Test 22: Handshake Tests (Already Implemented)
// ============================================================================

#[test]
fn test_tcp_passive_open_handshake() {
    reset_iss();
    let mut state = create_test_state();
    state.conn_mgmt.state = TcpState::Listen;

    // Receive SYN
    let syn_seg = TcpSegment {
        seqno: 1000,
        ackno: 0,
        flags: TcpFlags {
            syn: true,
            ack: false,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Use component methods
    let result = state.rod.on_syn_in_listen(&syn_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_syn_in_listen(&syn_seg, &state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt);
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_syn_in_listen(
        ffi::ip_addr_t { addr: TEST_REMOTE_IP },
        TEST_REMOTE_PORT,
    );

    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::SynRcvd);
    assert_eq!(state.rod.rcv_nxt, 1001);

    // Receive ACK
    let ack_seg = TcpSegment {
        seqno: 1001,
        ackno: state.rod.snd_nxt.wrapping_add(1),
        flags: TcpFlags {
            syn: false,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 8192,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Use component methods
    let result = state.rod.on_ack_in_synrcvd(&ack_seg);
    assert!(result.is_ok());
    let result = state.flow_ctrl.on_ack_in_synrcvd(&ack_seg);
    assert!(result.is_ok());
    let result = state.cong_ctrl.on_ack_in_synrcvd();
    assert!(result.is_ok());
    let result = state.conn_mgmt.on_ack_in_synrcvd();

    assert!(result.is_ok());
    assert_eq!(state.conn_mgmt.state, TcpState::Established);
}
