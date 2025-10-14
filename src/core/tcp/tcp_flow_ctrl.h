/**
 * @file
 * TCP Flow Control
 *
 * State and interface for TCP flow control including receive and send windows,
 * window scaling, and persist timer.
 */

#ifndef LWIP_HDR_TCP_FLOW_CTRL_H
#define LWIP_HDR_TCP_FLOW_CTRL_H

#include "lwip/opt.h"

#if LWIP_TCP

#include "lwip/err.h"
#include "tcp_types.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef u16_t tcpflags_t;

struct tcp_flow_ctrl_state {
  tcpflags_t flags;
#if LWIP_WND_SCALE
#define TF_WND_SCALE   0x0100U
#endif

  tcpwnd_size_t rcv_wnd;
  tcpwnd_size_t rcv_ann_wnd;
  u32_t rcv_ann_right_edge;

  tcpwnd_size_t snd_wnd;
  tcpwnd_size_t snd_wnd_max;
  u32_t snd_wl1;
  u32_t snd_wl2;

#if LWIP_WND_SCALE
  u8_t snd_scale;
  u8_t rcv_scale;
#endif

  u8_t persist_cnt;
  u8_t persist_backoff;
  u8_t persist_probe;
};

#ifdef __cplusplus
}
#endif

#endif /* LWIP_TCP */

#endif /* LWIP_HDR_TCP_FLOW_CTRL_H */
