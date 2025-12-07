# Demikernel TCP Stack Architecture and Control Path Documentation

The Demikernel TCP stack implements a **modular, async/await-based architecture** with separate socket types for different connection phases.

**Key Principle:** Different socket types for different connection phases (connecting, listening, established), with a shared control block for established connection state.

---

## 🏗️ **Architecture Overview**

```
┌─────────────────────────────────────────────────────────────────┐
│                    APPLICATION LAYER                            │
│  Calls: socket(), bind(), listen(), connect(), push(), pop()    │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ↓
┌─────────────────────────────────────────────────────────────────┐
│                    TCP PEER (peer.rs)                           │
│  • Manages socket address mappings                              │
│  • Routes packets to appropriate sockets                        │
│  • Coordinates ISN generation                                   │
└──────────────────────────────┬──────────────────────────────────┘
                               │
               ┌───────────────┼───────────────┐
               ↓               ↓               ↓
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────────┐
│   TCP SOCKET     │ │   TCP SOCKET     │ │    TCP SOCKET        │
│  (socket.rs)     │ │  (socket.rs)     │ │   (socket.rs)        │
│                  │ │                  │ │                      │
│  SocketState:    │ │  SocketState:    │ │  SocketState:        │
│  Unbound/Bound   │ │  Listening       │ │  Connecting/         │
│                  │ │                  │ │  Established/Closing │
└──────────────────┘ └────────┬─────────┘ └──────────┬───────────┘
                              │                      │
                              ↓                      ↓
                   ┌──────────────────┐   ┌─────────────────────┐
                   │ PASSIVE SOCKET   │   │  ACTIVE OPEN SOCKET │
                   │ (passive_open.rs)│   │  (active_open.rs)   │
                   │                  │   │                     │
                   │ • Handles SYN    │   │ • Sends SYN         │
                   │ • Sends SYN+ACK  │   │ • Waits for SYN+ACK │
                   │ • Waits for ACK  │   │ • Sends final ACK   │
                   └────────┬─────────┘   └──────────┬──────────┘
                            │                        │
                            └──────────┬─────────────┘
                                       ↓
                   ┌───────────────────────────────────────────┐
                   │        ESTABLISHED SOCKET (mod.rs)        │
                   │  • Control Block with modular state       │
                   │  • Background sender/retransmitter/acker  │
                   └───────────────────────────────────────────┘
                                       │
              ┌────────────────────────┼────────────────────────┐
              ↓                        ↓                        ↓
┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐
│ CONNECTION MGMT     │  │ ORDERED DELIVERY    │  │ FLOW CONTROL        │
│ (connection_        │  │ (ordered_delivery_  │  │ (flow_control_      │
│  management_        │  │  state.rs)          │  │  state.rs)          │
│  state.rs)          │  │                     │  │                     │
│                     │  │ • Send/recv queues  │  │ • Send window       │
│ • TCP state machine │  │ • Sequence numbers  │  │ • Window scaling    │
│ • Local/remote addr │  │ • Retransmissions   │  │ • MSS               │
│ • Config options    │  │ • Out-of-order data │  │                     │
└─────────────────────┘  └─────────────────────┘  └─────────────────────┘
              │
              ↓
┌─────────────────────┐
│ CONGESTION CONTROL  │
│ (congestion_        │
│  control_state.rs)  │
│                     │
│ • RTO calculator    │
│ • CC algorithm      │
│ • CWND management   │
└─────────────────────┘
```

---

## 📦 **Module Structure**

