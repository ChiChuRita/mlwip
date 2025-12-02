//! Test Helper Functions for TCP Control Path Tests
//!
//! This module provides utilities for creating test segments, mock network interfaces,
//! and other testing infrastructure.

use lwip_tcp_rust::state::{TcpConnectionState, TcpState};
use lwip_tcp_rust::tcp_proto;
use lwip_tcp_rust::ffi;
use core::sync::atomic::{AtomicU32, Ordering};

/// Test IP addresses (matching lwIP test suite)
pub const TEST_LOCAL_IP: u32 = 0xC0A80001; // 192.168.0.1
pub const TEST_REMOTE_IP: u32 = 0xC0A80002; // 192.168.0.2
pub const TEST_LOCAL_PORT: u16 = 0x101;
pub const TEST_REMOTE_PORT: u16 = 0x100;

/// Counters for tracking test events
#[derive(Debug, Default)]
pub struct TestCounters {
    pub recv_calls: u32,
    pub recved_bytes: u32,
    pub recv_calls_after_close: u32,
    pub recved_bytes_after_close: u32,
    pub close_calls: u32,
    pub err_calls: u32,
    pub last_err: i8,
}

/// Counters for tracking TX events
#[derive(Debug, Default)]
pub struct TestTxCounters {
    pub num_tx_calls: u32,
    pub num_tx_bytes: u32,
    pub tx_segments: Vec<TestSegment>,
}

/// A test TCP segment
#[derive(Debug, Clone)]
pub struct TestSegment {
    pub src_ip: u32,
    pub dst_ip: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub seqno: u32,
    pub ackno: u32,
    pub flags: u8,
    pub window: u16,
    pub data: Vec<u8>,
}

impl TestSegment {
    /// Create a new test segment
    pub fn new(
        src_ip: u32,
        dst_ip: u32,
        src_port: u16,
        dst_port: u16,
        seqno: u32,
        ackno: u32,
        flags: u8,
        window: u16,
        data: &[u8],
    ) -> Self {
        Self {
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            seqno,
            ackno,
            flags,
            window,
            data: data.to_vec(),
        }
    }

    /// Check if segment has SYN flag
    pub fn has_syn(&self) -> bool {
        (self.flags & tcp_proto::TCP_SYN) != 0
    }

    /// Check if segment has ACK flag
    pub fn has_ack(&self) -> bool {
        (self.flags & tcp_proto::TCP_ACK) != 0
    }

    /// Check if segment has FIN flag
    pub fn has_fin(&self) -> bool {
        (self.flags & tcp_proto::TCP_FIN) != 0
    }

    /// Check if segment has RST flag
    pub fn has_rst(&self) -> bool {
        (self.flags & tcp_proto::TCP_RST) != 0
    }
}

/// Create a segment for testing RX path
pub fn create_rx_segment(
    state: &TcpConnectionState,
    data: &[u8],
    seqno_offset: i32,
    ackno_offset: i32,
    flags: u8,
    window: u16,
) -> TestSegment {
    let seqno = if seqno_offset >= 0 {
        state.rod.rcv_nxt.wrapping_add(seqno_offset as u32)
    } else {
        state.rod.rcv_nxt.wrapping_sub((-seqno_offset) as u32)
    };

    let ackno = if ackno_offset >= 0 {
        state.rod.snd_nxt.wrapping_add(ackno_offset as u32)
    } else {
        state.rod.snd_nxt.wrapping_sub((-ackno_offset) as u32)
    };

    TestSegment::new(
        state.conn_mgmt.remote_ip.addr,
        state.conn_mgmt.local_ip.addr,
        state.conn_mgmt.remote_port,
        state.conn_mgmt.local_port,
        seqno,
        ackno,
        flags,
        window,
        data,
    )
}

/// Create a generic segment
pub fn create_segment(
    src_ip: u32,
    dst_ip: u32,
    src_port: u16,
    dst_port: u16,
    data: &[u8],
    seqno: u32,
    ackno: u32,
    flags: u8,
) -> TestSegment {
    TestSegment::new(src_ip, dst_ip, src_port, dst_port, seqno, ackno, flags, 8192, data)
}

