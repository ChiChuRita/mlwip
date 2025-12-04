/**
 * @file
 * C wrapper for Rust TCP implementation
 *
 * This file provides C-compatible wrappers that call into the Rust TCP implementation.
 * It maintains the same API as the original lwIP TCP, so applications don't need changes.
 *
 * COMPILE FLAG:
 * This file is only compiled when LWIP_USE_RUST_TCP=ON (CMake) or LWIP_USE_RUST_TCP=1 (Makefile).
 * To use the legacy C TCP implementation instead, set LWIP_USE_RUST_TCP=OFF/0.
 *
 * See TCP_BACKEND_SELECTION.md for details on switching between implementations.
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
extern void tcp_init_rust(void);
extern void tcp_input_rust(struct pbuf *p, struct netif *inp);
extern struct tcp_pcb* tcp_new_rust(void);
extern struct tcp_pcb* tcp_new_ip_type_rust(u8_t type);
extern void tcp_tmr_rust(void);
extern err_t tcp_bind_rust(struct tcp_pcb *pcb, const ip_addr_t *ipaddr, u16_t port);
extern err_t tcp_connect_rust(struct tcp_pcb *pcb, const ip_addr_t *ipaddr, u16_t port, tcp_connected_fn connected);
extern err_t tcp_write_rust(struct tcp_pcb *pcb, const void *dataptr, u16_t len, u8_t apiflags);
extern err_t tcp_output_rust(struct tcp_pcb *pcb);
extern err_t tcp_close_rust(struct tcp_pcb *pcb);
extern void tcp_abort_rust(struct tcp_pcb *pcb);
extern void tcp_recved_rust(struct tcp_pcb *pcb, u16_t len);
extern void tcp_arg_rust(struct tcp_pcb *pcb, void *arg);
extern void tcp_recv_rust(struct tcp_pcb *pcb, tcp_recv_fn recv);
extern void tcp_sent_rust(struct tcp_pcb *pcb, tcp_sent_fn sent);
extern void tcp_poll_rust(struct tcp_pcb *pcb, tcp_poll_fn poll, u8_t interval);
extern void tcp_err_rust(struct tcp_pcb *pcb, tcp_err_fn err);
extern void tcp_accept_rust(struct tcp_pcb *pcb, tcp_accept_fn accept);
extern err_t tcp_shutdown_rust(struct tcp_pcb *pcb, int shut_rx, int shut_tx);
extern void tcp_bind_netif_rust(struct tcp_pcb *pcb, const struct netif *netif);
extern struct tcp_pcb* tcp_listen_with_backlog_rust(struct tcp_pcb *pcb, u8_t backlog);
extern struct tcp_pcb* tcp_listen_with_backlog_and_err_rust(struct tcp_pcb *pcb, u8_t backlog, err_t *err);
extern void tcp_setprio_rust(struct tcp_pcb *pcb, u8_t prio);
extern err_t tcp_tcp_get_tcp_addrinfo_rust(struct tcp_pcb *pcb, int local, ip_addr_t *addr, u16_t *port);
extern void tcp_netif_ip_addr_changed_rust(const ip_addr_t *old_addr, const ip_addr_t *new_addr);
#if TCP_LISTEN_BACKLOG
extern void tcp_backlog_delayed_rust(struct tcp_pcb *pcb);
extern void tcp_backlog_accepted_rust(struct tcp_pcb *pcb);
#endif
#if LWIP_TCP_PCB_NUM_EXT_ARGS
extern u8_t tcp_ext_arg_alloc_id_rust(void);
extern void tcp_ext_arg_set_callbacks_rust(struct tcp_pcb *pcb, u8_t id, const struct tcp_ext_arg_callbacks *callbacks);
extern void tcp_ext_arg_set_rust(struct tcp_pcb *pcb, u8_t id, void *arg);
extern void* tcp_ext_arg_get_rust(const struct tcp_pcb *pcb, u8_t id);
#endif

/**
 * Initialize TCP module
 * Called from lwip_init()
 */
void
tcp_init(void)
{
  tcp_init_rust();
}

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
 * Create a new TCP PCB with specific IP type
 * @param type IP address type (IPADDR_TYPE_V4, IPADDR_TYPE_V6, IPADDR_TYPE_ANY)
 * @return pointer to new PCB or NULL on error
 */
