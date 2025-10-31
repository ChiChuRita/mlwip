## Rust TCP ↔ lwIP Call Flow Overview

This document explains when and why functions are called across the boundaries between lwIP (C) and the Rust TCP layer:
- IP → TCP (ingress path)
- Sockets/Netconn → TCP (egress/control path)
- Timers → TCP (periodic work)

It also lists the key boundary symbols and shows text visuals for typical flows.

### Legend
- [C] = lwIP C code
- [R] = Rust TCP
- → = direct call
- ⇄ = callbacks or bi-directional interactions

---

### Initialization and Timers

Flow:
```
[C] app → lwip_init()
        ↳ tcp_init()                 (C wrapper)
              → tcp_init_rust()      (R) sets up internal state/timers

Timer scheduling (on-demand):
[C] TCP_REG(...) adds first PCB → tcp_timer_needed() → sys_timeout(TCP_TMR_INTERVAL, tcpip_tcp_timer)
                                         │
                                         ▼
                                tcpip_tcp_timer() → tcp_tmr() (C wrapper) → tcp_tmr_rust() (R)
```

Why:
- `lwip_init()` centralizes module init. When `LWIP_TCP` is enabled, it calls `tcp_init()`.
- The wrapper’s `tcp_init()` forwards to Rust `tcp_init_rust()` where TCP module state and Rust-side timers are prepared.
- TCP timers are only started when needed (there are active/TIME-WAIT PCBs). lwIP’s timer infrastructure calls back into `tcp_tmr()`; the wrapper forwards to `tcp_tmr_rust()`.

Key references:
- `src/core/init.c`: `lwip_init()` calls `tcp_init()`.
- `src/core/timeouts.c`: cyclic timers and `tcp_timer_needed()`.
- `src/core/tcp_rust/wrapper.c`: `tcp_init`, `tcp_tmr` forwarders.

---

### Ingress: IP layer delivering TCP packets (ip_input → tcp_input)

Flow (IPv4 similar for IPv6):
```
[C] ip4_input(pbuf*, netif*)
    └─ classifies packet → TCP protocol →
       tcp_input(pbuf*, netif*)        (C wrapper)
           → tcp_input_rust(p, inp)    (R)
               • parse/process segment
               • advance connection state
               • free p (ffi::pbuf_free) when done
```

Why:
- The IP layer demultiplexes by protocol and hands TCP segments to the TCP core entrypoint.
- The C wrapper preserves lwIP’s public API and translates into the Rust implementation.

Rust → C dependencies commonly used here:
- `pbuf_*` functions for buffer lifetime
- `ip_output_if` when responding (ACK/RST/SYN-ACK)

---

### Egress/Control: Sockets/Netconn API driving TCP

High-level picture:
```
[C] sockets / netconn APIs
    • socket(), bind(), listen(), accept(), connect(), send(), recv(), setsockopt(), close()
        ↳ internal api_msg / netconn flows
            ↳ tcp_*() entrypoints (C wrapper)
                → tcp_*_rust(...) (R)
```

Representative calls across the boundary:
- `tcp_new()` → `tcp_new_rust()`
- `tcp_bind()` → `tcp_bind_rust()`
- `tcp_listen_with_backlog[_and_err]()` → `tcp_listen_with_backlog[_and_err]_rust()`
- `tcp_accept()` (sets callback) → `tcp_accept_rust()`
- `tcp_connect()` → `tcp_connect_rust()`
- `tcp_write()`/`tcp_output()` → `tcp_write_rust()`/`tcp_output_rust()`
- `tcp_close()`/`tcp_abort()` → `tcp_close_rust()`/`tcp_abort_rust()`
- `tcp_recved()` (application consumed data) → `tcp_recved_rust()`
- `tcp_err()/tcp_recv()/tcp_sent()/tcp_poll()` (register callbacks) → `*_rust()`

Callbacks (application upcalls):
```
[R] event (e.g., data received, ACKed, connection established, error)
  → invoke registered C callback pointers:
      tcp_recv_fn / tcp_sent_fn / tcp_connected_fn / tcp_err_fn / tcp_poll_fn
```

Why:
- The lwIP sockets/netconn layers are consumers of the TCP API surface. The wrapper ensures existing C API is intact while delegating behavior to Rust.

---

### Typical Sequences (Text Visuals)

