# TCP Modularization Summary

## Overview

We've created a modular TCP architecture for lwIP by separating TCP functionality into five independent modules based on functional concerns (not directional input/output). This document summarizes the design decisions and structure implemented.

## Design Principles

### 1. Functional Separation Over Directional Separation

**Original lwIP Structure** (Directional):
- `tcp_in.c` - All input processing (all concerns mixed)
- `tcp_out.c` - All output processing (all concerns mixed)
- `tcp.c` - Core management

**New Modular Structure** (Functional):
- Each module handles **both input and output** for its specific concern
- Modules are organized by **what they do**, not when they run

### 2. Maximum Modularity

✅ **Each module is self-contained**
- No module includes another module's header
- No direct state access between modules
- Dependencies are explicit through function parameters

✅ **Clear ownership**
- Each module owns its state and flags
- Cross-module communication via control logic coordination

✅ **Easy to test and swap**
- Modules can be unit tested independently
- Easy to replace implementations (e.g., different congestion control algorithms)

## Module Breakdown

### Module 1: Connection Management (`tcp_conn_mgmt`)

**Responsibility**: TCP state machine, connection lifecycle, keepalive, application callbacks

**Owns**:
- `enum tcp_state` - TCP states (CLOSED, LISTEN, SYN_SENT, ESTABLISHED, etc.)
- Connection timers (`tmr`, `last_timer`)
- Application polling (`polltmr`, `pollinterval`)
- Keepalive state (`keep_idle`, `keep_intvl`, `keep_cnt`, `keep_cnt_sent`)
- Callbacks (`recv`, `sent`, `connected`, `poll`, `errf`, `callback_arg`)
- Flags: `TF_FIN`, `TF_RXCLOSED`, `TF_CLOSEPEND`, `TF_BACKLOGPEND`

**Functions** (to be implemented):
- State transitions
- SYN/FIN/RST processing
- Keepalive generation

### Module 2: Reliable & Ordered Delivery (`tcp_reliability`)

**Responsibility**: Sequence numbers, ACK processing, retransmission, RTT estimation, segment queues

**Owns**:
- Sequence numbers (`rcv_nxt`, `snd_nxt`, `snd_lbb`, `lastack`)
- Segment queues (`unsent`, `unacked`, `ooseq`, `refused_data`)
- Retransmission state (`rtime`, `rto`, `nrtx`, `rto_end`, `dupacks`)
- RTT estimation (`rttest`, `rtseq`, `sa`, `sv`)
- MSS (`mss` - negotiated during handshake)
- Send buffer (`snd_buf`, `snd_queuelen`)
- SACK (`rcv_sacks[]`)
- Timestamps (`ts_lastacksent`, `ts_recent`)
- Flags: `TF_ACK_DELAY`, `TF_ACK_NOW`, `TF_NODELAY`, `TF_NAGLEMEMERR`, `TF_TIMESTAMP`, `TF_SACK`

**Functions** (to be implemented):
- ACK processing
- Out-of-order segment handling
- RTT updates
- Retransmission logic
- Data enqueueing

**Note**: MSS lives here but is passed as a parameter to other modules that need it (e.g., congestion control)

### Module 3: Flow Control (`tcp_flow_ctrl`)

**Responsibility**: Window management, window scaling, zero-window probing

**Owns**:
- Receive window (`rcv_wnd`, `rcv_ann_wnd`, `rcv_ann_right_edge`)
- Send window (`snd_wnd`, `snd_wnd_max`)
- Window update tracking (`snd_wl1`, `snd_wl2`)
- Window scaling (`snd_scale`, `rcv_scale`)
- Persist timer (`persist_cnt`, `persist_backoff`, `persist_probe`)
- Flags: `TF_WND_SCALE`

**Functions** (to be implemented):
- Window updates from peer
- Receive window calculation
- Zero-window probing
- Persist timer management

### Module 4: Congestion Control (`tcp_congestion`)

