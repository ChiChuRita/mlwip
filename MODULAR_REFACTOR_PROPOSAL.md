# Modular TCP Refactoring Proposal
## Component-Specific State Methods (No Separate Control Path)

**Date:** November 18, 2025
**Author:** Based on Professor's Feedback
**Status:** Proposal - Awaiting Implementation

---

## Executive Summary

**Current Approach:**
- Control path functions (e.g., `process_syn_in_listen()`) have write access to **all state components**
- These functions modify ROD, Flow Control, Congestion Control, and Connection Management state
- This creates a "privileged" control path with special permissions

**Proposed Approach:**
- **Eliminate the privileged control path** entirely
- Each state component provides **component-specific methods** for each relevant TCP state
- The dispatcher (`input_listen()`, etc.) orchestrates calling these methods in sequence
- **No function can write to multiple components** - complete modular separation

**Benefits:**
1. ✅ **Pure modularity** - No special-case "control path can write everything"
2. ✅ **Cleaner separation** - Even state transitions respect component boundaries
3. ✅ **Simpler reasoning** - Each component owns only its own state
4. ✅ **Better testability** - Test each component's state handling independently
5. ✅ **Future-proof** - Easier to swap/extend components (e.g., different CC algorithms)

---

## Current Architecture (To Be Replaced)

### Current: Control Path Has Special Privileges

```rust
// CURRENT: process_syn_in_listen() writes to EVERYTHING
pub fn process_syn_in_listen(
    state: &mut TcpConnectionState,  // ← Writes to ALL components
    seg: &TcpSegment,
    remote_ip: ip_addr_t,
    remote_port: u16,
) -> Result<(), &'static str> {
    // Writes to Connection Management
    state.conn_mgmt.remote_ip = remote_ip;
    state.conn_mgmt.remote_port = remote_port;
    state.conn_mgmt.state = TcpState::SynRcvd;

    // Writes to ROD
    state.rod.irs = seg.seqno;
    state.rod.rcv_nxt = seg.seqno.wrapping_add(1);
    state.rod.iss = generate_iss();
    state.rod.snd_nxt = state.rod.iss;

    // Writes to Flow Control
    state.flow_ctrl.snd_wnd = seg.wnd;
    state.flow_ctrl.rcv_wnd = 4096;

    // Writes to Congestion Control
    state.cong_ctrl.cwnd = ...;

    Ok(())
}
```

**Problem:** One function touches 4 different components! This violates modular boundaries.

---

## Proposed Architecture

### Key Principle: Component-Specific Methods

Each component provides **state-specific methods** that:
1. Take `&mut` to **only their own state**
2. Take `&` (read-only) references to other components if needed
3. Return updates/events to the caller

### Example: Handling SYN in LISTEN State