```
src/inetstack/protocols/layer4/tcp/
├── mod.rs                    # Module exports
├── peer.rs                   # TcpPeer - socket management and packet routing
├── socket.rs                 # TcpSocket - unified socket interface with SocketState
├── active_open.rs            # Active connection (client-side handshake)
├── passive_open.rs           # Passive connection (server-side handshake)
├── header.rs                 # TCP header parsing/serialization
├── sequence_number.rs        # SeqNumber type with wrapping arithmetic
├── isn_generator.rs          # Initial Sequence Number generation
└── established/
    ├── mod.rs                # EstablishedSocket-established connection handling
    ├── ctrlblk.rs            # ControlBlock - central state container+State enum
    ├── connection_management_state.rs  # Connection state machine
    ├── ordered_delivery_state.rs       # Reliable delivery (ROD)
    ├── flow_control_state.rs           # Flow control state
    ├── congestion_control_state.rs     # Congestion control state
    ├── rto.rs                          # RTO calculator
    └── congestion_control/
        ├── mod.rs            # CC trait and exports
        ├── none.rs           # No congestion control
        ├── cubic.rs          # CUBIC algorithm
        └── options.rs        # CC configuration
```

---

## 🔄 **Socket States**

### **SocketState Enum (socket.rs)**

The `TcpSocket` uses a `SocketState` enum to track the current phase:

| State | Description | Inner Type |
| --- | --- | --- |
| `Unbound` | Fresh socket, not yet bound | None |
| `Bound(SocketAddrV4)` | Bound to local address | Local address |
| `Listening(SharedPassiveSocket)` | Accepting connections | PassiveSocket |
| `Connecting(SharedActiveOpenSocket)` | Connection in progress | ActiveOpenSocket |
| `Established(SharedEstablishedSocket)` | Connection active | EstablishedSocket |
| `Closing(SharedEstablishedSocket)` | Connection closing | EstablishedSocket |

### **Established State Enum (ctrlblk.rs)**

Once established, the `ControlBlock` tracks the TCP state machine:

| State | Description |
| --- | --- |
| `Established` | Connection is active, data can flow |
| `FinWait1` | Local close initiated, FIN sent, waiting for ACK |
| `FinWait2` | FIN ACK’d, waiting for remote FIN |
| `Closing` | Simultaneous close, FIN sent and received |
| `TimeWait` | Waiting for 2*MSL before final close |
| `CloseWait` | Remote closed, waiting for local close |
| `LastAck` | Local close after remote close, waiting for final ACK |
| `Closed` | Connection fully closed |

---

## 🎯 **Function Categories**

### **1. Application API Functions** (TcpPeer → TcpSocket)

| Function | Purpose | State Transition |
| --- | --- | --- |
| `socket()` | Create new TCP socket | `→ Unbound` |
| `bind()` | Assign local address | `Unbound → Bound` |
| `listen()` | Start accepting connections | `Bound → Listening` |
| `connect()` | Initiate connection | `Bound → Connecting → Established` |
| `accept()` | Accept incoming connection | Returns new `Established` socket |
| `push()` | Send data | `Established` (no change) |
| `pop()` | Receive data | `Established` (no change) |
| `close()` | Graceful close | `Established → FinWait1 → ... → Closed` |
| `hard_close()` | Force close | `Any → Closed` |

---

### **2. Connection Setup - Active Open (Client)**

**File:** `active_open.rs`

| Function | Current State | Trigger | New State | Actions |
| --- | --- | --- | --- | --- |
| `SharedActiveOpenSocket::new()` | - | `connect()` called | `Connecting` | Initialize socket with local ISN |
| `connect()` | `Connecting` | Coroutine starts | - | Send SYN, wait for SYN+ACK |
| `process_ack()` | `Connecting` | Receive SYN+ACK | `Established` | Validate ACK, send final ACK, create EstablishedSocket |
| `receive()` | `Connecting` | Packet arrives | - | Queue packet for processing |

- **State Modifications in `process_ack()`:**
    
    ```rust
    // Validate sequence numbers
    let expected_seq = local_isn + SeqNumber::from(1);
    if !(header.ack && header.ack_num == expected_seq) { /* error */ }
    
    // Extract options
    for option in header.iter_options() {
        match option {
            TcpOptions2::WindowScale(w) => remote_window_scale_bits = Some(*w),
            TcpOptions2::MaximumSegmentSize(m) => mss = *m as usize,
            _ => continue,
        }
    }
    
    // Create EstablishedSocket with:
    // - receiver_seq_no = header.seq_num + 1 (peer's ISN + 1)
    // - sender_seq_no = local_isn + 1 (our ISN + 1)
    // - Window sizes and scaling
    // - MSS
    ```
    

