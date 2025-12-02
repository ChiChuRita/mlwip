#ifndef LWIP_HDR_TEST_TCP_RUST_H
#define LWIP_HDR_TEST_TCP_RUST_H

#include "../lwip_check.h"

#if LWIP_USE_RUST_TCP
Suite *tcp_rust_suite(void);
#endif

#endif /* LWIP_HDR_TEST_TCP_RUST_H */