```rust
// NEW APPROACH: Each component handles its own state

// 1. ROD component handles sequence number initialization
impl ReliableOrderedDeliveryState {
    /// Handle SYN reception in LISTEN state
    /// Only modifies ROD state
    pub fn on_syn_in_listen(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Store peer's initial sequence number
        self.irs = seg.seqno;
        self.rcv_nxt = seg.seqno.wrapping_add(1);

        // Generate our initial sequence number
        self.iss = Self::generate_iss();
        self.snd_nxt = self.iss;
        self.snd_lbb = self.iss;
        self.lastack = self.iss;

        Ok(())
    }
}

// 2. Flow Control component handles window initialization
impl FlowControlState {
    /// Handle SYN reception in LISTEN state
    /// Only modifies FC state
    pub fn on_syn_in_listen(
        &mut self,
        seg: &TcpSegment,
        conn_mgmt: &ConnectionManagementState,  // Read-only for MSS
    ) -> Result<(), &'static str> {
        // Store peer's advertised window
        self.snd_wnd = seg.wnd;
        self.snd_wnd_max = seg.wnd;

        // Initialize our receive window
        self.rcv_wnd = 4096;  // TODO: Base on actual buffer
        self.rcv_ann_wnd = self.rcv_wnd;

        Ok(())
    }
}

// 3. Congestion Control component handles cwnd initialization
impl CongestionControlState {
    /// Handle SYN reception in LISTEN state
    /// Only modifies CC state
    pub fn on_syn_in_listen(
        &mut self,
        conn_mgmt: &ConnectionManagementState,  // Read-only for MSS
    ) -> Result<(), &'static str> {
        // RFC 5681: IW = min(4*MSS, max(2*MSS, 4380 bytes))
        let mss = conn_mgmt.mss as u16;
        self.cwnd = core::cmp::min(4 * mss, core::cmp::max(2 * mss, 4380));
        self.ssthresh = 0xFFFF;

        Ok(())
    }
}

// 4. Connection Management handles state transition and endpoint storage
impl ConnectionManagementState {
    /// Handle SYN reception in LISTEN state
    /// Only modifies Connection Management state
    pub fn on_syn_in_listen(
        &mut self,
        remote_ip: ip_addr_t,
        remote_port: u16,
    ) -> Result<(), &'static str> {
        // Validate current state
        if self.state != TcpState::Listen {
            return Err("Not in LISTEN state");
        }

        // Store remote endpoint
        self.remote_ip = remote_ip;
        self.remote_port = remote_port;

        // Transition to SYN_RCVD
        self.state = TcpState::SynRcvd;

        Ok(())
    }
}

// 5. Dispatcher orchestrates calling all components
impl TcpInput {
    fn input_listen(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
        remote_ip: ip_addr_t,
        remote_port: u16,
    ) -> Result<InputAction, &'static str> {
        // Only accept SYN in LISTEN
        if !seg.flags.syn || seg.flags.ack {
            return Ok(InputAction::SendRst);
        }

        // Call each component's handler in sequence
        // Each component only modifies its own state
        state.rod.on_syn_in_listen(seg)?;
        state.flow_ctrl.on_syn_in_listen(seg, &state.conn_mgmt)?;
        state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;
        state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;

        // Send SYN+ACK
        Ok(InputAction::SendSynAck)
    }
}
```

---

## Complete Modular Separation

### No More "Control Path Exception"

**Old Model:**
```
┌─────────────────────────────────────────┐
│      Control Path (Special Case)        │
│  Can write to ALL components            │
└─────────────────────────────────────────┘
            │
            ├─► ConnectionManagementState
            ├─► ReliableOrderedDeliveryState
            ├─► FlowControlState
            └─► CongestionControlState
```

**New Model:**
```
┌──────────────────────────────────────────────────┐
│              Input Dispatcher                     │
│  (Orchestrates, but cannot write to any state)   │
└──────────────────────────────────────────────────┘
            │
            ├─► ROD::on_syn_in_listen(&mut rod, ...)
            ├─► FlowControl::on_syn_in_listen(&mut fc, ...)
            ├─► CongControl::on_syn_in_listen(&mut cc, ...)
            └─► ConnMgmt::on_syn_in_listen(&mut conn, ...)
```

**Key Difference:** The dispatcher **never writes state directly**. It only calls component methods.

---

## Implementation Strategy

### Phase 1: Define Component Method Signatures

For each TCP state and each component, define what needs to happen:

| TCP State | ROD Method | FC Method | CC Method | Conn Mgmt Method |
|-----------|------------|-----------|-----------|------------------|
| **LISTEN → SYN_RCVD** | `on_syn_in_listen()` | `on_syn_in_listen()` | `on_syn_in_listen()` | `on_syn_in_listen()` |
| **SYN_SENT → ESTABLISHED** | `on_synack_in_synsent()` | `on_synack_in_synsent()` | `on_synack_in_synsent()` | `on_synack_in_synsent()` |
| **SYN_RCVD → ESTABLISHED** | `on_ack_in_synrcvd()` | `on_ack_in_synrcvd()` | (no-op) | `on_ack_in_synrcvd()` |
| **ESTABLISHED (recv FIN)** | `on_fin_in_established()` | (no-op) | (no-op) | `on_fin_in_established()` |
| **FIN_WAIT_1 → FIN_WAIT_2** | `on_ack_in_finwait1()` | (no-op) | (no-op) | `on_ack_in_finwait1()` |
| ... | ... | ... | ... | ... |

