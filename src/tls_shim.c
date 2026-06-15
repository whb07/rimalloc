#include <stddef.h>

extern void _mi_theap_empty;

__thread void* mi_tls_theap_default __attribute__((tls_model("initial-exec"), visibility("hidden"))) = &_mi_theap_empty;
static __thread void* mi_tls_theap_cached __attribute__((tls_model("initial-exec"))) = &_mi_theap_empty;
__thread void* __mi_theap_main __attribute__((tls_model("initial-exec"), visibility("hidden"))) = NULL;

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
