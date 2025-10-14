/**
 * @file
 * TCP Connection Management
 *
 * State and interface for TCP connection management including state machine,
 * timers, keepalive, and application callbacks.
 */

#ifndef LWIP_HDR_TCP_CONN_MGMT_H
#define LWIP_HDR_TCP_CONN_MGMT_H

#include "lwip/opt.h"

#if LWIP_TCP

#include "lwip/err.h"

#ifdef __cplusplus
extern "C" {
#endif

enum tcp_state {
  CLOSED      = 0,
  LISTEN      = 1,
  SYN_SENT    = 2,
  SYN_RCVD    = 3,
  ESTABLISHED = 4,
  FIN_WAIT_1  = 5,
  FIN_WAIT_2  = 6,
  CLOSE_WAIT  = 7,
  CLOSING     = 8,
  LAST_ACK    = 9,
  TIME_WAIT   = 10
};

#if LWIP_CALLBACK_API
struct tcp_pcb;
struct pbuf;

typedef err_t (*tcp_recv_fn)(void *arg, struct tcp_pcb *tpcb, struct pbuf *p, err_t err);
typedef err_t (*tcp_sent_fn)(void *arg, struct tcp_pcb *tpcb, u16_t len);
typedef err_t (*tcp_connected_fn)(void *arg, struct tcp_pcb *tpcb, err_t err);
typedef err_t (*tcp_poll_fn)(void *arg, struct tcp_pcb *tpcb);
typedef void  (*tcp_err_fn)(void *arg, err_t err);
#endif

struct tcp_pcb_listen;

typedef u16_t tcpflags_t;

struct tcp_conn_mgmt_state {
  enum tcp_state state;

  tcpflags_t flags;
#define TF_FIN         0x20U
#define TF_RXCLOSED    0x10U
#define TF_CLOSEPEND   0x08U
#if TCP_LISTEN_BACKLOG
#define TF_BACKLOGPEND 0x0200U
#endif

  u32_t tmr;
  u8_t last_timer;

  u8_t polltmr;
  u8_t pollinterval;

  u32_t keep_idle;
#if LWIP_TCP_KEEPALIVE
  u32_t keep_intvl;
  u32_t keep_cnt;
#endif
  u8_t keep_cnt_sent;

#if LWIP_CALLBACK_API || TCP_LISTEN_BACKLOG
  struct tcp_pcb_listen* listener;
#endif

#if LWIP_CALLBACK_API
  tcp_sent_fn sent;
  tcp_recv_fn recv;
  tcp_connected_fn connected;
  tcp_poll_fn poll;
  tcp_err_fn errf;
#endif

  void *callback_arg;
};

#ifdef __cplusplus
}
#endif

#endif /* LWIP_TCP */

#endif /* LWIP_HDR_TCP_CONN_MGMT_H */