---

### **3. Connection Setup - Passive Open (Server)**

**File:** `passive_open.rs`

| Function | Current State | Trigger | New State | Actions |
| --- | --- | --- | --- | --- |
| `SharedPassiveSocket::new()` | - | `listen()` called | `Listening` | Initialize with backlog |
| `receive()` | `Listening` | SYN arrives | - | Call `handle_new_syn()` |
| `handle_new_syn()` | `Listening` | Valid SYN | - | Spawn handshake coroutine |
| `send_syn_ack_and_wait_for_ack()` | - | Coroutine | - | Send SYN+ACK, wait for ACK |
| `wait_for_ack()` | - | ACK arrives | `Established` | Validate ACK, create EstablishedSocket |
| `do_accept()` | `Listening` | Accept called | - | Return ready socket from queue |
- **State Modifications in `send_syn_ack()`:**
    
    ```rust
    let mut tcp_hdr = TcpHeader::new(local.port(), remote.port());
    tcp_hdr.syn = true;
    tcp_hdr.seq_num = local_isn;          // Our ISN
    tcp_hdr.ack = true;
    tcp_hdr.ack_num = remote_isn + 1;     // Acknowledge peer's SYN
    tcp_hdr.window_size = tcp_config.get_receive_window_size();
    tcp_hdr.push_option(TcpOptions2::MaximumSegmentSize(mss));
    tcp_hdr.push_option(TcpOptions2::WindowScale(window_scale));
    ```
    
- **State Modifications in `wait_for_ack()` → Create EstablishedSocket:**
    
    ```rust
    SharedEstablishedSocket::new(
        local, remote,
        runtime, layer3_endpoint,
        data_from_ack,             // Optional data piggybacked on ACK
        tcp_config, socket_options,
        remote_isn + 1,            // receiver_seq_no (RCV.NXT)
        ack_delay_timeout,
        local_window_size_bytes,
        local_window_scale_bits,
        local_isn + 1,             // sender_seq_no (SND.NXT)
        remote_window_size_bytes,
        remote_window_scale_bits,
        mss,
        congestion_control::None::new,
        None,
    )
    ```
    

---

### **4. Connection Teardown**

**File:** `established/mod.rs`

| Function | Current State | Trigger | New State | Actions |
| --- | --- | --- | --- | --- |
| `close()` | `Established` | App calls close | `FinWait1` | Call `local_close()` |
| `close()` | `CloseWait` | App calls close | `LastAck` | Call `remote_already_closed()` |
| `local_close()` | `FinWait1` | - | `TimeWait` | Send FIN, wait for FIN+ACK |
| `remote_already_closed()` | `LastAck` | - | `Closed` | Send FIN, wait for ACK |
- **State Transitions in `local_close()`:**
    
    ```rust
    // 1. Set state to FIN_WAIT_1
    self.control_block.connection_management.state = State::FinWait1;
    
    // 2. Send FIN and wait for both:
    //    - Remote's FIN
    //    - ACK for our FIN
    let (result1, result2) = join!(wait_for_fin, push_fin_and_wait_for_ack);
    
    // 3. After both complete, enter TIME_WAIT
    debug_assert_eq!(state, State::TimeWait);
    yield_with_timeout(MSL * 2).await;
    self.control_block.connection_management.state = State::Closed;
    ```
    
- **FIN Processing in `check_and_process_fin()`:**
    
    ```rust
    // When FIN received and all prior data received:
    let state = match cb.connection_management.state {
        State::Established => State::CloseWait,   // Remote initiated close
        State::FinWait1 => State::Closing,        // Simultaneous close
        State::FinWait2 => State::TimeWait,       // Normal close completion
        state => unreachable!("Cannot be in {:?}", state),
    };
    cb.connection_management.state = state;
    // Push empty buffer to signal EOF to application
    cb.delivery.pop_queue.push(DemiBuffer::new(0));
    // Move RCV.NXT over the FIN
    cb.delivery.receive_next_seq_no = cb.delivery.receive_next_seq_no + 1.into();
    ```
    

