/**
 * @file
 * Tests for Rust TCP implementation integration
 */

#include "test_tcp_rust.h"
#include "lwip/tcp.h"
#include "lwip/stats.h"
#include "tcp_helper.h"
#include "lwip/priv/tcp_priv.h"

#if LWIP_USE_RUST_TCP

static void
tcp_rust_setup(void)
{
  tcp_init();
}

static void
tcp_rust_teardown(void)
{
  tcp_remove_all();
}

START_TEST(test_tcp_rust_new)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  if (pcb != NULL) {
    fail_unless(tcp_state_get(pcb) == CLOSED);
    tcp_abort(pcb);
  }
}
END_TEST

START_TEST(test_tcp_rust_new_ip_type)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new_ip_type(IPADDR_TYPE_V4);
  fail_unless(pcb != NULL);

  if (pcb != NULL) {
    fail_unless(tcp_state_get(pcb) == CLOSED);
    tcp_abort(pcb);
  }
}
END_TEST

START_TEST(test_tcp_rust_bind)
{
  struct tcp_pcb *pcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_bind(pcb, IP_ADDR_ANY, 8080);
  fail_unless(err == ERR_OK);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_bind_any)
{
  struct tcp_pcb *pcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_bind(pcb, IP_ADDR_ANY, 9000);
  fail_unless(err == ERR_OK);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_listen)
{
  struct tcp_pcb *pcb;
  struct tcp_pcb *lpcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_bind(pcb, IP_ADDR_ANY, 8080);
  fail_unless(err == ERR_OK);

  lpcb = tcp_listen(pcb);
  fail_unless(lpcb != NULL);

  fail_unless(tcp_state_get(lpcb) == LISTEN);

  tcp_abort(lpcb);
}
END_TEST

START_TEST(test_tcp_rust_listen_backlog)
{
  struct tcp_pcb *pcb;
  struct tcp_pcb *lpcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_bind(pcb, IP_ADDR_ANY, 8081);
  fail_unless(err == ERR_OK);

  lpcb = tcp_listen_with_backlog(pcb, 5);
  fail_unless(lpcb != NULL);

  fail_unless(tcp_state_get(lpcb) == LISTEN);

  tcp_abort(lpcb);
}
END_TEST

START_TEST(test_tcp_rust_connect)
{
  struct tcp_pcb *pcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_connect(pcb, &test_remote_ip, TEST_REMOTE_PORT, NULL);
  fail_unless(err == ERR_OK);

  fail_unless(tcp_state_get(pcb) == SYN_SENT);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_close_closed)
{
  struct tcp_pcb *pcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_close(pcb);
  fail_unless(err == ERR_OK);
}
END_TEST

START_TEST(test_tcp_rust_abort)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_abort_listen)
{
  struct tcp_pcb *pcb;
  struct tcp_pcb *lpcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_bind(pcb, IP_ADDR_ANY, 8082);
  lpcb = tcp_listen(pcb);
  fail_unless(lpcb != NULL);

  tcp_abort(lpcb);
}
END_TEST

START_TEST(test_tcp_rust_setprio)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_setprio(pcb, 100);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_arg)
{
  struct tcp_pcb *pcb;
  int test_data = 42;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_arg(pcb, &test_data);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_sndbuf)
{
  struct tcp_pcb *pcb;
  u16_t sndbuf;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  sndbuf = tcp_sndbuf(pcb);
  (void)sndbuf;

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_sndqueuelen)
{
  struct tcp_pcb *pcb;
  u16_t qlen;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  qlen = tcp_sndqueuelen(pcb);
  fail_unless(qlen == 0);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_state_get)
{
  struct tcp_pcb *pcb;
  enum tcp_state state;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  state = tcp_state_get(pcb);
  fail_unless(state == CLOSED);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_keepalive)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_set_keep_idle(pcb, 60000);
  fail_unless(tcp_get_keep_idle(pcb) == 60000);

  tcp_set_keep_intvl(pcb, 10000);
  fail_unless(tcp_get_keep_intvl(pcb) == 10000);

  tcp_set_keep_cnt(pcb, 5);
  fail_unless(tcp_get_keep_cnt(pcb) == 5);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_nagle)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_nagle_disable(pcb);
  fail_unless(tcp_nagle_disabled(pcb));

  tcp_nagle_enable(pcb);
  fail_unless(!tcp_nagle_disabled(pcb));

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_multiple_pcbs)
{
  struct tcp_pcb *pcb1, *pcb2, *pcb3;
  LWIP_UNUSED_ARG(_i);

  pcb1 = tcp_new();
  pcb2 = tcp_new();
  pcb3 = tcp_new();

  fail_unless(pcb1 != NULL);
  fail_unless(pcb2 != NULL);
  fail_unless(pcb3 != NULL);

  fail_unless(pcb1 != pcb2);
  fail_unless(pcb2 != pcb3);
  fail_unless(pcb1 != pcb3);

  tcp_abort(pcb1);
  tcp_abort(pcb2);
  tcp_abort(pcb3);
}
END_TEST

