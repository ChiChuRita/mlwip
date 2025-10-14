/**
 * @file
 * TCP Demultiplexing
 *
 * State and interface for TCP demultiplexing including port numbers,
 * IP addresses, and network interface binding.
 */

#ifndef LWIP_HDR_TCP_DMUX_H
#define LWIP_HDR_TCP_DMUX_H

#include "lwip/opt.h"

#if LWIP_TCP

#include "lwip/ip_addr.h"
#include "lwip/err.h"

#ifdef __cplusplus
extern "C" {
#endif

struct tcp_dmux_state {
  u16_t local_port;
  u16_t remote_port;

  ip_addr_t local_ip;
  ip_addr_t remote_ip;

  u8_t netif_idx;
};

#ifdef __cplusplus
}
#endif

#endif /* LWIP_TCP */

#endif /* LWIP_HDR_TCP_DMUX_H */