---

### **5. Packet Input Processing**

**File:** `established/ordered_delivery_state.rs`

| Function | Purpose |
| --- | --- |
| `receive()` | Entry point for incoming packets |
| `process_packet()` | Main packet processing pipeline |
| `check_segment_in_window()` | Validate segment within receive window |
| `check_and_process_rst()` | Handle RST segments |
| `check_syn()` | Reject unexpected SYN |
| `check_and_process_ack()` | Process ACK, update state |
| `process_data()` | Handle data segments |
| `check_and_process_fin()` | Handle FIN segments |

**Packet Processing Pipeline:**

```rust
fn process_packet(control_block, layer3_endpoint, header, data, now) -> Result<(), Fail> {
    // 1. Check segment in receive window, trim if needed
    check_segment_in_window(...)?;
    
    // 2. Handle RST (connection reset)
    check_and_process_rst(control_block, &header)?;
    
    // 3. Reject unexpected SYN
    check_syn(&header)?;
    
    // 4. Process ACK - update send window, CC state, delivery state
    control_block.check_and_process_ack(&header, now)?;
    
    // 5. Process data payload
    if !data.is_empty() {
        process_data(...)?;
    }
    
    // 6. Process FIN flag
    check_and_process_fin(control_block, &header, seg_end, layer3_endpoint)?;
    
    // 7. Schedule delayed ACK if needed
    if ack_deadline.is_none() {
        ack_deadline = Some(now + ack_delay_timeout);
    }
    
    Ok(())
}}
```

---

### **6. RST Handling**

**File:** `established/ordered_delivery_state.rs` and `passive_open.rs`

| Function | Context | Actions |
| --- | --- | --- |
| `check_and_process_rst()` | Established connection | Set state to `Closed`, return error |
| `send_rst()` | Passive socket | Send RST to invalid connection attempts |
- **RST Processing:**
    
    ```rust
    fn check_and_process_rst(cb: &mut ControlBlock, header: &TcpHeader) -> Result<(), Fail> {
        if !header.rst {
            return Ok(());
        }
        info!("Received RST: remote reset connection");
        cb.delivery.recv_fin_seq_no.set(Some(header.seq_num));
        cb.connection_management.state = State::Closed;
        Err(Fail::new(libc::ECONNRESET, "remote reset connection"))
    }
    ```
    
- **RST Generation (passive_open.rs):**
    
    ```rust
    fn send_rst(&mut self, remote: &SocketAddrV4, tcp_hdr: TcpHeader) {
        // Generate RST according to RFC 793 Section 3.4
        let (seq_num, ack_num) = if tcp_hdr.ack {
            (tcp_hdr.ack_num, Some(tcp_hdr.ack_num + 1))
        } else {
            (SeqNumber::from(0), Some(tcp_hdr.seq_num + header_size))
        };
        
        let mut tcp_hdr = TcpHeader::new(local.port(), remote.port());
        tcp_hdr.rst = true;
        tcp_hdr.seq_num = seq_num;
        if let Some(ack_num) = ack_num {
            tcp_hdr.ack = true;
            tcp_hdr.ack_num = ack_num;
        }
        // Send packet...
    }
    ```
    

---

## 📊 **State Component Breakdown**

### **ControlBlock (ctrlblk.rs)**

The central state container for established connections:

```rust
pub struct ControlBlock {
    pub connection_management: ConnectionManagementState,
    pub delivery: OrderedDeliveryState,
    pub flow_control: FlowControlState,
    pub congestion_control: CongestionControlState,
}
```

---

### **ConnectionManagementState**

| Field | Type | Purpose |
| --- | --- | --- |
| `local` | `SocketAddrV4` | Local endpoint address |
| `remote` | `SocketAddrV4` | Remote endpoint address |
| `tcp_config` | `TcpConfig` | Configuration options |
| `socket_options` | `TcpSocketOptions` | SO_* options |
| `state` | `State` | Current TCP state machine state |

---

### **OrderedDeliveryState (Reliable Ordered Delivery)**

