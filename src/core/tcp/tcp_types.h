/**
 * @file
 * TCP Modular Types
 *
 * Protocol-level type definitions shared across TCP modules.
 * These types are configuration-driven and affect multiple modules uniformly.
 */

#ifndef LWIP_HDR_TCP_TYPES_H
#define LWIP_HDR_TCP_TYPES_H

#include "lwip/opt.h"

#if LWIP_TCP

#ifdef __cplusplus
extern "C" {
#endif

#if LWIP_WND_SCALE
typedef u32_t tcpwnd_size_t;
#else
typedef u16_t tcpwnd_size_t;
#endif

#ifdef __cplusplus
}
#endif

#endif /* LWIP_TCP */

#endif /* LWIP_HDR_TCP_TYPES_H */
