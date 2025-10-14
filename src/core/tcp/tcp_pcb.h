/**
 * @file
 * TCP Protocol Control Block (Modular)
 *
 * Main TCP PCB structure that composes all modular TCP components.
 * This is the modular replacement for struct tcp_pcb in lwip/tcp.h.
 */

#ifndef LWIP_HDR_TCP_PCB_H
#define LWIP_HDR_TCP_PCB_H

#include "lwip/opt.h"

#if LWIP_TCP

#include "lwip/ip_addr.h"
#include "tcp_conn_mgmt.h"
#include "tcp_reliability.h"
#include "tcp_flow_ctrl.h"
#include "tcp_congestion.h"
#include "tcp_dmux.h"

#ifdef __cplusplus
extern "C" {
#endif

#if LWIP_CALLBACK_API
struct tcp_pcb;

typedef err_t (*tcp_accept_fn)(void *arg, struct tcp_pcb *newpcb, err_t err);
#endif

#if LWIP_TCP_PCB_NUM_EXT_ARGS
struct tcp_pcb_ext_args;
#endif

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

struct tcp_pcb_listen {
  struct tcp_dmux_state dmux;

#if LWIP_CALLBACK_API
  tcp_accept_fn accept;
#endif

#if TCP_LISTEN_BACKLOG
  u8_t backlog;
  u8_t accepts_pending;
#endif

  void *callback_arg;

#if LWIP_TCP_PCB_NUM_EXT_ARGS
  struct tcp_pcb_ext_args *ext_args;
#endif
};

#ifdef __cplusplus
}
#endif

#endif /* LWIP_TCP */

#endif /* LWIP_HDR_TCP_PCB_H */
