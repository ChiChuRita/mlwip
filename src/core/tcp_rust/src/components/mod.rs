//! TCP State Components
//!
//! This module contains the five disjoint TCP state components:
//! 1. Connection Management - TCP state machine and connection lifecycle
//! 2. Reliable Ordered Delivery - Sequence numbers, ACKs, retransmissions
//! 3. Flow Control - Receive and send windows
//! 4. Congestion Control - Congestion window and slow start
//! 5. Demultiplexing - Connection identification (uses 4-tuple from ConnMgmt)

mod connection_mgmt;
mod rod;
mod flow_control;
mod congestion_control;

pub use connection_mgmt::ConnectionManagementState;
pub use rod::ReliableOrderedDeliveryState;
pub use flow_control::FlowControlState;
pub use congestion_control::CongestionControlState;

/// Demultiplexing State
///
/// Currently empty - demuxing uses the 4-tuple from ConnectionManagementState.
/// Included for completeness as per design document.
pub struct DemuxState {
    // Empty by design
}

impl DemuxState {
    pub fn new() -> Self {
        Self {}
    }
}
