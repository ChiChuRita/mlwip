# lwip TCP Stack Architecture and Control Path Documentation

The control path implements a **state-based dispatcher pattern**:

```
Packet arrives → tcp_input() → Checks current state → Calls state-specific function
```

**Key Principle:** One function per state/scenario. Each function handles state transitions for that specific case.

---

## 🏗️ **Architecture Overview**

```
┌─────────────────────────────────────────────┐
│        APPLICATION LAYER                    │
│  Calls: tcp_bind(), tcp_connect(), etc.     │
└──────────────┬──────────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────────┐
│         CONTROL PATH (control_path.rs)      │
│  • State transitions (process_* functions)  │
│  • API functions (tcp_bind, tcp_listen)     │
│  • Validation (RFC 5961 security)           │
└──────────────┬──────────────────────────────┘
               ↑
               │ Incoming packets
┌──────────────┴──────────────────────────────┐
│          IP LAYER / tcp_input()             │
│  Routes packets based on connection state   │
└─────────────────────────────────────────────┘
```

---

## 🎯 **Function Categories**

### **1. Application API Functions** (Called by user code)

| Function | Purpose | State Transition |
| --- | --- | --- |
| `tcp_bind()` | Assign local IP/port | `CLOSED → CLOSED` |
| `tcp_listen()` | Start accepting connections | `CLOSED → LISTEN` |
| `tcp_connect()` | Initiate connection to remote | `CLOSED → SYN_SENT` |
| `tcp_abort()` | Force close with RST | `ANY → CLOSED` |
| `initiate_close()` | Graceful close with FIN | `ESTABLISHED → FIN_WAIT_1CLOSE_WAIT → LAST_ACK` |

---

### **2. State Transition Functions** (Called internally by dispatcher)

### **Connection Setup (3-Way Handshake)**

| Function | Current State | Trigger | New State | State Modified |
| --- | --- | --- | --- | --- |
| `process_syn_in_listen()` | `LISTEN` | Receive SYN | `SYN_RCVD` | • `conn_mgmt.state`
• `rod.irs` (peer’s ISN)
• `rod.rcv_nxt`
• `rod.iss` (our ISN)
•`flow_ctrl.rcv_wnd`•`flow_ctrl.snd_wnd`•`conn_mgmt.remote_ip/port` |
| `process_synack_in_synsent()` | `SYN_SENT` | Receive SYN+ACK | `ESTABLISHED` | • `conn_mgmt.state`
• `rod.irs`
• `rod.rcv_nxt`
• `rod.snd_nxt`
• `rod.lastack`
• `flow_ctrl.snd_wnd` |
| `process_ack_in_synrcvd()` | `SYN_RCVD` | Receive ACK | `ESTABLISHED` | • `conn_mgmt.state`
• `rod.snd_nxt`
• `rod.lastack` |

---

### **Connection Teardown (4-Way Handshake)**

| Function | Current State | Trigger | New State | State Modified |
| --- | --- | --- | --- | --- |
| `process_fin_in_established()` | `ESTABLISHED` | Receive FIN | `CLOSE_WAIT` | • `conn_mgmt.state`• `rod.rcv_nxt` (+1 for FIN) |
| `process_ack_in_finwait1()` | `FIN_WAIT_1` | Receive ACK of our FIN | `FIN_WAIT_2` | • `conn_mgmt.state`• `rod.lastack` |
| `process_fin_in_finwait2()` | `FIN_WAIT_2` | Receive FIN | `TIME_WAIT` | • `conn_mgmt.state`• `rod.rcv_nxt` (+1 for FIN) |
| `process_ack_in_lastack()` | `LAST_ACK` | Receive ACK of our FIN | `CLOSED` | • `conn_mgmt.state` |

---

### **Simultaneous Close**

| Function | Current State | Trigger | New State | State Modified |
| --- | --- | --- | --- | --- |
| `process_fin_in_finwait1()` | `FIN_WAIT_1` | Receive FIN (crossing) | `CLOSING` | • `conn_mgmt.state`• `rod.rcv_nxt` (+1 for FIN) |
| `process_ack_in_closing()` | `CLOSING` | Receive ACK of our FIN | `TIME_WAIT` | • `conn_mgmt.state`• `rod.lastack` |

---

### **Connection Reset**

| Function | Current State | Trigger | New State | State Modified |
| --- | --- | --- | --- | --- |
| `process_rst()` | `ANY` | Receive valid RST | `CLOSED` | • `conn_mgmt.state` |

---

### **3. Input Dispatcher** (Routes incoming packets)

| Function | Purpose |
| --- | --- |
| `tcp_input()` | Main dispatcher - routes based on state |
| `input_listen()` | Handle segments in LISTEN |
| `input_synsent()` | Handle segments in SYN_SENT |
| `input_synrcvd()` | Handle segments in SYN_RCVD |
| `input_established()` | Handle segments in ESTABLISHED |
| `input_finwait1()` | Handle segments in FIN_WAIT_1 |
| `input_finwait2()` | Handle segments in FIN_WAIT_2 |
| `input_closewait()` | Handle segments in CLOSE_WAIT |
| `input_closing()` | Handle segments in CLOSING |
| `input_lastack()` | Handle segments in LAST_ACK |
| `input_timewait()` | Handle segments in TIME_WAIT |
| `input_closed()` | Handle segments in CLOSED (send RST) |
| `handle_rst()` | Process RST with validation |

