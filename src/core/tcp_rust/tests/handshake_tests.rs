//! Integration tests for TCP handshake implementation

use lwip_tcp_rust::{TcpConnectionState, TcpState, TcpSegment, TcpFlags};

#[test]
fn test_three_way_handshake_passive() {
    // Simulate passive open (server side)
    let mut state = TcpConnectionState::new();
    state.conn_mgmt.state = TcpState::Listen;
    state.conn_mgmt.mss = 1460;
    state.conn_mgmt.local_port = 80;

    // Step 1: Receive SYN
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

    let remote_ip = unsafe { core::mem::zeroed() };

    // Use component methods
    let result = state.rod.on_syn_in_listen(&syn_seg);
    assert!(result.is_ok(), "ROD SYN processing failed");

    let result = state.flow_ctrl.on_syn_in_listen(&syn_seg, &state.conn_mgmt);
    assert!(result.is_ok(), "FlowControl SYN processing failed");

    let result = state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt);
    assert!(result.is_ok(), "CongControl SYN processing failed");

    let result = state.conn_mgmt.on_syn_in_listen(remote_ip, 12345);
    assert!(result.is_ok(), "ConnMgmt SYN processing failed");

    assert_eq!(state.conn_mgmt.state, TcpState::SynRcvd);
    assert_eq!(state.rod.rcv_nxt, 1001);

    // Step 2: Receive ACK for our SYN+ACK
    let ack_seg = TcpSegment {
        seqno: 1001,
        ackno: state.rod.iss + 1,
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
    assert!(result.is_ok(), "ROD ACK processing failed");

    let result = state.flow_ctrl.on_ack_in_synrcvd(&ack_seg);
    assert!(result.is_ok(), "FlowControl ACK processing failed");

    let result = state.cong_ctrl.on_ack_in_synrcvd();
    assert!(result.is_ok(), "CongControl ACK processing failed");

    let result = state.conn_mgmt.on_ack_in_synrcvd();
    assert!(result.is_ok(), "ConnMgmt ACK processing failed");

    assert_eq!(state.conn_mgmt.state, TcpState::Established);
}

#[test]
fn test_three_way_handshake_active() {
    // Simulate active open (client side)
    let mut state = TcpConnectionState::new();
    state.conn_mgmt.state = TcpState::SynSent;
    state.rod.iss = 5000;
    state.rod.snd_nxt = 5000;
    state.conn_mgmt.mss = 1460;

    // Step 1: Receive SYN+ACK
    let synack_seg = TcpSegment {
        seqno: 2000,
        ackno: 5001,  // ACKing our SYN
        flags: TcpFlags {
            syn: true,
            ack: true,
            fin: false,
            rst: false,
            psh: false,
            urg: false,
        },
        wnd: 16384,
        tcphdr_len: 20,
        payload_len: 0,
    };

    // Use component methods
    let result = state.rod.on_synack_in_synsent(&synack_seg);
    assert!(result.is_ok(), "ROD SYN+ACK processing failed");

    let result = state.flow_ctrl.on_synack_in_synsent(&synack_seg);
    assert!(result.is_ok(), "FlowControl SYN+ACK processing failed");

    let result = state.cong_ctrl.on_synack_in_synsent(&state.conn_mgmt);
    assert!(result.is_ok(), "CongControl SYN+ACK processing failed");

    let result = state.conn_mgmt.on_synack_in_synsent();
    assert!(result.is_ok(), "ConnMgmt SYN+ACK processing failed");

    assert_eq!(state.conn_mgmt.state, TcpState::Established);
    assert_eq!(state.rod.rcv_nxt, 2001);
    assert_eq!(state.rod.lastack, 5001);
}

#[test]
fn test_reset_handling() {
    let mut state = TcpConnectionState::new();
    state.conn_mgmt.state = TcpState::Established;

    // Use component methods
    let _ = state.rod.on_rst();
    let _ = state.flow_ctrl.on_rst();
    let _ = state.cong_ctrl.on_rst();
    let _ = state.conn_mgmt.on_rst();

    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
}

#[test]
fn test_state_initialization() {
    let state = TcpConnectionState::new();

    assert_eq!(state.conn_mgmt.state, TcpState::Closed);
    assert_eq!(state.conn_mgmt.mss, 536);
    assert_eq!(state.conn_mgmt.ttl, 255);
    assert_eq!(state.rod.rto, 3000);
    assert_eq!(state.cong_ctrl.ssthresh, 0xFFFF);
}

#[test]
fn test_congestion_window_initialization() {
    let mut state = TcpConnectionState::new();
    state.conn_mgmt.state = TcpState::Listen;
    state.conn_mgmt.mss = 1460;

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

    let remote_ip = unsafe { core::mem::zeroed() };

    // Use component methods
    let _ = state.rod.on_syn_in_listen(&syn_seg);
    let _ = state.flow_ctrl.on_syn_in_listen(&syn_seg, &state.conn_mgmt);
    let _ = state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt);
    let _ = state.conn_mgmt.on_syn_in_listen(remote_ip, 12345);

    // RFC 5681: IW = min(4*MSS, max(2*MSS, 4380))
    // With MSS=1460: min(5840, max(2920, 4380)) = min(5840, 4380) = 4380
    assert_eq!(state.cong_ctrl.cwnd, 4380);
}