1) Passive open (server listen → accept):
```
[C] socket/bind/listen
  ↳ tcp_new() → [R]
  ↳ tcp_bind() → [R]
  ↳ tcp_listen_with_backlog() → [R]

Incoming SYN:
[C] ip4_input → tcp_input()
  → [R] tcp_input_rust parses SYN, allocates child conn, sets state
  → [R] send SYN-ACK via ip_output_if

ACK completes handshake:
  → [R] transition to ESTABLISHED, set pcb->listener, queue on accept backlog

Application accept callback:
[C] registered tcp_accept() callback is called from [R]
```

2) Active open (client connect):
```
[C] connect()
  ↳ tcp_connect(pcb, addr, port, connected_cb)
      → [R] tcp_connect_rust: craft SYN, send via ip_output_if, set state= SYN_SENT

SYN-ACK arrives:
[C] ip_input → tcp_input()
  → [R] tcp_input_rust validates ack/seq, sets ESTABLISHED
  → [R] invoke connected_cb
```

3) Sending data:
```
[C] send()/write()/netconn_write()
  ↳ tcp_write(pcb, data, len, flags)
      → [R] tcp_write_rust: queue segments, update wnd/queuelen
  ↳ tcp_output(pcb)
      → [R] tcp_output_rust: emit segments via ip_output_if

ACK arrives:
[C] ip_input → tcp_input()
  → [R] free acked segments, update cwnd/ssthresh
  → [R] invoke tcp_sent_fn with acked length
```

4) Receive data:
```
[C] ip_input → tcp_input()
  → [R] place payload into receive queue, update rcv_nxt/wnd
  → [R] invoke tcp_recv_fn(arg, pcb, pbuf*, ERR_OK)

When app consumes:
[C] tcp_recved(pcb, len)
  → [R] tcp_recved_rust: grow advertised window, maybe ACK now
```

5) Close/Abort:
```
[C] close()/shutdown()
  ↳ tcp_close(pcb)
      → [R] tcp_close_rust: FIN if needed; move to closing states; final free in timer path

Error/abort path:
[C] tcp_abort(pcb)
  → [R] immediate teardown; [R] invoke tcp_err_fn(arg, ERR_ABRT)
```

---

### Boundary Symbol Map

From C to Rust (wrapper forwards exactly these):
- `tcp_init`, `tcp_tmr`, `tcp_input`
- `tcp_new`, `tcp_new_ip_type`
- `tcp_bind`, `tcp_connect`, `tcp_shutdown`, `tcp_output`, `tcp_write`
- `tcp_close`, `tcp_abort`, `tcp_recved`
- `tcp_accept`, `tcp_recv`, `tcp_sent`, `tcp_poll`, `tcp_err`
- `tcp_listen_with_backlog`, `tcp_listen_with_backlog_and_err`
- `tcp_bind_netif`, `tcp_setprio`, `tcp_tcp_get_tcp_addrinfo`
- (optional) `tcp_ext_arg_*` if enabled via `LWIP_TCP_PCB_NUM_EXT_ARGS`

From Rust to C (typical dependencies):
- `pbuf_alloc/free/header/realloc` (buffer management)
- `ip_output_if` / `ip4_output_if` / `ip6_output_if` (packet egress)
- `mem_malloc/mem_free` (only when appropriate per lwIP config)
- `sys_timeout/sys_untimeout` (timer integration)
- Application callbacks supplied from C (`tcp_recv_fn`, etc.)

---

### Notes on PCBs and Ext Args

- The public API and some macros in lwIP access PCB fields directly (e.g., `tcp_sndbuf`, `tcp_sndqueuelen`). Under `LWIP_USE_RUST_TCP`, the wrapper preserves the C handle while the Rust layer owns the TCP algorithm/state. Ext-arg slots (`LWIP_TCP_PCB_NUM_EXT_ARGS`) can be used to attach Rust state to a C `tcp_pcb`.
- A fully opaque Rust-owned PCB is possible if C-side field consumers are switched to function calls (instead of macros) under the Rust backend configuration.

---

### Minimal Entry/Exit Points (Checklist)
- Called during init: `tcp_init()` → `tcp_init_rust()`
- Periodic: `tcp_tmr()` → `tcp_tmr_rust()` (scheduled via `tcp_timer_needed()`)
- Ingress: `tcp_input()` → `tcp_input_rust()`
- Sockets/Netconn control/data: `tcp_*` → `tcp_*_rust`
- App callbacks invoked by Rust: `tcp_recv_fn`, `tcp_sent_fn`, `tcp_connected_fn`, `tcp_err_fn`, `tcp_poll_fn`