| Field | Type | RFC 793 Term | Purpose |
| --- | --- | --- | --- |
| `send_unacked` | `SharedAsyncValue<SeqNumber>` | SND.UNA | Oldest unacknowledged byte |
| `send_next_seq_no` | `SharedAsyncValue<SeqNumber>` | SND.NXT | Next sequence number to send |
| `unsent_next_seq_no` | `SeqNumber` | - | Next sequence to allocate |
| `sender_fin_seq_no` | `Option<SeqNumber>` | - | Sequence number of FIN |
| `unacked_queue` | `SharedAsyncQueue<UnackedSegment>` | - | Segments awaiting ACK |
| `unsent_queue` | `SharedAsyncQueue<DemiBuffer>` | - | Data waiting to be sent |
| `retransmit_deadline_time_secs` | `SharedAsyncValue<Option<Instant>>` | - | RTO deadline |
| `reader_next_seq_no` | `SeqNumber` | - | Next byte for application |
| `receive_next_seq_no` | `SeqNumber` | RCV.NXT | Next expected sequence |
| `recv_fin_seq_no` | `SharedAsyncValue<Option<SeqNumber>>` | - | Received FIN sequence |
| `pop_queue` | `AsyncQueue<DemiBuffer>` | - | Data ready for application |
| `ack_delay_timeout_secs` | `Duration` | - | Delayed ACK timeout |
| `ack_deadline_time_secs` | `SharedAsyncValue<Option<Instant>>` | - | Delayed ACK deadline |
| `buffer_size_bytes` | `u32` | - | Receive buffer size |
| `window_scale_shift_bits` | `u8` | - | Window scale factor |
| `out_of_order_frames` | `VecDeque<(SeqNumber, DemiBuffer)>` | - | Out-of-order segments |

---

### **FlowControlState**

| Field | Type | RFC 793 Term | Purpose |
| --- | --- | --- | --- |
| `send_window` | `SharedAsyncValue<u32>` | SND.WND | Available send window |
| `send_window_last_update_seq` | `SeqNumber` | SND.WL1 | Seq used for last window update |
| `send_window_last_update_ack` | `SeqNumber` | SND.WL2 | Ack used for last window update |
| `send_window_scale_shift_bits` | `u8` | - | Window scale factor |
| `mss` | `usize` | - | Maximum Segment Size |

---

### **CongestionControlState**

| Field | Type | Purpose |
| --- | --- | --- |
| `rto_calculator` | `RtoCalculator` | Compute retransmission timeout |
| `cc_algorithm` | `Box<dyn CongestionControl>` | Pluggable CC algorithm |

---

## 📊 **State Modification Matrix**

### **Legend**

- ✅ = Modified
- ❌ = Not modified
- 🔄 = Sometimes modified

| State Component | Setup (SYN/SYN+ACK) | Data Transfer | ACK Processing | Teardown | RST |
| --- | --- | --- | --- | --- | --- |
| `conn_mgmt.state` | ✅ | ❌ | 🔄 | ✅ | ✅ |
| `conn_mgmt.local` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `conn_mgmt.remote` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `delivery.send_unacked` | ✅ | ❌ | ✅ | ✅ | ❌ |
| `delivery.send_next_seq_no` | ✅ | ✅ | ❌ | ✅ | ❌ |
| `delivery.receive_next_seq_no` | ✅ | ✅ | ❌ | ✅ | ❌ |
| `delivery.unacked_queue` | ❌ | ✅ | ✅ | ✅ | ❌ |
| `delivery.unsent_queue` | ❌ | ✅ | ❌ | ❌ | ❌ |
| `delivery.pop_queue` | ❌ | ✅ | ❌ | ✅ | ❌ |
| `delivery.recv_fin_seq_no` | ❌ | ❌ | ❌ | ✅ | ✅ |
| `flow_ctrl.send_window` | ✅ | ❌ | ✅ | ❌ | ❌ |
| `flow_ctrl.mss` | ✅ | ❌ | ❌ | ❌ | ❌ |
| `cong_ctrl.rto_calculator` | ❌ | ❌ | ✅ | ❌ | ❌ |
| `cong_ctrl.cc_algorithm` | ✅ | ✅ | ✅ | ❌ | ❌ |

