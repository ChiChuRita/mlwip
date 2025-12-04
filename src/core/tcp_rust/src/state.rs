//! TCP Connection State
//!
//! This module provides the complete TCP connection state by aggregating
//! the five disjoint state components from the components module.

// Re-export components for backwards compatibility
pub use crate::components::{
    ConnectionManagementState,
    ReliableOrderedDeliveryState,
    FlowControlState,
    CongestionControlState,
    DemuxState,
};

/// TCP State Machine States
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TcpState {
    Closed = 0,
    Listen = 1,
    SynSent = 2,
    SynRcvd = 3,
    Established = 4,
    FinWait1 = 5,
    FinWait2 = 6,
    CloseWait = 7,
    Closing = 8,
    LastAck = 9,
    TimeWait = 10,
}

impl TcpState {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            0 => Some(TcpState::Closed),
            1 => Some(TcpState::Listen),
            2 => Some(TcpState::SynSent),
            3 => Some(TcpState::SynRcvd),
            4 => Some(TcpState::Established),
            5 => Some(TcpState::FinWait1),
            6 => Some(TcpState::FinWait2),
            7 => Some(TcpState::CloseWait),
            8 => Some(TcpState::Closing),
            9 => Some(TcpState::LastAck),
            10 => Some(TcpState::TimeWait),
            _ => None,
        }
    }

    pub fn is_closing(&self) -> bool {
        *self >= TcpState::FinWait1
    }
}

/// Complete TCP Connection State
///
/// Aggregates all five state components.
/// This structure enforces the separation of concerns.
pub struct TcpConnectionState {
    pub conn_mgmt: ConnectionManagementState,
    pub rod: ReliableOrderedDeliveryState,
    pub flow_ctrl: FlowControlState,
    pub cong_ctrl: CongestionControlState,
    pub demux: DemuxState,

    pub callback_arg: *mut core::ffi::c_void,
    pub recv_callback: Option<unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, *mut core::ffi::c_void, i8) -> i8>,
    pub sent_callback: Option<unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, u16) -> i8>,
    pub err_callback: Option<unsafe extern "C" fn(*mut core::ffi::c_void, i8)>,
    pub connected_callback: Option<unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, i8) -> i8>,
    pub poll_callback: Option<unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void) -> i8>,
    pub accept_callback: Option<unsafe extern "C" fn(*mut core::ffi::c_void, *mut core::ffi::c_void, i8) -> i8>,
    pub poll_interval: u8,
}

impl TcpConnectionState {
    pub fn new() -> Self {
        Self {
            conn_mgmt: ConnectionManagementState::new(),
            rod: ReliableOrderedDeliveryState::new(),
            flow_ctrl: FlowControlState::new(),
            cong_ctrl: CongestionControlState::new(),
            demux: DemuxState::new(),
            callback_arg: core::ptr::null_mut(),
            recv_callback: None,
            sent_callback: None,
            err_callback: None,
            connected_callback: None,
            poll_callback: None,
            accept_callback: None,
            poll_interval: 0,
        }
    }
}
