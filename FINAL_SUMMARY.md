# TCP Rust Modularization: Complete Technical Reference

> "We decomposed TCP into five components with non-overlapping write scopes, enforced by Rust's type system at compile time."

---

## What's the Problem?

### Traditional TCP = Spaghetti State

```
┌─────────────────────────────────────┐
│         Monolithic tcp_pcb          │
│  ┌───┬───┬───┬───┬───┬───┬───┐    │
│  │snd│rcv│cwnd│wnd│state│...│60+ │    │
│  └───┴───┴───┴───┴───┴───┴───┘    │
│         ↑   ↑   ↑   ↑   ↑          │
│    ANY FUNCTION CAN WRITE ANYWHERE  │
└─────────────────────────────────────┘
```

**Why is this bad?**

| Problem | Real-World Impact |
|---------|-------------------|
| **Bugs spread** | A congestion control bug can corrupt connection state |
| **Hard to test** | Need full TCP stack to test one component |
| **Hard to verify** | Can't prove correctness of isolated logic |
| **Slow innovation** | Changing ACK logic requires touching 10 files |
| **Hard to offload** | No clear boundaries for hardware acceleration |

---

## What's Our Solution?

### Five Disjoint Components

```
┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
│ ConnMgmt│ │   ROD   │ │FlowCtrl │ │CongCtrl │ │  Demux  │
├─────────┤ ├─────────┤ ├─────────┤ ├─────────┤ ├─────────┤
│ state   │ │ snd_nxt │ │ snd_wnd │ │ cwnd    │ │(no state│
│ 4-tuple │ │ rcv_nxt │ │ rcv_wnd │ │ ssthresh│ │  uses   │
│ timers  │ │ lastack │ │ scaling │ │         │ │ 4-tuple)│
│ options │ │ iss/irs │ │ persist │ │         │ │         │
└────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └─────────┘
     │           │           │           │
     ▼           ▼           ▼           ▼
  ONLY writes  ONLY writes  ONLY writes  ONLY writes
  to ConnMgmt  to ROD       to FlowCtrl  to CongCtrl
```

**The Rule:** Each component method can only write to its own state.

---

## Why Rust?

### Compile-Time Enforcement

```rust
impl ConnectionManagementState {
    pub fn on_syn(&mut self) {
        self.state = SynRcvd;     // ✅ Allowed (my field)
        self.snd_nxt = 100;       // ❌ COMPILE ERROR (ROD's field)
    }
}
```

**Key insight:** In C, you'd need discipline. In Rust, the compiler enforces it.

### Quick Comparison

| C (lwIP) | Rust (Ours) |
|----------|-------------|
| `pcb->state = SYN_RCVD; pcb->snd_nxt = x;` | Each component has its own `&mut self` |
| Runtime bugs if wrong field touched | Compile-time error if wrong field touched |
| Trust the programmer | Trust the type system |

---

## Architecture

### Original lwIP Structure (C)
```
src/core/
├── tcp.c       (~2,700 lines) - Connection management, timers, API
├── tcp_in.c    (~2,200 lines) - Input processing, state machine
├── tcp_out.c   (~1,800 lines) - Output, segment transmission
└── include/lwip/priv/tcp_priv.h - tcp_pcb structure (~300 bytes)
```

### New Rust Structure
```
src/core/tcp_rust/src/
├── lib.rs              (791 lines)  - FFI bridge, C-compatible exports
├── state.rs            (89 lines)   - TcpConnectionState composition
├── tcp_api.rs          (222 lines)  - High-level API orchestration
├── tcp_types.rs        (65 lines)   - Shared types (TcpFlags, TcpSegment, etc.)
├── tcp_proto.rs        (177 lines)  - Protocol constants, TcpHdr struct
└── components/
    ├── mod.rs                       - Component exports
    ├── connection_mgmt.rs (283 lines) - TCP state machine
    ├── rod.rs             (295 lines) - Reliable Ordered Delivery
    ├── flow_control.rs    (172 lines) - Window management
    └── congestion_control.rs (170 lines) - Congestion window
```

**Total: ~2,300 lines of Rust source + ~2,000 lines of tests**

---

## The 5 Components

### Design Principle
> Each component **owns its own state** and **only modifies its own state**.
> The API layer orchestrates calls across components.

### 1. ConnectionManagementState (`connection_mgmt.rs`)

**Purpose:** TCP state machine, connection lifecycle, connection tuple

