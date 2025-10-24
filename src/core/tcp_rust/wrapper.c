/**
 * @file
 * C wrapper for Rust TCP implementation
 *
 * This file provides C-compatible wrappers that call into the Rust TCP implementation.
 * It maintains the same API as the original lwIP TCP, so applications don't need changes.
 */

#include "lwip/opt.h"

#if LWIP_TCP /* don't build if not configured for use in lwipopts.h */

#include "lwip/tcp.h"
#include "lwip/pbuf.h"
#include "lwip/netif.h"
#include "lwip/ip_addr.h"
#include "lwip/err.h"
#include "lwip/priv/tcp_priv.h"

/* External declarations for Rust functions */
extern void tcp_input_rust(struct pbuf *p, struct netif *inp);
extern struct tcp_pcb* tcp_new_rust(void);
extern void tcp_tmr_rust(void);
extern err_t tcp_bind_rust(struct tcp_pcb *pcb, const ip_addr_t *ipaddr, u16_t port);
extern err_t tcp_connect_rust(struct tcp_pcb *pcb, const ip_addr_t *ipaddr, u16_t port, tcp_connected_fn connected);
extern err_t tcp_write_rust(struct tcp_pcb *pcb, const void *dataptr, u16_t len, u8_t apiflags);
extern err_t tcp_output_rust(struct tcp_pcb *pcb);
extern err_t tcp_close_rust(struct tcp_pcb *pcb);
extern void tcp_abort_rust(struct tcp_pcb *pcb);
extern void tcp_recved_rust(struct tcp_pcb *pcb, u16_t len);

/**
 * TCP input function called by IP layer
 * Forwards to Rust implementation
 */
void
tcp_input(struct pbuf *p, struct netif *inp)
{
  tcp_input_rust(p, inp);
}

/**
 * Create a new TCP PCB
 * @return pointer to new PCB or NULL on error
 */
struct tcp_pcb *
tcp_new(void)
{
  return tcp_new_rust();
}

/**
 * TCP timer function - must be called periodically
 */
void
tcp_tmr(void)
{
  tcp_tmr_rust();
}

/**
 * Bind TCP PCB to local address and port
 */
err_t
tcp_bind(struct tcp_pcb *pcb, const ip_addr_t *ipaddr, u16_t port)
{
  if (pcb == NULL) {
    return ERR_ARG;
  }
  return tcp_bind_rust(pcb, ipaddr, port);
}

/**
 * Connect to remote host
 */
err_t
tcp_connect(struct tcp_pcb *pcb, const ip_addr_t *ipaddr,
            u16_t port, tcp_connected_fn connected)
{
  if (pcb == NULL) {
    return ERR_ARG;
  }
  return tcp_connect_rust(pcb, ipaddr, port, connected);
}

/**
 * Write data to TCP connection
 */
err_t
tcp_write(struct tcp_pcb *pcb, const void *dataptr, u16_t len, u8_t apiflags)
{
  if (pcb == NULL || dataptr == NULL) {
    return ERR_ARG;
  }
  return tcp_write_rust(pcb, dataptr, len, apiflags);
}

/**
 * Trigger TCP output
 */
err_t
tcp_output(struct tcp_pcb *pcb)
{
  if (pcb == NULL) {
    return ERR_ARG;
  }
  return tcp_output_rust(pcb);
}

/**
 * Close TCP connection
 */
err_t
tcp_close(struct tcp_pcb *pcb)
{
  if (pcb == NULL) {
    return ERR_ARG;
  }
  return tcp_close_rust(pcb);
}

/**
 * Abort TCP connection
 */
void
tcp_abort(struct tcp_pcb *pcb)
{
  if (pcb != NULL) {
    tcp_abort_rust(pcb);
  }
}

/**
 * Indicate that application has processed received data
 */
void
tcp_recved(struct tcp_pcb *pcb, u16_t len)
{
  if (pcb != NULL) {
    tcp_recved_rust(pcb, len);
  }
}

#endif /* LWIP_TCP */
