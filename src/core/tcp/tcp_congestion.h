/**
 * @file
 * TCP Congestion Control
 *
 * State and interface for TCP congestion control including congestion window,
 * slow start threshold, and fast recovery state.
 */

#ifndef LWIP_HDR_TCP_CONGESTION_H
#define LWIP_HDR_TCP_CONGESTION_H

#include "lwip/opt.h"

#if LWIP_TCP

#include "lwip/err.h"
#include "tcp_types.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef u16_t tcpflags_t;

struct tcp_congestion_state {
  tcpflags_t flags;
#define TF_INFR        0x04U
#define TF_RTO         0x0800U

  tcpwnd_size_t cwnd;
  tcpwnd_size_t ssthresh;
  tcpwnd_size_t bytes_acked;
};

#ifdef __cplusplus
}
#endif

#endif /* LWIP_TCP */

#endif /* LWIP_HDR_TCP_CONGESTION_H */