**Responsibility**: Congestion window management, slow start, congestion avoidance, fast recovery

**Owns**:
- Congestion window (`cwnd`)
- Slow start threshold (`ssthresh`)
- Congestion avoidance state (`bytes_acked`)
- Flags: `TF_INFR` (in fast recovery), `TF_RTO` (RTO recovery)

**Functions** (to be implemented):
- cwnd updates on ACK
- Slow start algorithm
- Congestion avoidance algorithm
- Fast recovery entry/exit
- Duplicate ACK handling

**Note**: Receives MSS and connection state as parameters, doesn't import from other modules

### Module 5: Demultiplexing (`tcp_dmux`)

**Responsibility**: PCB lookup, address/port management, network interface binding

**Owns**:
- Port numbers (`local_port`, `remote_port`)
- IP addresses (`local_ip`, `remote_ip`)
- Network interface binding (`netif_idx`)

**Functions** (to be implemented):
- 4-tuple PCB lookup
- Bind operations
- Connect operations

## File Structure

```
src/core/tcp/
├── tcp_types.h          - Shared protocol-level types (tcpwnd_size_t)
├── tcp_conn_mgmt.h      - Connection management state + interface
├── tcp_reliability.h    - Reliability state + interface
├── tcp_flow_ctrl.h      - Flow control state + interface
├── tcp_congestion.h     - Congestion control state + interface
├── tcp_dmux.h           - Demux state + interface
└── tcp_pcb.h            - Main PCB structure (composes all modules)
```

## Header Architecture

### Shared Types (`tcp_types.h`)

Contains **only protocol-level configuration types** that affect multiple modules uniformly:

```c
#if LWIP_WND_SCALE
typedef u32_t tcpwnd_size_t;
#else
typedef u16_t tcpwnd_size_t;
#endif
```

### Module Headers

Each module header (`tcp_<module>.h`) contains:
1. **State structure** - Module-specific state variables
2. **Module-specific flags** - Only flags owned by this module
3. **Function declarations** - To be added in implementation phase

**Include dependencies**:
- Connection Management: Only `lwip/err.h` (most independent)
- Reliability: `tcp_types.h` (needs window size types)
- Flow Control: `tcp_types.h` (needs window size types)
- Congestion Control: `tcp_types.h` (needs window size types)
- Demux: Only `lwip/ip_addr.h`, `lwip/err.h`

**No module includes another module's header** ✅

### PCB Structure (`tcp_pcb.h`)

The main PCB structure that composes all modules:

```c
struct tcp_pcb {
  struct tcp_conn_mgmt_state conn_mgmt;
  struct tcp_reliability_state reliability;
  struct tcp_flow_ctrl_state flow_ctrl;
  struct tcp_congestion_state congestion;
  struct tcp_dmux_state dmux;

  #if TCP_OVERSIZE
  u16_t unsent_oversize;
  #endif

  #if LWIP_TCP_PCB_NUM_EXT_ARGS
  struct tcp_pcb_ext_args *ext_args;
  #endif
};
```

## Key Design Decisions

### 1. Flag Distribution

Instead of one global `flags` field, each module has its own flags:

| Module | Flags Owned |
|--------|------------|
| Connection Management | `TF_FIN`, `TF_RXCLOSED`, `TF_CLOSEPEND`, `TF_BACKLOGPEND` |
| Reliability | `TF_ACK_DELAY`, `TF_ACK_NOW`, `TF_NODELAY`, `TF_NAGLEMEMERR`, `TF_TIMESTAMP`, `TF_SACK` |
| Flow Control | `TF_WND_SCALE` |
| Congestion Control | `TF_INFR`, `TF_RTO` |

Each module can only access and modify its own flags.

### 2. MSS Location

MSS lives in **reliability module** because:
- MSS is negotiated during TCP handshake (connection establishment)
- Reliability manages segment creation and knows segment sizes
- Other modules (congestion control) receive MSS as a **function parameter**