/// Initialize a TCP connection state for testing
pub fn create_test_state() -> TcpConnectionState {
    let mut state = TcpConnectionState::new();
    
    // Set up basic connection parameters
    state.conn_mgmt.local_ip.addr = TEST_LOCAL_IP;
    state.conn_mgmt.remote_ip.addr = TEST_REMOTE_IP;
    state.conn_mgmt.local_port = TEST_LOCAL_PORT;
    state.conn_mgmt.remote_port = TEST_REMOTE_PORT;
    state.conn_mgmt.mss = 536;
    
    state
}

/// Set TCP state and initialize sequence numbers
pub fn set_tcp_state(
    state: &mut TcpConnectionState,
    tcp_state: TcpState,
    local_ip: u32,
    remote_ip: u32,
    local_port: u16,
    remote_port: u16,
) {
    state.conn_mgmt.state = tcp_state;
    state.conn_mgmt.local_ip.addr = local_ip;
    state.conn_mgmt.remote_ip.addr = remote_ip;
    state.conn_mgmt.local_port = local_port;
    state.conn_mgmt.remote_port = remote_port;

    // Initialize sequence numbers for ESTABLISHED state
    if tcp_state == TcpState::Established {
        state.rod.iss = 1000;
        state.rod.snd_nxt = 1001;
        state.rod.lastack = 1001;
        state.rod.irs = 2000;
        state.rod.rcv_nxt = 2001;
        
        state.flow_ctrl.snd_wnd = 8192;
        state.flow_ctrl.rcv_wnd = 8192;
        state.cong_ctrl.cwnd = 4 * state.conn_mgmt.mss as u16;
    }
}

/// Global ISS counter for testing (mimics tcp_next_iss)
static TEST_ISS: AtomicU32 = AtomicU32::new(6510);

/// Generate next ISS for testing
pub fn next_iss() -> u32 {
    TEST_ISS.fetch_add(1, Ordering::SeqCst)
}

/// Reset ISS to default value
pub fn reset_iss() {
    TEST_ISS.store(6510, Ordering::SeqCst);
}

/// Mock TX function that captures sent segments
pub struct MockTxCapture {
    pub segments: Vec<TestSegment>,
}

impl MockTxCapture {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn capture_segment(&mut self, seg: TestSegment) {
        self.segments.push(seg);
    }

    pub fn clear(&mut self) {
        self.segments.clear();
    }

    pub fn count(&self) -> usize {
        self.segments.len()
    }

    pub fn last(&self) -> Option<&TestSegment> {
        self.segments.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_state() {
        let state = create_test_state();
        assert_eq!(state.conn_mgmt.local_ip.addr, TEST_LOCAL_IP);
        assert_eq!(state.conn_mgmt.remote_ip.addr, TEST_REMOTE_IP);
        assert_eq!(state.conn_mgmt.local_port, TEST_LOCAL_PORT);
        assert_eq!(state.conn_mgmt.remote_port, TEST_REMOTE_PORT);
    }

    #[test]
    fn test_segment_flags() {
        let seg = TestSegment::new(
            TEST_REMOTE_IP,
            TEST_LOCAL_IP,
            TEST_REMOTE_PORT,
            TEST_LOCAL_PORT,
            1000,
            2000,
            tcp_proto::TCP_SYN | tcp_proto::TCP_ACK,
            8192,
            &[],
        );

        assert!(seg.has_syn());
        assert!(seg.has_ack());
        assert!(!seg.has_fin());
        assert!(!seg.has_rst());
    }

    #[test]
    fn test_set_tcp_state() {
        let mut state = create_test_state();
        set_tcp_state(
            &mut state,
            TcpState::Established,
            TEST_LOCAL_IP,
            TEST_REMOTE_IP,
            TEST_LOCAL_PORT,
            TEST_REMOTE_PORT,
        );

        assert_eq!(state.conn_mgmt.state, TcpState::Established);
        assert_eq!(state.rod.snd_nxt, 1001);
        assert_eq!(state.rod.rcv_nxt, 2001);
    }
}