**Note:** Some components may have no-ops for certain states (e.g., CC doesn't change on FIN).

### Phase 2: Implement Component Methods

#### Example: ROD Component

```rust
// src/core/tcp_rust/src/rod.rs (NEW FILE)

impl ReliableOrderedDeliveryState {
    // ========================================================================
    // State-Specific Event Handlers
    // ========================================================================

    /// LISTEN → SYN_RCVD: Initialize sequence numbers
    pub fn on_syn_in_listen(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        self.irs = seg.seqno;
        self.rcv_nxt = seg.seqno.wrapping_add(1);
        self.iss = Self::generate_iss();
        self.snd_nxt = self.iss;
        self.snd_lbb = self.iss;
        self.lastack = self.iss;
        Ok(())
    }

    /// SYN_SENT → ESTABLISHED: Process SYN+ACK
    pub fn on_synack_in_synsent(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Validate ACK number
        if seg.ackno != self.iss.wrapping_add(1) {
            return Err("Invalid ACK number");
        }

        self.irs = seg.seqno;
        self.rcv_nxt = seg.seqno.wrapping_add(1);
        self.snd_nxt = self.iss.wrapping_add(1);
        self.lastack = seg.ackno;
        Ok(())
    }

    /// SYN_RCVD → ESTABLISHED: Process ACK of our SYN
    pub fn on_ack_in_synrcvd(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Validate ACK number
        if seg.ackno != self.iss.wrapping_add(1) {
            return Err("Invalid ACK number");
        }

        self.snd_nxt = self.iss.wrapping_add(1);
        self.lastack = seg.ackno;
        Ok(())
    }

    /// ESTABLISHED: Process FIN
    pub fn on_fin_in_established(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        // Validate sequence number
        if seg.seqno != self.rcv_nxt {
            return Err("Invalid sequence number for FIN");
        }

        // FIN consumes one sequence number
        self.rcv_nxt = self.rcv_nxt.wrapping_add(1);
        Ok(())
    }

    /// FIN_WAIT_1 → FIN_WAIT_2: ACK of our FIN
    pub fn on_ack_in_finwait1(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        let expected_ack = self.snd_nxt.wrapping_add(1);
        if seg.ackno != expected_ack {
            return Err("ACK doesn't acknowledge our FIN");
        }

        self.lastack = seg.ackno;
        Ok(())
    }

    // ... more state handlers ...

    // ========================================================================
    // Helper Functions
    // ========================================================================

    fn generate_iss() -> u32 {
        unsafe {
            static mut ISS_COUNTER: u32 = 0;
            ISS_COUNTER = ISS_COUNTER.wrapping_add(1);
            ISS_COUNTER
        }
    }

    /// Validate sequence number (read-only check)
    pub fn validate_sequence_number(
        &self,
        seg: &TcpSegment,
        rcv_wnd: u16,  // From FC component
    ) -> bool {
        let seqno = seg.seqno;

        // Special case: zero window
        if rcv_wnd == 0 {
            return seqno == self.rcv_nxt;
        }

        // Check if segment overlaps with receive window
        let seg_end = seqno.wrapping_add(seg.payload_len as u32);
        Self::seq_in_window(seqno, self.rcv_nxt, rcv_wnd) ||
            (seg.payload_len > 0 && Self::seq_in_window(seg_end.wrapping_sub(1), self.rcv_nxt, rcv_wnd))
    }

    fn seq_in_window(seq: u32, rcv_nxt: u32, rcv_wnd: u16) -> bool {
        let diff = seq.wrapping_sub(rcv_nxt);
        diff < rcv_wnd as u32
    }
}
```

#### Example: Flow Control Component

```rust
// src/core/tcp_rust/src/flow_control.rs (NEW FILE)

impl FlowControlState {
    /// LISTEN → SYN_RCVD: Initialize windows
    pub fn on_syn_in_listen(
        &mut self,
        seg: &TcpSegment,
        _conn_mgmt: &ConnectionManagementState,
    ) -> Result<(), &'static str> {
        self.snd_wnd = seg.wnd;
        self.snd_wnd_max = seg.wnd;
        self.rcv_wnd = 4096;
        self.rcv_ann_wnd = self.rcv_wnd;
        Ok(())
    }

    /// SYN_SENT → ESTABLISHED: Store peer's window
    pub fn on_synack_in_synsent(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        self.snd_wnd = seg.wnd;
        self.snd_wnd_max = seg.wnd;
        Ok(())
    }

    /// SYN_RCVD → ESTABLISHED: Update peer's window
    pub fn on_ack_in_synrcvd(
        &mut self,
        seg: &TcpSegment,
    ) -> Result<(), &'static str> {
        self.snd_wnd = seg.wnd;
        Ok(())
    }

    // No-op for states where FC doesn't change
    pub fn on_fin_in_established(&mut self) -> Result<(), &'static str> {
        Ok(())  // FC state doesn't change on FIN
    }
}
```

#### Example: Congestion Control Component

```rust
// src/core/tcp_rust/src/congestion_control.rs (NEW FILE)

impl CongestionControlState {
    /// LISTEN → SYN_RCVD: Initialize cwnd
    pub fn on_syn_in_listen(
        &mut self,
        conn_mgmt: &ConnectionManagementState,
    ) -> Result<(), &'static str> {
        let mss = conn_mgmt.mss as u16;
        self.cwnd = core::cmp::min(4 * mss, core::cmp::max(2 * mss, 4380));
        self.ssthresh = 0xFFFF;
        Ok(())
    }

    /// SYN_SENT → ESTABLISHED: Initialize cwnd (active open)
    pub fn on_synack_in_synsent(
        &mut self,
        conn_mgmt: &ConnectionManagementState,
    ) -> Result<(), &'static str> {
        let mss = conn_mgmt.mss as u16;
        self.cwnd = mss;
        self.ssthresh = 0xFFFF;
        Ok(())
    }

    // No-op for many states
    pub fn on_ack_in_synrcvd(&mut self) -> Result<(), &'static str> {
        Ok(())  // CC doesn't change
    }

    pub fn on_fin_in_established(&mut self) -> Result<(), &'static str> {
        Ok(())  // CC doesn't change on FIN
    }
}
```

#### Example: Connection Management Component

```rust
// src/core/tcp_rust/src/connection_mgmt.rs (NEW FILE)

impl ConnectionManagementState {
    /// LISTEN → SYN_RCVD: Store remote endpoint and transition
    pub fn on_syn_in_listen(
        &mut self,
        remote_ip: ip_addr_t,
        remote_port: u16,
    ) -> Result<(), &'static str> {
        if self.state != TcpState::Listen {
            return Err("Not in LISTEN state");
        }

        self.remote_ip = remote_ip;
        self.remote_port = remote_port;
        self.state = TcpState::SynRcvd;
        Ok(())
    }

    /// SYN_SENT → ESTABLISHED: Transition to established
    pub fn on_synack_in_synsent(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::SynSent {
            return Err("Not in SYN_SENT state");
        }

        self.state = TcpState::Established;
        Ok(())
    }

    /// SYN_RCVD → ESTABLISHED: Transition to established
    pub fn on_ack_in_synrcvd(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::SynRcvd {
            return Err("Not in SYN_RCVD state");
        }

        self.state = TcpState::Established;
        Ok(())
    }

    /// ESTABLISHED → CLOSE_WAIT: Process FIN
    pub fn on_fin_in_established(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Established {
            return Err("Not in ESTABLISHED state");
        }

        self.state = TcpState::CloseWait;
        Ok(())
    }

    /// ESTABLISHED → FIN_WAIT_1: Initiate close
    pub fn on_close_in_established(&mut self) -> Result<(), &'static str> {
        if self.state != TcpState::Established {
            return Err("Not in ESTABLISHED state");
        }

        self.state = TcpState::FinWait1;
        Ok(())
    }

    // ... more state transitions ...
}
```

### Phase 3: Refactor Input Dispatchers

```rust
// src/core/tcp_rust/src/tcp_in.rs (REFACTORED)

impl TcpInput {
    /// Input processing for LISTEN state
    fn input_listen(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
        remote_ip: ip_addr_t,
        remote_port: u16,
    ) -> Result<InputAction, &'static str> {
        // Only accept SYN in LISTEN
        if !seg.flags.syn || seg.flags.ack {
            return Ok(InputAction::SendRst);
        }

        // Call each component's handler
        // Note: Order matters! ROD/FC/CC first, then ConnMgmt (state transition)
        state.rod.on_syn_in_listen(seg)?;
        state.flow_ctrl.on_syn_in_listen(seg, &state.conn_mgmt)?;
        state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;
        state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;

        Ok(InputAction::SendSynAck)
    }

    /// Input processing for SYN_SENT state
    fn input_synsent(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Expecting SYN+ACK
        if !seg.flags.syn || !seg.flags.ack {
            return Ok(InputAction::Drop);
        }

        // Call each component's handler
        state.rod.on_synack_in_synsent(seg)?;
        state.flow_ctrl.on_synack_in_synsent(seg)?;
        state.cong_ctrl.on_synack_in_synsent(&state.conn_mgmt)?;
        state.conn_mgmt.on_synack_in_synsent()?;

        Ok(InputAction::SendAck)
    }

    /// Input processing for SYN_RCVD state
    fn input_synrcvd(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
            return Ok(InputAction::Drop);
        }

        // Expecting ACK of our SYN
        if !seg.flags.ack {
            return Ok(InputAction::Drop);
        }

        // Call each component's handler
        state.rod.on_ack_in_synrcvd(seg)?;
        state.flow_ctrl.on_ack_in_synrcvd(seg)?;
        state.cong_ctrl.on_ack_in_synrcvd()?;
        state.conn_mgmt.on_ack_in_synrcvd()?;

        Ok(InputAction::Accept)
    }

    /// Input processing for ESTABLISHED state
    fn input_established(
        state: &mut TcpConnectionState,
        seg: &TcpSegment,
    ) -> Result<InputAction, &'static str> {
        // Validate sequence number
        if !state.rod.validate_sequence_number(seg, state.flow_ctrl.rcv_wnd) {
            return Ok(InputAction::Drop);
        }

        // Check for FIN
        if seg.flags.fin {
            state.rod.on_fin_in_established(seg)?;
            state.flow_ctrl.on_fin_in_established()?;
            state.cong_ctrl.on_fin_in_established()?;
            state.conn_mgmt.on_fin_in_established()?;
            return Ok(InputAction::SendAck);
        }

        // Handle data/ACKs (future: data path handlers)
        Ok(InputAction::Accept)
    }
}
```

---

## File Organization

### New Module Structure

```
src/core/tcp_rust/src/
├── lib.rs                          # Public API, FFI exports
├── ffi.rs                          # FFI type definitions
├── tcp_proto.rs                    # Protocol constants
├── state.rs                        # State structure definitions
│
├── components/                     # NEW: Component implementations
│   ├── mod.rs
│   ├── rod.rs                     # ROD methods (on_syn_in_listen, etc.)
│   ├── flow_control.rs            # FC methods
│   ├── congestion_control.rs     # CC methods
│   └── connection_mgmt.rs        # Conn Mgmt methods
│
├── tcp_in.rs                      # Input dispatcher (orchestrates components)
├── tcp_out.rs                     # Output path (segment transmission)
│
└── validation.rs                  # Validation helpers (RFC 5961, etc.)
```

### Key Changes:

1. **Eliminate `control_path.rs`** - No longer needed
2. **Create `components/` directory** - Each component in its own file
3. **Dispatcher remains** - But only orchestrates, never writes state

---

## Benefits Analysis

### 1. Complete Modular Separation ✅

**Before:**
- Control path: Can write to all components (special case)
- Data path: Each component writes only its own state

**After:**
- ALL code: Each component writes only its own state (no exceptions!)

### 2. Simpler Mental Model ✅

**Before:**
```rust
// Where does state transition happen?
// Answer: In control_path.rs, which also handles ROD, FC, CC
```

**After:**
```rust
// Where does state transition happen?
// Answer: In ConnectionManagementState::on_*() methods
// Where do sequence numbers update?
// Answer: In ReliableOrderedDeliveryState::on_*() methods
```

Clear 1:1 mapping between state and component.

### 3. Better Testability ✅

**Before:**
```rust
#[test]
fn test_handshake() {
    // Must test entire control path at once
    // Hard to test ROD logic independently
}
```

**After:**
```rust
#[test]
fn test_rod_syn_handling() {
    let mut rod = ReliableOrderedDeliveryState::new();
    let seg = TcpSegment { seqno: 1000, ... };

    rod.on_syn_in_listen(&seg).unwrap();

    assert_eq!(rod.irs, 1000);
    assert_eq!(rod.rcv_nxt, 1001);
    // Test ONLY ROD logic, in isolation
}

#[test]
fn test_fc_syn_handling() {
    let mut fc = FlowControlState::new();
    let seg = TcpSegment { wnd: 8192, ... };

    fc.on_syn_in_listen(&seg, &conn_mgmt).unwrap();

    assert_eq!(fc.snd_wnd, 8192);
    // Test ONLY FC logic, in isolation
}
```

### 4. Future Data Path Consistency ✅

**Before:**
- Control path: Special rules (writes everything)
- Data path: Component rules (writes only own state)

**After:**
- Control path: Same rules as data path
- Data path: Same rules as control path
- **No distinction between control and data path!**

### 5. Easier Component Replacement ✅

Want to swap congestion control algorithms?

**Before:**
```rust
// Must find all places control path touches CC state
// Must ensure new algorithm works with control path
```

**After:**
```rust
// Implement new CongestionControlState with same on_*() methods
// Swap it in - guaranteed to work because interface is identical
```

---

## Migration Path

### Step 1: Create Component Method Stubs

Add empty/stub methods to existing state structs:

```rust
// In state.rs, add impl blocks

impl ReliableOrderedDeliveryState {
    pub fn on_syn_in_listen(&mut self, seg: &TcpSegment) -> Result<(), &'static str> {
        unimplemented!("TODO")
    }
    // ... more stubs
}

impl FlowControlState {
    pub fn on_syn_in_listen(&mut self, seg: &TcpSegment, conn: &ConnectionManagementState) -> Result<(), &'static str> {
        unimplemented!("TODO")
    }
    // ... more stubs
}

// etc.
```

### Step 2: Move Logic from control_path.rs

Take each function in `control_path.rs` and split it into component methods:

```rust
// OLD: control_path.rs
pub fn process_syn_in_listen(state: &mut TcpConnectionState, ...) {
    state.rod.irs = seg.seqno;           // ← Move to ROD::on_syn_in_listen()
    state.flow_ctrl.snd_wnd = seg.wnd;   // ← Move to FC::on_syn_in_listen()
    state.cong_ctrl.cwnd = ...;          // ← Move to CC::on_syn_in_listen()
    state.conn_mgmt.state = SynRcvd;     // ← Move to ConnMgmt::on_syn_in_listen()
}
```

### Step 3: Refactor Dispatchers

Update `tcp_in.rs` dispatchers to call component methods:

```rust
// OLD
ControlPath::process_syn_in_listen(state, seg, ...)?;

// NEW
state.rod.on_syn_in_listen(seg)?;
state.flow_ctrl.on_syn_in_listen(seg, &state.conn_mgmt)?;
state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;
state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;
```

### Step 4: Move Component Implementations to Separate Files

Once methods are working, reorganize:

```bash
mv state.rs state_definitions.rs
mkdir components/
# Move impl blocks to components/*.rs
# Create components/mod.rs
```

### Step 5: Delete control_path.rs

Once all logic is moved, delete the old control path file.

### Step 6: Update Tests

Update tests to use component methods directly:

```rust
// OLD
let result = ControlPath::process_syn_in_listen(&mut state, ...);

// NEW
state.rod.on_syn_in_listen(&seg)?;
state.flow_ctrl.on_syn_in_listen(&seg, &state.conn_mgmt)?;
state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;
state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;
```

---

## Handling Edge Cases

### What if multiple components need to coordinate?

**Example:** ACK validation needs both ROD and FC state

**Solution:** Use read-only references

```rust
impl ReliableOrderedDeliveryState {
    /// Validate ACK number
    pub fn validate_ack(
        &self,
        seg: &TcpSegment,
        flow_ctrl: &FlowControlState,  // Read-only
    ) -> AckValidation {
        // Can read FC state for validation
        // But cannot modify FC state
        // ...
    }
}
```

### What if order matters?

**Example:** State transition must happen AFTER all other updates

**Solution:** Dispatcher controls order

```rust
fn input_listen(state: &mut TcpConnectionState, ...) {
    // Update data components first
    state.rod.on_syn_in_listen(seg)?;
    state.flow_ctrl.on_syn_in_listen(seg, &state.conn_mgmt)?;
    state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;

    // State transition happens last
    state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;

    // Now state.conn_mgmt.state == SYN_RCVD
}
```

Dispatcher enforces correct order, but no component has special privileges.

### What about error handling?

**Example:** If ROD validation fails, don't update other components

**Solution:** Use `?` operator - fails fast

```rust
fn input_listen(state: &mut TcpConnectionState, ...) {
    // If ROD fails, function returns early
    state.rod.on_syn_in_listen(seg)?;  // ← If this fails...

    // These never execute
    state.flow_ctrl.on_syn_in_listen(seg, &state.conn_mgmt)?;
    state.cong_ctrl.on_syn_in_listen(&state.conn_mgmt)?;
    state.conn_mgmt.on_syn_in_listen(remote_ip, remote_port)?;
}
```

Rust's error handling ensures atomicity.

---

## Comparison: Old vs New

| Aspect | Current (Control Path Privileged) | Proposed (Component Methods) |
|--------|-----------------------------------|------------------------------|
| **Modularity** | Partial - control path exception | Complete - no exceptions |
| **Write Permissions** | Control path: all<br>Data path: own only | All code: own component only |
| **State Transitions** | Mixed with other logic | Pure - only in ConnMgmt methods |
| **Testability** | Must test entire control path | Test each component independently |
| **Mental Model** | "Control path is special" | "All components are equal" |
| **Code Location** | `control_path.rs` - monolithic | `components/*.rs` - separated |
| **Future Data Path** | Different rules than control | Same rules everywhere |
| **Component Swap** | Must update control path | Just implement interface |

---

## Recommendation

**Adopt the proposed component-method approach** because:

1. ✅ **Eliminates conceptual inconsistency** - No "privileged" code
2. ✅ **Simpler architecture** - Same rules everywhere
3. ✅ **Better aligns with modular goals** - Pure separation
4. ✅ **More testable** - Isolate component logic
5. ✅ **Future-proof** - Easy to extend/swap components
6. ✅ **Professor's feedback** - Matches the suggested approach

The migration is straightforward:
- Keep existing tests (update them incrementally)
- Move logic gradually (one state transition at a time)
- No change to external API/FFI layer

**Timeline:** 1-2 weeks for full migration of all state transitions.

---

## Next Steps

1. ✅ **Review and approve this proposal**
2. Create component method signatures (stubs)
3. Migrate one state transition (e.g., LISTEN → SYN_RCVD) as proof-of-concept
4. Update tests for that transition
5. Migrate remaining transitions incrementally
6. Reorganize files (create `components/` directory)
7. Delete `control_path.rs`
8. Update documentation

---

## Questions for Discussion

1. **Method naming convention?**
   - Option A: `on_syn_in_listen()` (current proposal)
   - Option B: `handle_syn_in_listen()`
   - Option C: `process_syn_listen_state()`

2. **No-op methods?**
   - Should components provide no-op methods for states where they don't change?
   - Or should dispatcher skip calling them?

3. **Validation location?**
   - Keep validation in ROD component? (current proposal)
   - Move to separate `validation.rs` module?

4. **Error aggregation?**
   - If multiple components can fail, how to report which one failed?
   - Currently: First failure short-circuits (stops calling remaining components)

5. **Testing strategy?**
   - Test components in isolation? (unit tests)
   - Test full state transitions? (integration tests)
   - Both?

---

**This proposal provides a clear path forward to eliminate the "control path exception" and achieve complete modular separation, exactly as your professor suggested.**
