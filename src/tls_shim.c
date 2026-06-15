#include <stddef.h>

extern void _mi_theap_empty;

__thread void* mi_tls_theap_default __attribute__((tls_model("initial-exec"), visibility("hidden"))) = &_mi_theap_empty;
static __thread void* mi_tls_theap_cached __attribute__((tls_model("initial-exec"))) = &_mi_theap_empty;
__thread void* __mi_theap_main __attribute__((tls_model("initial-exec"), visibility("hidden"))) = NULL;
static __thread void* mi_tls_thread_locals __attribute__((tls_model("initial-exec"))) = NULL;
static __thread int mi_tls_recurse __attribute__((tls_model("initial-exec"))) = 0;

void* _mi_tls_theap_default_get(void) {
  return mi_tls_theap_default;
}

void _mi_tls_theap_default_set(void* theap) {
  mi_tls_theap_default = theap;
}

void* _mi_tls_theap_cached_get(void) {
  return mi_tls_theap_cached;
}

void _mi_tls_theap_cached_set(void* theap) {
  mi_tls_theap_cached = theap;
}

void* _mi_tls_theap_main_get(void) {
  return __mi_theap_main;
}

void _mi_tls_theap_main_set(void* theap) {
  __mi_theap_main = theap;
}

void* _mi_tls_thread_locals_get(void) {
  return mi_tls_thread_locals;
}

void _mi_tls_thread_locals_set(void* tls) {
  mi_tls_thread_locals = tls;
}

int _mi_tls_recurse_get(void) {
  return mi_tls_recurse;
}

void _mi_tls_recurse_set(int recurse) {
  mi_tls_recurse = recurse;
}
