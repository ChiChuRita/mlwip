# Modularizing TCP Implementations

This document outlines our plan to modularize TCP implementations. Specifically, it answers four questions:

  * Why modularize TCP implementations?
  * Which TCP implementations do we seek to modularize?
  * What does the design of a modular TCP look like?
  * What are the steps involved in implementing a modular TCP?

-----

## Why Modularize TCP?

Modern transports like TCP and QUIC bundle distinct concerns—connection management, reliability, congestion control, and flow control—into a monolithic implementation with tightly coupled code and shared state. This entanglement introduces several costs:

  * **Slower innovation.** Updating one mechanism (e.g., an ACK strategy) often requires modifying unrelated code paths such as buffering, timers, or retransmission logic, due to shared assumptions and state. This coupling makes even small changes risky and time-consuming.
  * **Difficult hardware offload.** NIC and SoC designers rely on clear boundaries to support protocol features in hardware. Entangled implementations obscure these boundaries, complicating efforts to offload selective functionality across multiple protocols.
  * **Harder testing and verification.** Shared state across components makes it harder to isolate bugs and prove correctness, contributing to persistent errors even in production stacks.

Our goal is to show that TCP implementations can be modularized in a principled way, enabling a clear separation of concerns without changing wire format or semantics. Doing so can enable faster iteration, simplify verification, and facilitate clean interfaces for offload and reuse.

-----

## What TCP Implementations Are We Targeting?

We plan to modularize three TCP implementations that differ in complexity and deployment environments:

  * **The smoltcp stack.** A minimal TCP stack written in Rust, deployed in bare-metal and real-time systems on Cortex-M and RISC-V microcontrollers. It is the simplest real-world TCP implementation we could find with any practical adoption.
  * **The lwIP TCP stack.** A C-based TCP stack widely used in embedded systems. It is bundled by nearly every major microcontroller vendor—including Texas Instruments, Intel, and Xilinx—as part of their board-support packages.
  * **The Demikernel TCP stack.** A more complex Rust-based stack designed for datacenter servers. Unlike the others, it was written with some modularity in mind, which makes it a promising target despite its greater complexity.

In all three cases, our modularization is an internal re-architecture, i.e., it preserves wire formats and application interfaces, ensuring compatibility with existing systems. By applying a common set of modularization principles across these diverse implementations, we aim to demonstrate that our approach is both general and practical.

-----

## How to Modularize TCP?

Our approach modularizes TCP by first separating the control path from the data path, and then decomposing the data path into distinct components, such as congestion control, flow control, and reliable, ordered delivery. Crucially, each data-path component has a non-overlapping write scope: it can only modify a disjoint subset of the per-connection state. The figure below illustrates this architecture, which we now describe.

[Figure 1. Control and data separation for TCP. Arrows indicate write permissions.]

The **control path** handles connection setup and teardown (SYN/FIN), resets, exceptional timeouts, and admission/policy decisions. It is the only part of the code permitted to write to the entire connection state, as required for TCP state transitions.

The **data path** handles steady-state transfer and is factored into four components:

  * **Demultiplexing** maps incoming segments to the correct connection, typically using only port numbers and without maintaining state.
  * **Reliable, ordered delivery** tracks sequence numbers, processes ACKs, and triggers retransmissions.
  * **Flow control** enforces both the peer’s advertised window and the sender’s local buffer limits.
  * **Congestion control** adjusts the send rate to avoid overwhelming the network.

Each component may read the full connection state but can only update its own fields. This containment of side effects is key to enabling modular reasoning about correctness and performance.

We will enforce this discipline using Rust’s ownership model. Each event handler will receive a mutable reference only to its corresponding component’s state, ensuring that writes are confined to the appropriate module. The control path is the sole exception: it is allowed mutable access to all state, as required for managing global connection transitions. Components may read other parts of the state as needed, but only through immutable references. This enforcement is compile-time checked, making the boundaries between components precise and robust.

-----