**Owned State:**
```rust
pub struct ConnectionManagementState {
    // Connection Tuple
    pub local_ip: ip_addr_t,
    pub remote_ip: ip_addr_t,
    pub local_port: u16,
    pub remote_port: u16,

    // State Machine
    pub state: TcpState,  // 11 states: Closed → Listen → SynSent → ...

    // Keep-Alive
    pub keep_idle: u32,   // Default: 7200000ms (2 hours)
    pub keep_intvl: u32,  // Default: 75000ms
    pub keep_cnt: u32,    // Default: 9

    // Options
    pub mss: u16,         // Default: 536
    pub ttl: u8,          // Default: 255
    pub prio: u8,         // Default: 64
    pub flags: u16,
}
```

**Key Methods:**
| Method | Transition | Description |
|--------|------------|-------------|
| `on_bind()` | CLOSED→CLOSED | Store local IP/port |
| `on_listen()` | CLOSED→LISTEN | Start listening |
| `on_connect()` | CLOSED→SYN_SENT | Initiate active open |
| `on_syn_in_listen()` | LISTEN→SYN_RCVD | Received SYN |
| `on_synack_in_synsent()` | SYN_SENT→ESTABLISHED | Received SYN+ACK |
| `on_ack_in_synrcvd()` | SYN_RCVD→ESTABLISHED | Handshake complete |
| `on_close()` | Various→FIN states | Initiate close |
| `on_fin_in_established()` | ESTABLISHED→CLOSE_WAIT | Peer closed |
| `on_rst()` | ANY→CLOSED | Connection reset |
| `on_abort()` | ANY→CLOSED | Abort connection |

---

### 2. ReliableOrderedDeliveryState (`rod.rs`)

**Purpose:** Sequence numbers, acknowledgments, retransmission tracking

**Owned State:**
```rust
pub struct ReliableOrderedDeliveryState {
    // Sequence Numbers
    pub snd_nxt: u32,     // Next sequence number to send
    pub rcv_nxt: u32,     // Next sequence number expected
    pub lastack: u32,     // Last cumulative ACK received
    pub iss: u32,         // Initial Send Sequence
    pub irs: u32,         // Initial Receive Sequence

    // Send Buffer Tracking
    pub snd_lbb: u32,     // Next byte to buffer
    pub snd_buf: u16,     // Available send buffer space
    pub snd_queuelen: u16,// Queued pbuf count

    // RTT & Retransmission
    pub rto: i16,         // Retransmit timeout (default: 3000ms)
    pub nrtx: u8,         // Retransmit count
    pub dupacks: u8,      // Duplicate ACK count
}
```

**Key Methods:**
| Method | Description |
|--------|-------------|
| `on_connect()` | Generate ISS, initialize send state |
| `on_syn_in_listen()` | Store IRS from SYN, generate ISS |
| `on_synack_in_synsent()` | Validate ACK of our SYN, store IRS |
| `validate_sequence_number()` | RFC 793 sequence validation |
| `validate_ack()` | Returns: Duplicate, Valid, TooOld, TooNew |
| `validate_rst()` | RFC 5961 RST validation |

---

### 3. FlowControlState (`flow_control.rs`)

**Purpose:** Send and receive window management

**Owned State:**
```rust
pub struct FlowControlState {
    // Peer's Advertised Window
    pub snd_wnd: u16,         // Current send window
    pub snd_wnd_max: u16,     // Maximum seen
    pub snd_wl1: u32,         // Seq# for window update validation
    pub snd_wl2: u32,         // Ack# for window update validation

    // Our Receive Window
    pub rcv_wnd: u16,         // Available receive buffer
    pub rcv_ann_wnd: u16,     // Window to advertise

    // Window Scaling
    pub snd_scale: u8,
    pub rcv_scale: u8,
}
```

**Key Methods:**
| Method | Description |
|--------|-------------|
| `on_connect()` | Initialize rcv_wnd = 4096 |
| `on_syn_in_listen()` | Store peer's advertised window |
| `on_synack_in_synsent()` | Update window from SYN+ACK |

---

### 4. CongestionControlState (`congestion_control.rs`)

**Purpose:** Congestion window management

**Owned State:**
```rust
pub struct CongestionControlState {
    pub cwnd: u16,      // Congestion window
    pub ssthresh: u16,  // Slow start threshold (default: 0xFFFF)
}
```

**Key Methods:**
| Method | Description |
|--------|-------------|
| `on_connect()` | IW = min(4*MSS, max(2*MSS, 4380)) per RFC 5681 |
| `on_syn_in_listen()` | Same IW calculation |
| `on_synack_in_synsent()` | cwnd = MSS |

---

### 5. DemuxState (`mod.rs`)

**Purpose:** Connection demultiplexing (placeholder)

```rust
pub struct DemuxState {}  // Currently empty - demux uses 4-tuple from conn_mgmt
```

---

## C-to-Rust Bridge

### PCB Pointer Aliasing