---

## 🔄 **Background Coroutines**

Established connections run three background coroutines:

### **1. Acknowledger**

Sends delayed ACKs when no piggybacking opportunity occurs.

```rust
async fn acknowledger(cb: &mut ControlBlock, layer3_endpoint: &mut SharedLayer3Endpoint) {
    loop {
        // Wait for ACK deadline
        conditional_yield_until(cb.delivery.ack_deadline_time_secs).await;
        // Send ACK
        cb.delivery.send_ack(&cb.connection_management, layer3_endpoint);
    }
}
```

### **2. Retransmitter**

Handles retransmission of unacknowledged segments.

```rust
async fn background_retransmitter(cb: &mut ControlBlock, ...) {
    loop {
        // Wait for retransmit deadline
        conditional_yield_until(cb.delivery.retransmit_deadline_time_secs).await;
        // Retransmit oldest unacked segment
        retransmit_segment(...);
        // Update deadline
    }
}
```

### **3. Sender**

Processes the unsent queue and transmits data when window allows.

```rust
async fn background_sender(cb: &mut ControlBlock, ...) {
    loop {
        // Get next buffer from unsent queue
        let buffer = cb.delivery.unsent_queue.pop().await;
        // Send when window allows
        rod_send_buffer(buffer, ...).await;
    }
}
```

---

## 📝 **Key Differences from lwIP**

| Aspect | lwIP | Demikernel |
| --- | --- | --- |
| **Architecture** | State-based dispatcher | Phase-based socket types |
| **Concurrency** | Single-threaded callbacks | Async/await coroutines |
| **State Storage** | Single PCB structure | Modular state components |
| **Handshake** | Inline in tcp_input() | Separate socket types |
| **Teardown** | Function per state | Async close coroutines |
| **Window Updates** | Immediate | SharedAsyncValue watchers |
| **Congestion Control** | Built-in | Pluggable trait object |

---

## 🔮 **Feasibility Analysis: Component-Local State Transitions**

This section analyzes whether control path functions can be refactored so each component (`ConnectionManagementState`, `OrderedDeliveryState`, `FlowControlState`, `CongestionControlState`) handles only its own state updates via component-specific event handlers (e.g., `rod.on_syn_in_listen()`, `flow_ctrl.on_syn_in_listen()`).

---

### **Core Question**

Can we replace:

```rust
fn process_syn_in_listen(cb: &mut ControlBlock, seg: &TcpHeader, remote: SocketAddrV4) {
    cb.conn_mgmt.state = State::SynRcvd;
    cb.conn_mgmt.remote = remote;
    cb.rod.irs = seg.seq_num;
    cb.rod.rcv_nxt = seg.seq_num + 1;
    cb.flow_ctrl.snd_wnd = seg.window_size;
    // ...
}
```

With:

```rust
fn input_listen(cb: &mut ControlBlock, seg: &TcpHeader, remote: SocketAddrV4) {
    cb.rod.on_syn_in_listen(seg);           // Only modifies ROD
    cb.flow_ctrl.on_syn_in_listen(seg);     // Only modifies FC
    cb.conn_mgmt.on_syn_in_listen(remote);  // Only modifies CM (state → SYN_RCVD)
}
```

---

### **Problem 1: Cross-Component Read Dependencies**

Several state updates require reading another component’s state to make decisions.

| Update | Reads From | Problem |
| --- | --- | --- |
| CC RTT sampling | ROD’s `unacked_queue[].initial_tx` | CC needs ROD data to compute RTT |
| CC cwnd adjustment | ROD’s `send_unacked`, `send_next` | CC needs bytes-in-flight |
| CM FIN_WAIT_1→FIN_WAIT_2 | ROD’s `sender_fin_seq_no` | CM needs to know if ACK covers FIN |
| ROD retransmit decision | CC’s `rto_calculator.rto()` | ROD needs RTO value |
| Send decision | FC’s `send_window`, CC’s `cwnd` | Both needed for send window |