## Steps to Implement a Modular TCP

Implementing such a modular TCP consists of two steps: classifying state and modularizing packet-processing logic. We now explain each in turn.

### State Classification

We first classify each field in the per-connection state maintained by TCP into one of five categories: Connection Management, Reliable Order and Delivery, Flow Control, Congestion Control, and Demultiplexing. In practice, this means taking one large monolithic struct and breaking it into five disjoint structs, each owned by a distinct component.

The classification of many variables is straightforward, but others can be ambiguous. Below are guiding principles for each category.

  * **Connection Management:** TCP’s 11-state state machine governs a connection’s lifecycle. This category includes anything related to state transitions (e.g., timers) and general connection metadata (tuples, endpoints). This state can only be modified by control path logic.
  * **Reliable Order and Delivery:** This captures TCP’s core abstraction of a reliable, ordered byte stream. On the receiver side, this includes the state used to reorder and reassemble incoming data before delivering it to the application. On the sender side, it includes state used to buffer data, send it in order, and track acknowledgments. Note, flow and congestion control belong elsewhere—this category assumes a “perfect network.”
  * **Flow Control:** Flow control prevents overwhelming the receiver. It covers variables that compute or enforce the receiver window or similar limits.
  * **Congestion Control:** This limits how much data is in flight to avoid overloading the network. It includes congestion windows, duplicate-ACK counters, and any configuration of the congestion algorithm itself.
  * **Demultiplexing:** This is typically stateless as it is performed by checking the packet’s port numbers against existing connections. But we’re leaving it here for completeness.

A concrete example of how to classify the per-connection state for `smolTCP` can be found in the appendix. It includes a listing of the entire `smolTCP` state struct and explanations for the classification of each field.

### Modularizing Packet-Processing Logic

Having decomposed the connection state, we now modularize the packet-processing logic so that code paths respect these state boundaries. We explain this process using `smolTCP` as our running example, but the ideas apply more generally.

#### Event Model

TCP responds to three types of events:

1.  Incoming packets from the network,
2.  Application actions such as write or send requests, and
3.  Internal timers (e.g., retransmission, keepalive).

For now, we focus on (1) and (2); timers are deferred for later.

In `smoltcp`, the core packet logic is implemented in two functions:

  * `process()` for handling incoming packets (RX), and
  * `dispatch()` for generating outgoing packets (TX).

Our modularization rewrites these functions so that each event updates only the relevant component’s state.

#### Receiver Path

When a packet arrives, it may contain any combination of:

  * data,
  * acknowledgments (ACKs), and
  * control signals (e.g., SYN, FIN, RST).

For now, we consider only the data path (the ESTABLISHED state of the TCP machine), focusing on data and ACK packets.

In this view, the receiver (`process()`) executes two high-level subroutines:

```
process() {
    // Classify the incoming segment
    ...
    process_ack();
    process_data();
}
```

Each subroutine in-turn invokes event handlers for the data-path components that must update their state:

```
process_ack() {
    process_fc_rx_ack_events();
    process_cc_rx_ack_events();
    process_rod_rx_ack_events();
}
```

```
process_data() {
    process_fc_rx_data_events();
    process_cc_rx_data_events();
    process_rod_rx_data_events();
}
```

#### Sender Path

The sender path (`dispatch()`) handles outgoing data and ACK generation.
It similarly invokes per-component event handlers:

```
dispatch() {
    process_fc_tx_events();
    process_cc_tx_events();
    process_rod_tx_events();
}
```

#### Event Semantics and State Access

This event-based rewrite of TCP raises a natural question: how do we extract the logic from an existing implementation (e.g., the `process()` method in `smolTCP`) and partition it cleanly into well-scoped events?

The key idea is to define an event as any operation that performs a write to a component’s portion of the per-connection state. This corresponds directly to the state decomposition described earlier. Events are never defined solely for reads since data-path logic may freely read from any part of the connection state, but all writes must go through explicit events. Additionally, events must not be nested: each event is a flat, atomic unit of mutation.