The key insight: **C code treats a Rust struct pointer as an opaque `tcp_pcb*`**.

```rust
// Allocation: Rust struct returned as C pointer
pub unsafe extern "C" fn tcp_new_rust() -> *mut ffi::tcp_pcb {
    let state = Box::new(TcpConnectionState::new());
    Box::into_raw(state) as *mut ffi::tcp_pcb  // Cast to C type
}

// Usage: Cast back to Rust type
unsafe fn pcb_to_state_mut(pcb: *mut ffi::tcp_pcb) -> Option<&mut TcpConnectionState> {
    if pcb.is_null() { None }
    else { Some(&mut *(pcb as *mut TcpConnectionState)) }
}
```

### FFI Function Pattern

Every C-facing function follows this pattern:

```rust
#[no_mangle]
pub unsafe extern "C" fn tcp_bind_rust(
    pcb: *mut ffi::tcp_pcb,
    ipaddr: *const ffi::ip_addr_t,
    port: u16,
) -> i8 {
    // 1. Convert C pointer to Rust reference
    let Some(state) = pcb_to_state_mut(pcb) else {
        return ERR_ARG;
    };

    // 2. Call Rust implementation
    match tcp_bind(state, ip, port) {
        Ok(_) => ERR_OK,
        Err(_) => ERR_VAL,
    }
}
```

### Exported FFI Functions (35+)

| Category | Functions |
|----------|-----------|
| **Lifecycle** | `tcp_new_rust`, `tcp_close_rust`, `tcp_abort_rust` |
| **Setup** | `tcp_bind_rust`, `tcp_listen_with_backlog_rust`, `tcp_connect_rust` |
| **Data** | `tcp_write_rust`, `tcp_output_rust`, `tcp_recved_rust` |
| **Callbacks** | `tcp_recv_rust`, `tcp_sent_rust`, `tcp_err_rust`, `tcp_poll_rust`, `tcp_accept_rust` |
| **State Query** | `tcp_get_state_rust`, `tcp_get_sndbuf_rust`, `tcp_get_sndqueuelen_rust` |
| **Keep-Alive** | `tcp_get_keep_idle_rust`, `tcp_set_keep_idle_rust`, etc. |
| **Flags** | `tcp_set_flags_rust`, `tcp_clear_flags_rust`, `tcp_is_flag_set_rust` |

---

## API Orchestration Layer (`tcp_api.rs`)

The API layer coordinates component methods without directly modifying component state:

```rust
pub fn tcp_connect(
    state: &mut TcpConnectionState,
    remote_ip: ip_addr_t,
    remote_port: u16,
) -> Result<(), &'static str> {
    // Validate precondition
    if state.conn_mgmt.state != TcpState::Closed {
        return Err("Can only connect from CLOSED state");
    }

    // Each component initializes its own state
    state.rod.on_connect()?;           // Generate ISS
    state.flow_ctrl.on_connect()?;     // Init rcv_wnd
    state.cong_ctrl.on_connect(&state.conn_mgmt)?;  // Init cwnd
    state.conn_mgmt.on_connect(remote_ip, remote_port)?;  // → SYN_SENT

    Ok(())
}
```

### Input Processing (`tcp_input`)

```rust
pub fn tcp_input(state: &mut TcpConnectionState, seg: &TcpSegment, ...)
    -> Result<InputAction, &'static str>
{
    // 1. RST handling (any state)
    if seg.flags.rst {
        match state.rod.validate_rst(seg, state.flow_ctrl.rcv_wnd) {
            RstValidation::Valid => { state.conn_mgmt.on_rst()?; return Ok(InputAction::Abort); }
            RstValidation::Challenge => return Ok(InputAction::SendChallengeAck),
            RstValidation::Invalid => return Ok(InputAction::Drop),
        }
    }

    // 2. State-specific dispatch
    match state.conn_mgmt.state {
        TcpState::Listen => { /* SYN handling */ }
        TcpState::SynSent => { /* SYN+ACK handling */ }
        TcpState::Established => { /* Data + FIN handling */ }
        // ... all 11 states covered
    }
}
```

---

## Comparison with Original lwIP

### ✅ Identical Behavior

| Feature | lwIP | Rust | Notes |
|---------|------|------|-------|
| State machine | 11 states in `tcp_pcb->state` | `conn_mgmt.state` | Same transitions |
| Sequence fields | `snd_nxt, rcv_nxt, iss, irs` | Same in `rod` | Same semantics |
| Window fields | `snd_wnd, rcv_wnd` | Same in `flow_ctrl` | Same semantics |
| Congestion fields | `cwnd, ssthresh` | Same in `cong_ctrl` | Same semantics |
| IW calculation | `LWIP_TCP_CALC_INITIAL_CWND` | RFC 5681 formula | Identical |
| RST validation | RFC 5961 | `validate_rst()` | Identical |
| Seq# validation | RFC 793 | `validate_sequence_number()` | Identical |

