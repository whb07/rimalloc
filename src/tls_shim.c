#include <stddef.h>

static __thread void* mi_tls_theap_default __attribute__((tls_model("initial-exec"))) = NULL;
static __thread void* mi_tls_theap_cached __attribute__((tls_model("initial-exec"))) = NULL;

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