This raises an important design question: is our division of the data path into four components—demultiplexing, reliable ordered delivery, flow control, and congestion control—sufficiently precise?

If the answer is yes, then for each external trigger (e.g., packet RX, packet TX, timer firing), we should be able to invoke each relevant component’s handler at most once, in any order, without needing interleaved calls or cross-component coordination. If, instead, a component must be revisited mid-sequence due to dependencies created by another component’s update, it suggests that the current decomposition is too coarse and needs to be refined further.

Note that not all events need to touch all components. For example, a `data_rx` event may involve reliable delivery and flow control but leave congestion control untouched if no congestion-related state is updated along that path. This is a feature, not a bug—it reflects the fact that some logic paths involve only a subset of the components.

-----

## Summary

In summary, this document outlines a concrete path toward a modular TCP architecture—one that isolates concerns, localizes state, and enforces disciplined state access without altering TCP’s external behavior or wire semantics. By applying this framework across diverse implementations—`smoltcp`, `lwIP`, and `Demikernel`—we aim to demonstrate that modularization is both practical and general, not tied to a single codebase or environment. The resulting structure promises to accelerate innovation, simplify testing and reasoning, and establish a foundation upon which verification, hardware offload, and extensibility can be built systematically rather than retrofitted after the fact.


# State Classification

## 1. Connection Management 🤝

This category includes variables related to the connection's lifecycle, state machine, and general metadata.

- **`local_ip`, `remote_ip`, `local_port`, `remote_port`**: These four fields form the connection's unique identifier (the 4-tuple). They are essential metadata for identifying and managing the connection.
- **`netif_idx`**: Stores the network interface index, which is endpoint-specific metadata.
- **`so_options`**, **`flags`**: Socket options (`SO_KEEPALIVE`) and protocol flags (`TF_FIN`) that directly influence the connection's behavior and state transitions.
- **`tos`**, **`ttl`**: "Type of Service" and "Time to Live" are IP-layer parameters configured for the connection's entire lifecycle.
- **`next`**: A pointer to implement a linked list of all active PCBs, which is a core part of managing the set of connections.
- **`callback_arg`**, **`ext_args`**, **`sent`**, **`recv`**, **`connected`**, **`poll`**, **`errf`**: These are all related to the application's interface with the TCP stack, defining how the application is notified of connection lifecycle events (establishment, errors, etc.).
- **`state`**: This is the quintessential connection management variable, storing the current state in TCP’s 11-state finite state machine (e.g., `ESTABLISHED`, `FIN_WAIT_1`).
- **`prio`**: The connection's priority, used for scheduling, which is a management policy.
- **`polltmr`**, **`pollinterval`**, **`last_timer`**, **`tmr`**: These are general-purpose timers used for periodic polling and housekeeping, not specifically for retransmission or flow control.
- **`mss`**: The **Maximum Segment Size** is a fundamental parameter negotiated once during the connection setup (handshake).
- **`listener`**: A pointer to the listening PCB that created this connection, linking it to its origin and managing its lifecycle.
- **`keep_idle`**, **`keep_intvl`**, **`keep_cnt`**, **`keep_cnt_sent`**: All fields related to the **TCP Keepalive** mechanism, which checks if an idle connection is still active, a pure lifecycle management task.

---

## 2. Reliable Order and Delivery 📦

This category contains state for implementing TCP's core abstraction of a reliable, ordered byte stream. It focuses on tracking, buffering, and reassembling data.