### ⚠️ Simplified (Known Deviations)

| Feature | lwIP | Rust | Reason |
|---------|------|------|--------|
| ISS generation | Time-based (RFC 6528) | Simple counter | TODO noted |
| Port 0 | Allocates ephemeral | Returns error | Not implemented |
| rcv_wnd default | Based on config | Hardcoded 4096 | Simplified |
| PCB allocation | Pool (memp) | Box heap | Rust idiom |

### ❌ Not Yet Implemented

| Feature | Description |
|---------|-------------|
| Data path | `tcp_write`, `tcp_output` - stubs only |
| Segment TX/RX | No actual packet construction/parsing |
| Retransmission | Timer and logic not implemented |
| Timers | keepalive, 2MSL, retransmit all stubs |
| PCB lists | `tcp_active_pcbs` etc. not managed |
| Out-of-order | `ooseq` queue not implemented |
| Options | Window scaling, SACK not implemented |

---

## Test Coverage

### Test Files
| File | Tests | Coverage |
|------|-------|----------|
| `handshake_tests.rs` | 5 | 3-way handshake, RST, initialization |
| `control_path_tests.rs` | ~40 | State machine, API, validation |
| `lib.rs` (ffi_tests) | 10 | FFI functions, null handling |
| `tcp_proto.rs` | 3 | Header parsing |

**Total: 58 tests, all passing**

### Tested Scenarios
- Active open (client): CLOSED → SYN_SENT → ESTABLISHED
- Passive open (server): CLOSED → LISTEN → SYN_RCVD → ESTABLISHED
- Active close: ESTABLISHED → FIN_WAIT_1 → FIN_WAIT_2 → TIME_WAIT
- Passive close: ESTABLISHED → CLOSE_WAIT → LAST_ACK → CLOSED
- Simultaneous close: ESTABLISHED → FIN_WAIT_1 → CLOSING → TIME_WAIT
- RST generation and validation
- Sequence number validation (RFC 793)
- ACK validation (duplicate, valid, old, future)

---

## Key Design Decisions

### 1. Component Boundaries
**Challenge:** Where to draw lines between components?

**Resolution:**
- `conn_mgmt`: State machine + connection tuple + options
- `rod`: Anything involving sequence numbers
- `flow_ctrl`: Window management
- `cong_ctrl`: Congestion window only (minimal)

### 2. Cross-Component Data Access
**Challenge:** `cong_ctrl.on_connect()` needs MSS from `conn_mgmt`

**Resolution:** Pass immutable reference:
```rust
state.cong_ctrl.on_connect(&state.conn_mgmt)?;
```

### 3. Method Naming Convention
**Pattern:** `on_<event>_in_<state>`
```rust
on_syn_in_listen()       // Received SYN while in LISTEN
on_synack_in_synsent()   // Received SYN+ACK while in SYN_SENT
on_fin_in_established()  // Received FIN while in ESTABLISHED
```

### 4. Return Types for Validation
**Rich enums instead of bool:**
```rust
pub enum RstValidation { Valid, Challenge, Invalid }
pub enum AckValidation { Duplicate, Valid, TooOld, TooNew }
pub enum InputAction { SendSynAck, SendAck, SendRst, Accept, Abort, Drop, ... }
```

---

## Future Work

1. **Data Path Implementation**
   - Implement `tcp_write` buffering
   - Implement `tcp_output` segment construction
   - Connect `tcp_input_rust` to actual parsing

2. **Timers**
   - Retransmission timer
   - Keepalive timer
   - 2MSL timeout for TIME_WAIT

3. **Congestion Control**
   - Slow start algorithm
   - Congestion avoidance
   - Fast retransmit/recovery

4. **Options**
   - Window scaling negotiation
   - SACK support
   - Timestamps

5. **PCB Management**
   - Maintain `tcp_active_pcbs` list
   - Implement proper demux in `tcp_input_rust`

---

## Summary

| Aspect | Status |
|--------|--------|
| **Architecture** | ✅ Complete - 4 components + API layer |
| **State Machine** | ✅ Complete - all 11 states, all transitions |
| **FFI Bridge** | ✅ Complete - 35+ functions exported |
| **Control Path** | ✅ Complete - handshake, close, RST |
| **Validation** | ✅ Complete - RFC 793/5961 compliant |
| **Tests** | ✅ Complete - 58 tests passing |
| **Data Path** | ❌ Stubs only |
| **Timers** | ❌ Stubs only |
| **Segment TX/RX** | ❌ Not implemented |
