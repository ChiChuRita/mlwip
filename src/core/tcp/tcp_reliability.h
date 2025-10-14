/**
 * @file
 * TCP Reliable & Ordered Delivery
 *
 * State and interface for TCP reliability including sequence numbers,
 * retransmission, RTT estimation, and segment queues.
 */

#ifndef LWIP_HDR_TCP_RELIABILITY_H
#define LWIP_HDR_TCP_RELIABILITY_H

#include "lwip/opt.h"

#if LWIP_TCP

#include "lwip/err.h"
#include "tcp_types.h"

#ifdef __cplusplus
extern "C" {
#endif

struct tcp_seg;
struct pbuf;

#if LWIP_TCP_SACK_OUT
struct tcp_sack_range {
  u32_t left;
  u32_t right;
};
#endif

typedef u16_t tcpflags_t;

struct tcp_reliability_state {
  tcpflags_t flags;
#define TF_ACK_DELAY   0x01U
#define TF_ACK_NOW     0x02U
#define TF_NODELAY     0x40U
#define TF_NAGLEMEMERR 0x80U
#if LWIP_TCP_TIMESTAMPS
#define TF_TIMESTAMP   0x0400U
#endif
#if LWIP_TCP_SACK_OUT
#define TF_SACK        0x1000U
#endif

  u32_t rcv_nxt;
  u32_t snd_nxt;
  u32_t snd_lbb;
  u32_t lastack;

  struct tcp_seg *unsent;
  struct tcp_seg *unacked;
#if TCP_QUEUE_OOSEQ
  struct tcp_seg *ooseq;
#endif

  struct pbuf *refused_data;

  s16_t rtime;
  s16_t rto;
  u8_t nrtx;

  u32_t rttest;
  u32_t rtseq;
  s16_t sa;
  s16_t sv;

  u8_t dupacks;
  u32_t rto_end;

  u16_t mss;
  u16_t snd_queuelen;
  tcpwnd_size_t snd_buf;

#if LWIP_TCP_SACK_OUT
  struct tcp_sack_range rcv_sacks[LWIP_TCP_MAX_SACK_NUM];
#endif

#if LWIP_TCP_TIMESTAMPS
  u32_t ts_lastacksent;
  u32_t ts_recent;
#endif
};

#ifdef __cplusplus
}
#endif

#endif /* LWIP_TCP */

#endif /* LWIP_HDR_TCP_RELIABILITY_H */