- **`rcv_nxt`**: The sequence number of the **next byte** the receiver expects to receive, which is fundamental to ensuring in-order delivery.
- **`rcv_sacks`**: An array of **Selective Acknowledgment (SACK)** ranges, used to inform the sender exactly which out-of-order data blocks have been received.
- **`lastack`**: The sequence number of the **last byte** that was cumulatively acknowledged. This marks the boundary of successfully delivered data.
- **`snd_nxt`**: The sequence number of the **next byte** to be sent by the sender.
- **`snd_lbb`**: The sequence number of the next byte to be buffered from the application, managing the flow of data from the application into TCP's send buffer.
- **`snd_buf`**, **`snd_queuelen`**, **`unsent_oversize`**: These variables all manage the state and size of the **send buffer**, which holds application data before it is sent.
- **`bytes_acked`**: A temporary variable that tracks how many bytes were acknowledged in the current processing round, used to free up space in the `unacked` queue.
- **`unsent`**, **`unacked`**, **`ooseq`**: Pointers to the core data queues: `unsent` holds data not yet transmitted, `unacked` holds data sent but not yet acknowledged, and `ooseq` holds received out-of-sequence data awaiting reassembly.
- **`refused_data`**: A buffer for data that has been correctly received and reassembled but cannot yet be delivered to the application (e.g., the application's read buffer is full).
- **`rtime`**: The countdown for the **retransmission timer**. A timeout is a strong signal of network congestion.
- **`rttest`**, **`rtseq`**, **`sa`**, **`sv`**, **`rto`**: All variables used for **Round-Trip Time (RTT) estimation**. `sa` (smoothed RTT) and `sv` (RTT variance) are used to calculate the `rto` (Retransmission Timeout), which is critical for detecting loss and is a key input for many congestion control algorithms.
- **`nrtx`**: The number of **retransmissions** for a given segment. This is used to implement exponential backoff of the RTO during repeated losses.
- **`dupacks`**: The counter for **duplicate ACKs**. Reaching a threshold (typically 3) triggers fast retransmit and fast recovery, which are core congestion control algorithms.
- **`rto_end`**: A sequence number used to resolve ambiguity when an ACK arrives after a retransmission timeout.
- **`ts_lastacksent`**, **`ts_recent`**: State for the **TCP Timestamps** option. While timestamps also help with reliability (PAWS), their primary role in modern TCP is to enable highly accurate RTT measurements, which are fundamental to advanced congestion control algorithms.

---

## 3. Flow Control 🌊

Flow control prevents the sender from overwhelming the receiver's buffer. These variables are all related to the management of the receiver's advertised window.

- **`rcv_wnd`**: The amount of available space in the local **receive buffer**. This is the basis of the flow control window.
- **`rcv_ann_wnd`**, **`rcv_ann_right_edge`**: The receive window that this host will **advertise** to the sender and its corresponding sequence number.
- **`snd_wnd`**, **`snd_wnd_max`**: The flow control window **advertised by the peer**. The sender must ensure the amount of unacknowledged data does not exceed this value.
- **`snd_wl1`**, **`snd_wl2`**: Used to validate incoming window updates from the peer, preventing issues like Silly Window Syndrome.
- **`persist_cnt`**, **`persist_backoff`**, **`persist_probe`**: State for the **persist timer**, which is used to periodically probe the receiver when its window is zero to see if it has opened up again.
- **`snd_scale`**, **`rcv_scale`**: The **window scaling factors** negotiated during the handshake, allowing the advertised window to be larger than 64KB.

---

## 4. Congestion Control 🚦

This category includes state for preventing the connection from overwhelming the network. It involves estimating network capacity and reacting to signals of congestion like packet loss or delay.

- **`cwnd`**: The **congestion window**, which limits the amount of unacknowledged data allowed in the network. This is the central variable in sender-side congestion control.
- **`ssthresh`**: The **slow start threshold**, which determines the transition point between the aggressive "slow start" phase and the more conservative "congestion avoidance" phase.

---

## 5. Demultiplexing 📥

As noted in the problem description, demultiplexing is typically a stateless action. The logic for demultiplexing uses the 4-tuple (`local_ip`, `remote_ip`, `local_port`, `remote_port`) from an incoming packet to look up the corresponding `tcp_pcb`. Since the fields of the tuple themselves are state belonging to **Connection Management**, this category has no variables.