**Dispatcher Pattern:**

```rust
pub fn tcp_input(
    state: &mut TcpConnectionState,
    seg: &TcpSegment,
    remote_ip: ffi::ip_addr_t,
    remote_port: u16,
) -> Result<InputAction, &'static str> {
    // Handle RST first (any state)
    if seg.flags.rst {
        return Self::handle_rst(state, seg);
    }

    // Route based on current state
    match state.conn_mgmt.state {
        TcpState::Closed => Self::input_closed(state, seg),
        TcpState::Listen => Self::input_listen(state, seg, remote_ip, remote_port),
        TcpState::SynSent => Self::input_synsent(state, seg),
        TcpState::Established => Self::input_established(state, seg),
        // ... all 11 states
    }
}
```

**Returns `InputAction` to tell caller what to do:**

```rust
pub enum InputAction {
    Accept,           // Process segment normally
    Drop,             // Discard segment
    SendAck,          // Send ACK response
    SendSynAck,       // Send SYN+ACK (handshake)
    SendChallengeAck, // RFC 5961 security
    SendRst,          // Reset connection
    Abort,            // Abort connection (for RST processing)
}
```

---

## 📊 **State Modification Matrix**

### **Legend**

- ✅ = Modified
- ❌ = Not modified
- 🔄 = Sometimes modified

| State Component | Setup | Teardown | RST | Bind | Listen | Connect | Abort |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `conn_mgmt.state` | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ |
| `conn_mgmt.local_ip` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `conn_mgmt.local_port` | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ |
| `conn_mgmt.remote_ip` | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `conn_mgmt.remote_port` | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `rod.iss` | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `rod.irs` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `rod.snd_nxt` | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `rod.rcv_nxt` | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `rod.lastack` | 🔄 | 🔄 | ❌ | ❌ | ❌ | ✅ | ❌ |
| `flow_ctrl.rcv_wnd` | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |
| `flow_ctrl.snd_wnd` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `cong_ctrl.cwnd` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |

### **Detailed Breakdown by Function**

### **`process_syn_in_listen()` - LISTEN → SYN_RCVD**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::SynRcvd;
state.conn_mgmt.remote_ip = remote_ip;
state.conn_mgmt.remote_port = remote_port;

state.rod.irs = seg.seqno;                      // Store peer's ISN
state.rod.rcv_nxt = seg.seqno.wrapping_add(1);  // Next expected SEQ
state.rod.iss = generate_iss();                 // Generate our ISN
state.rod.snd_nxt = state.rod.iss;              // Initialize send SEQ

state.flow_ctrl.rcv_wnd = 4096;                 // Advertise receive window
state.flow_ctrl.snd_wnd = seg.wnd;              // Store peer's window
```

### **`process_synack_in_synsent()` - SYN_SENT → ESTABLISHED**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::Established;

state.rod.irs = seg.seqno;
state.rod.rcv_nxt = seg.seqno.wrapping_add(1);
state.rod.snd_nxt = state.rod.snd_nxt.wrapping_add(1);  // Account for SYN
state.rod.lastack = state.rod.snd_nxt;

state.flow_ctrl.snd_wnd = seg.wnd;
```

### **`process_ack_in_synrcvd()` - SYN_RCVD → ESTABLISHED**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::Established;
state.rod.snd_nxt = state.rod.iss.wrapping_add(1);
state.rod.lastack = seg.ackno;
```

### **`process_fin_in_established()` - ESTABLISHED → CLOSE_WAIT**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::CloseWait;state.rod.rcv_nxt = state.rod.rcv_nxt.wrapping_add(1);  // FIN consumes 1 SEQ
```

### **`initiate_close()` - Active Close**

```rust
// From ESTABLISHED → FIN_WAIT_1:
state.conn_mgmt.state = TcpState::FinWait1;
// From CLOSE_WAIT → LAST_ACK:
state.conn_mgmt.state = TcpState::LastAck;
// Note: Does NOT modify snd_nxt (FIN transmission will do that)
```

### **`tcp_bind()` - Assign Local Address**

```rust
// State modifications:
state.conn_mgmt.local_ip = local_ip;state.conn_mgmt.local_port = local_port;
```

### **`tcp_listen()` - Start Listening**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::Listen;
```

### **`tcp_connect()` - Initiate Connection**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::SynSent;
state.conn_mgmt.remote_ip = remote_ip;state.conn_mgmt.remote_port = remote_port;
state.rod.iss = generate_iss();
state.rod.snd_nxt = state.rod.iss;
state.rod.lastack = state.rod.iss.wrapping_sub(1);
state.flow_ctrl.rcv_wnd = 4096;state.cong_ctrl.cwnd = /* RFC 5681 initial window */;
```

### **`tcp_abort()` - Force Close**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::Closed;
```

### **`process_rst()` - Connection Reset**

```rust
// State modifications:
state.conn_mgmt.state = TcpState::Closed;
```

---