START_TEST(test_tcp_rust_null_pcb)
{
  err_t err;
  LWIP_UNUSED_ARG(_i);

  err = tcp_bind(NULL, IP_ADDR_ANY, 80);
  fail_unless(err == ERR_ARG);

  err = tcp_close(NULL);
  fail_unless(err == ERR_ARG);

  tcp_abort(NULL);
}
END_TEST

START_TEST(test_tcp_rust_shutdown)
{
  struct tcp_pcb *pcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_shutdown(pcb, 0, 1);
  fail_unless(err == ERR_OK);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_callbacks)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_recv(pcb, NULL);
  tcp_sent(pcb, NULL);
  tcp_err(pcb, NULL);
  tcp_poll(pcb, NULL, 4);
  tcp_accept(pcb, NULL);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_recved)
{
  struct tcp_pcb *pcb;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_recved(pcb, 100);
  tcp_recved(pcb, 200);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_write)
{
  struct tcp_pcb *pcb;
  err_t err;
  char data[] = "Hello";
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  err = tcp_connect(pcb, &test_remote_ip, TEST_REMOTE_PORT, NULL);
  fail_unless(err == ERR_OK);

  err = tcp_write(pcb, data, sizeof(data), TCP_WRITE_FLAG_COPY);
  fail_unless(err == ERR_OK);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_output)
{
  struct tcp_pcb *pcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);

  tcp_connect(pcb, &test_remote_ip, TEST_REMOTE_PORT, NULL);

  err = tcp_output(pcb);
  fail_unless(err == ERR_OK);

  tcp_abort(pcb);
}
END_TEST

START_TEST(test_tcp_rust_server_lifecycle)
{
  struct tcp_pcb *pcb;
  struct tcp_pcb *lpcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);
  fail_unless(tcp_state_get(pcb) == CLOSED);

  err = tcp_bind(pcb, IP_ADDR_ANY, 8888);
  fail_unless(err == ERR_OK);

  lpcb = tcp_listen(pcb);
  fail_unless(lpcb != NULL);
  fail_unless(tcp_state_get(lpcb) == LISTEN);

  err = tcp_close(lpcb);
  fail_unless(err == ERR_OK);
}
END_TEST

START_TEST(test_tcp_rust_client_lifecycle)
{
  struct tcp_pcb *pcb;
  err_t err;
  LWIP_UNUSED_ARG(_i);

  pcb = tcp_new();
  fail_unless(pcb != NULL);
  fail_unless(tcp_state_get(pcb) == CLOSED);

  err = tcp_connect(pcb, &test_remote_ip, TEST_REMOTE_PORT, NULL);
  fail_unless(err == ERR_OK);
  fail_unless(tcp_state_get(pcb) == SYN_SENT);

  tcp_abort(pcb);
}
END_TEST

Suite *
tcp_rust_suite(void)
{
  testfunc tests[] = {
    TESTFUNC(test_tcp_rust_new),
    TESTFUNC(test_tcp_rust_new_ip_type),
    TESTFUNC(test_tcp_rust_bind),
    TESTFUNC(test_tcp_rust_bind_any),
    TESTFUNC(test_tcp_rust_listen),
    TESTFUNC(test_tcp_rust_listen_backlog),
    TESTFUNC(test_tcp_rust_connect),
    TESTFUNC(test_tcp_rust_close_closed),
    TESTFUNC(test_tcp_rust_abort),
    TESTFUNC(test_tcp_rust_abort_listen),
    TESTFUNC(test_tcp_rust_setprio),
    TESTFUNC(test_tcp_rust_arg),
    TESTFUNC(test_tcp_rust_sndbuf),
    TESTFUNC(test_tcp_rust_sndqueuelen),
    TESTFUNC(test_tcp_rust_state_get),
    TESTFUNC(test_tcp_rust_keepalive),
    TESTFUNC(test_tcp_rust_nagle),
    TESTFUNC(test_tcp_rust_multiple_pcbs),
    TESTFUNC(test_tcp_rust_null_pcb),
    TESTFUNC(test_tcp_rust_shutdown),
    TESTFUNC(test_tcp_rust_callbacks),
    TESTFUNC(test_tcp_rust_recved),
    TESTFUNC(test_tcp_rust_write),
    TESTFUNC(test_tcp_rust_output),
    TESTFUNC(test_tcp_rust_server_lifecycle),
    TESTFUNC(test_tcp_rust_client_lifecycle),
  };
  return create_suite("TCP_RUST", tests, sizeof(tests)/sizeof(testfunc),
                      tcp_rust_setup, tcp_rust_teardown);
}

#endif /* LWIP_USE_RUST_TCP */