**Proposed Solution:** Read-only “view” structs passed to handlers:

```rust
impl OrderedDeliveryState {
    fn on_ack(&mut self, seg: &TcpHeader, cc_view: &CongestionControlView) {
        let rto = cc_view.rto(); // Read-only access
        self.update_retransmit_deadline(rto);
    }
}
```

**Feasibility:** ✅ Workable, but adds boilerplate. Must carefully define what each view exposes.

---

### **Problem 2: Ordering Dependencies**

Some events require components to process in a specific order.

**Example: ACK Processing**

```
1. ROD must process ACK first (removes from unacked_queue)
2. CC then samples RTT from the removed segments
3. CM checks if FIN was ACK'd for state transition
```

If CC runs before ROD, it would sample RTT from stale queue data.

**Proposed Solution:** Dispatcher enforces order:

```rust
fn dispatch_ack(&mut self, seg: &TcpHeader, now: Instant) {
    // Order is critical
    self.rod.on_ack(seg, now);                           // 1st
    self.cc.on_ack(seg, now, &self.rod.view());          // 2nd (reads ROD)
    self.conn_mgmt.on_ack(seg, &self.rod.view());        // 3rd (reads ROD)
}
```

**Feasibility:** ✅ Works, but ordering is implicit in dispatcher code—just moves the coupling.

---

### **Problem 3: Atomic Multi-Component Transitions**

Some TCP events require multiple components to update atomically.

**Example: Receiving SYN in LISTEN**
- ROD sets `irs`, `rcv_nxt`, generates `iss`
- FC sets `snd_wnd`, `mss`
- CM sets `remote`, transitions to `SYN_RCVD`

If FC fails mid-update (e.g., invalid MSS), should ROD changes be rolled back?

**Current Behavior:** Single function either succeeds entirely or fails early—no partial state.

**With Component Handlers:** Each handler modifies state independently. Partial failures leave inconsistent state.

**Proposed Solutions:**
1. **Two-phase approach:** Validate in all handlers first, then commit:

```rust
let rod_update = self.rod.prepare_syn(seg)?;
   let fc_update = self.flow_ctrl.prepare_syn(seg)?;
   let cm_update = self.conn_mgmt.prepare_syn()?;
   // All validated, now commit
   rod_update.commit(&mut self.rod);
   fc_update.commit(&mut self.flow_ctrl);
   cm_update.commit(&mut self.conn_mgmt);
```

2. **Accept partial updates:** TCP can recover from most inconsistencies via retransmission.

**Feasibility:** ⚠️ Two-phase adds significant complexity. Accepting partial updates may work but needs careful analysis.

---

### **Problem 4: State-Dependent Behavior in Other Components**

Components sometimes behave differently based on connection state (owned by CM).

**Example:** ROD’s `process_data()` checks state:

```rust
match connection_management.state {
    State::Established | State::FinWait1 | State::FinWait2 => { /* accept data */ },
    state => { warn!("Ignoring data in {:?}", state); return; }
}
```

If ROD can’t read CM’s state, how does it know whether to accept data?

**Proposed Solutions:**
1. **Pass state to handler:** `rod.on_data(seg, data, current_state)`
2. **Event includes state:** `DataReceived { seg, data, state }`
3. **ROD doesn’t check:** Let dispatcher filter events before dispatching

**Feasibility:** ✅ Option 3 is cleanest—dispatcher decides what events each component sees.

---

### **Problem 5: Handlers Returning Actions**

Some component updates trigger outbound actions (send ACK, send RST, etc.).

**Example:** Processing FIN should send ACK:

```rust
// Current: directly calls send
fn check_and_process_fin(cb: &mut ControlBlock, ...) {
    if header.fin {
        cb.delivery.send_ack(&cb.conn_mgmt, layer3_endpoint);
    }
}
```

With component-local handlers, who sends the ACK?

**Proposed Solutions:**
1. **Return actions:** `fn on_fin(&mut self, seg) -> Vec<Action>` where `Action::SendAck`
2. **Post-dispatch hook:** Dispatcher checks if ACK needed after all handlers run
3. **Keep send logic in dispatcher:** Only state updates in handlers