struct tcp_pcb *
tcp_new_ip_type(u8_t type)
{
  return tcp_new_ip_type_rust(type);
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

/**
 * Set the argument passed to callbacks
 */
void
tcp_arg(struct tcp_pcb *pcb, void *arg)
{
  if (pcb != NULL) {
    tcp_arg_rust(pcb, arg);
  }
}

/**
 * Set the receive callback
 */
void
tcp_recv(struct tcp_pcb *pcb, tcp_recv_fn recv)
{
  if (pcb != NULL) {
    tcp_recv_rust(pcb, recv);
  }
}

/**
 * Set the sent callback
 */
void
tcp_sent(struct tcp_pcb *pcb, tcp_sent_fn sent)
{
  if (pcb != NULL) {
    tcp_sent_rust(pcb, sent);
  }
}

/**
 * Set the poll callback
 */
void
tcp_poll(struct tcp_pcb *pcb, tcp_poll_fn poll, u8_t interval)
{
  if (pcb != NULL) {
    tcp_poll_rust(pcb, poll, interval);
  }
}

/**
 * Set the error callback
 */
void
tcp_err(struct tcp_pcb *pcb, tcp_err_fn err)
{
  if (pcb != NULL) {
    tcp_err_rust(pcb, err);
  }
}

/**
 * Set the accept callback
 */
void
tcp_accept(struct tcp_pcb *pcb, tcp_accept_fn accept)
{
  if (pcb != NULL) {
    tcp_accept_rust(pcb, accept);
  }
}

/**
 * Shutdown TCP connection
 */
err_t
tcp_shutdown(struct tcp_pcb *pcb, int shut_rx, int shut_tx)
{
  if (pcb == NULL) {
    return ERR_ARG;
  }
  return tcp_shutdown_rust(pcb, shut_rx, shut_tx);
}

/**
 * Bind to a specific network interface
 */
void
tcp_bind_netif(struct tcp_pcb *pcb, const struct netif *netif)
{
  if (pcb != NULL) {
    tcp_bind_netif_rust(pcb, netif);
  }
}

/**
 * Listen for incoming connections
 */
struct tcp_pcb *
tcp_listen_with_backlog_and_err(struct tcp_pcb *pcb, u8_t backlog, err_t *err)
{
  if (pcb == NULL) {
    if (err != NULL) {
      *err = ERR_ARG;
    }
    return NULL;
  }
  return tcp_listen_with_backlog_and_err_rust(pcb, backlog, err);
}

/**
 * Listen for incoming connections (simplified)
 */
struct tcp_pcb *
tcp_listen_with_backlog(struct tcp_pcb *pcb, u8_t backlog)
{
  if (pcb == NULL) {
    return NULL;
  }
  return tcp_listen_with_backlog_rust(pcb, backlog);
}

/**
 * Set connection priority
 */
void
tcp_setprio(struct tcp_pcb *pcb, u8_t prio)
{
  if (pcb != NULL) {
    tcp_setprio_rust(pcb, prio);
  }
}

/**
 * Get TCP address info
 */
err_t
tcp_tcp_get_tcp_addrinfo(struct tcp_pcb *pcb, int local, ip_addr_t *addr, u16_t *port)
{
  if (pcb == NULL) {
    return ERR_ARG;
  }
  return tcp_tcp_get_tcp_addrinfo_rust(pcb, local, addr, port);
}

/**
 * Handle network interface IP address changes
 */
void
tcp_netif_ip_addr_changed(const ip_addr_t *old_addr, const ip_addr_t *new_addr)
{
  tcp_netif_ip_addr_changed_rust(old_addr, new_addr);
}

#if TCP_LISTEN_BACKLOG
/**
 * TCP backlog delayed
 */
void
tcp_backlog_delayed(struct tcp_pcb *pcb)
{
  if (pcb != NULL) {
    tcp_backlog_delayed_rust(pcb);
  }
}

/**
 * TCP backlog accepted
 */
void
tcp_backlog_accepted(struct tcp_pcb *pcb)
{
  if (pcb != NULL) {
    tcp_backlog_accepted_rust(pcb);
  }
}
#endif /* TCP_LISTEN_BACKLOG */

#if LWIP_TCP_PCB_NUM_EXT_ARGS
/**
 * Allocate extension argument ID
 */
u8_t
tcp_ext_arg_alloc_id(void)
{
  return tcp_ext_arg_alloc_id_rust();
}

/**
 * Set extension argument callbacks
 */
void
tcp_ext_arg_set_callbacks(struct tcp_pcb *pcb, u8_t id, const struct tcp_ext_arg_callbacks *callbacks)
{
  if (pcb != NULL) {
    tcp_ext_arg_set_callbacks_rust(pcb, id, callbacks);
  }
}

/**
 * Set extension argument
 */
void
tcp_ext_arg_set(struct tcp_pcb *pcb, u8_t id, void *arg)
{
  if (pcb != NULL) {
    tcp_ext_arg_set_rust(pcb, id, arg);
  }
}

/**
 * Get extension argument
 */
void *
tcp_ext_arg_get(const struct tcp_pcb *pcb, u8_t id)
{
  if (pcb == NULL) {
    return NULL;
  }
  return tcp_ext_arg_get_rust(pcb, id);
}
#endif /* LWIP_TCP_PCB_NUM_EXT_ARGS */

#endif /* LWIP_TCP */