Example:
```c
tcp_congestion_on_ack(&pcb->congestion, acked, pcb->reliability.mss, state);
```

### 3. State Location

`enum tcp_state` lives in **connection management** because:
- State machine is part of connection lifecycle
- Other modules receive state as a **function parameter** when needed

### 4. Cross-Module Communication

**Pattern**: Control logic coordinates module interactions

```c
// In tcp_input() control logic:

// 1. Reliability processes ACK, returns bytes acked
tcpwnd_size_t acked;
tcp_reliability_input_ack(&pcb->reliability, ackno, &acked);

// 2. Control logic passes result to congestion control
tcp_congestion_on_ack(&pcb->congestion, acked, pcb->reliability.mss, pcb->conn_mgmt.state);
                                           ^^^^ passed as parameter ^^^^

// 3. Flow control updates independently
tcp_flow_ctrl_input_window(&pcb->flow_ctrl, wnd, seqno, ackno);
```

**Benefits**:
- ✅ Dependencies are explicit (function signatures)
- ✅ No hidden coupling between modules
- ✅ Easy to test with mock parameters
- ✅ Clear ownership of each piece of state

## Comparison with Original lwIP

### Original Structure

```
tcp.h (struct tcp_pcb)
├── All state variables mixed together
├── Single flags field with all flags
└── No clear boundaries

tcp_in.c
├── Demux logic
├── State machine logic
├── ACK processing logic
├── Window update logic
├── Congestion control logic (input side)
└── All concerns intermingled

tcp_out.c
├── Segment creation logic
├── Retransmission logic
├── Window enforcement logic
├── Congestion control logic (output side)
└── All concerns intermingled
```

**Problem**: Hard to modify one concern without affecting others

### Modular Structure

```
tcp_pcb.h (struct tcp_pcb)
├── conn_mgmt (state machine, callbacks)
├── reliability (sequences, queues, RTT)
├── flow_ctrl (windows, persist)
├── congestion (cwnd, ssthresh)
└── dmux (ports, IPs)

tcp_conn_mgmt.c (future)
└── All connection management logic (input + output)

tcp_reliability.c (future)
└── All reliability logic (input + output)

tcp_flow_ctrl.c (future)
└── All flow control logic (input + output)

tcp_congestion.c (future)
└── All congestion control logic (input + output)

tcp_dmux.c (future)
└── All demux logic
```

**Benefit**: Each concern is isolated and swappable

## Benefits Achieved

1. **✅ Clear Functional Boundaries**: Each module has a single, well-defined responsibility

2. **✅ True Modularity**: No cross-module header includes, explicit dependencies

3. **✅ Swappable Components**: Easy to replace congestion control algorithm without touching reliability

4. **✅ Testability**: Each module can be unit tested independently with mock parameters

5. **✅ Hardware Offload Ready**: Clear interfaces for offloading specific functions (e.g., offload congestion control to NIC)

6. **✅ Easier Verification**: Smaller state spaces per module make formal verification feasible

7. **✅ Better Code Organization**: Related functionality (input + output for a concern) is together

8. **✅ Independent from Old Implementation**: No dependency on `tcpbase.h`, completely self-contained

## Next Steps (Implementation Phase)

1. **Add function declarations** to interface headers
2. **Create .c implementation files** for each module
3. **Extract logic** from tcp_in.c, tcp_out.c, tcp.c into appropriate modules
4. **Implement control logic** to coordinate modules
5. **Create unit tests** for each module
6. **Integration testing** to verify behavioral equivalence
7. **Performance benchmarking** to measure modularization overhead

## Current Status

✅ **Complete**: Header structure with state definitions
- All 5 module headers created
- State structures defined
- Flags distributed appropriately
- PCB structure defined
- No linter errors
- Maximum modularity achieved

🔜 **Next**: Function declarations and implementation

---

*This modularization provides a foundation for modern TCP research, enabling rapid experimentation with new transport protocol features while maintaining a clean, verifiable architecture.*