**Feasibility:** ✅ Option 1 or 3 both work. Option 1 is more modular but adds return value complexity.

---

### **Problem 6: ISN Generation Location**

ISN generation currently happens in handshake code and writes to ROD:

```rust
let local_isn = self.isn_generator.generate(&local, &remote);
// ... passed to EstablishedSocket::new() which sets rod.iss
```

ISN generator is owned by `TcpPeer`, not any component.

**Question:** Should `rod.on_syn_in_listen()` generate ISN, or receive it as parameter?

**If ROD generates:** ROD needs access to ISN generator (new dependency)
**If passed in:** Caller still “knows” ROD needs ISN (some coupling remains)

**Feasibility:** ⚠️ Either way introduces some coupling. Passing as parameter is simpler.

---

### **Problem 7: Window Scale Negotiation**

Window scale is negotiated during handshake and affects both FC and ROD:
- FC stores `send_window_scale_shift_bits`
- ROD stores `window_scale_shift_bits` (for receive window)

Both extract from same TCP options in SYN/SYN+ACK.

**With separate handlers:** Both parse options independently (duplicate work) or share parsed results.

**Proposed Solution:** Pre-parse options, pass structured data:

```rust
struct SynInfo {
    seq_num: SeqNumber,
    window_size: u16,
    mss: Option<u16>,
    window_scale: Option<u8>,
}

rod.on_syn_in_listen(&syn_info);
flow_ctrl.on_syn_in_listen(&syn_info);
```

**Feasibility:** ✅ Works well, but requires defining these intermediate structs.

---

### **Problem 8: Rust Borrow Checker Constraints**

The dispatcher needs `&mut` access to multiple components while passing `&` views:

```rust
fn dispatch_ack(&mut self, seg: &TcpHeader) {
    let rod_view = self.rod.view();        // Borrows self.rod immutably
    self.rod.on_ack(seg);                  // ERROR: already borrowed
    self.cc.on_ack(seg, &rod_view);
}
```

**Proposed Solutions:**
1. **Clone view data:** `let rod_view = self.rod.view().clone();` (allocation)
2. **Split struct:** Use `split_borrow` patterns or separate the ControlBlock
3. **Unsafe:** Interior mutability (RefCell) or raw pointers

**Feasibility:** ⚠️ This is a real Rust ergonomics issue. Cloning is safest but has overhead.

---

### **Summary: Feasibility Assessment**

| Problem | Severity | Solvable? | Complexity |
| --- | --- | --- | --- |
| Cross-component reads | Medium | ✅ Yes | View structs |
| Ordering dependencies | Medium | ✅ Yes | Explicit dispatcher order |
| Atomic transitions | High | ⚠️ Partial | Two-phase or accept partial |
| State-dependent behavior | Low | ✅ Yes | Dispatcher filters events |
| Action returns | Low | ✅ Yes | Return actions or post-hooks |
| ISN generation | Low | ✅ Yes | Pass as parameter |
| Option parsing | Low | ✅ Yes | Pre-parse to struct |
| Borrow checker | Medium | ⚠️ Partial | Clone views or split struct |

### **Verdict**

**Feasible with caveats.** The refactoring is possible but:

1. **Doesn’t eliminate coupling—moves it.** Ordering dependencies move from control path functions to dispatcher. Read dependencies become explicit via view structs, but still exist.
2. **Borrow checker friction.** Rust’s ownership model makes “component A reads component B” patterns awkward. Requires cloning or careful struct design.
3. **Atomicity is hard.** Two-phase commit adds significant complexity for marginal benefit in a single-threaded context.
4. **Best candidates for refactoring:**
    - `FlowControlState` — mostly independent, just reads window from headers
    - `CongestionControlState` — clear read-only dependencies on ROD
5. **Hardest to refactor:**
    - `ConnectionManagementState` — state transitions depend on ROD state (FIN ACK’d?)
    - `OrderedDeliveryState` — central to everything, many bidirectional dependencies