#include <stddef.h>

extern void _mi_theap_empty;

typedef struct mi_tls_slot_s {
  size_t version;
  void* value;
} mi_tls_slot_t;

typedef struct mi_thread_locals_s {
  size_t count;
  mi_tls_slot_t slots[1];
} mi_thread_locals_t;

mi_thread_locals_t mi_thread_locals_empty __attribute__((visibility("hidden"))) = { 0, {{0,NULL}} };

__thread void* __mi_theap_default __attribute__((tls_model("initial-exec"), visibility("hidden"))) = &_mi_theap_empty;
__thread void* __mi_theap_cached __attribute__((tls_model("initial-exec"), visibility("hidden"))) = &_mi_theap_empty;
__thread void* __mi_theap_main __attribute__((tls_model("initial-exec"), visibility("hidden"))) = NULL;
__thread void* mi_thread_locals __attribute__((tls_model("initial-exec"), visibility("hidden"))) = &mi_thread_locals_empty;

void* _mi_tls_theap_default_get(void) {
  return __mi_theap_default;
}

void _mi_tls_theap_default_set(void* theap) {
  __mi_theap_default = theap;
}

void* _mi_tls_theap_cached_get(void) {
  return __mi_theap_cached;
}

void _mi_tls_theap_cached_set(void* theap) {
  __mi_theap_cached = theap;
}

void* _mi_tls_theap_main_get(void) {
  return __mi_theap_main;
}

void _mi_tls_theap_main_set(void* theap) {
  __mi_theap_main = theap;
}

void* _mi_tls_thread_locals_get(void) {
  return mi_thread_locals;
}

void _mi_tls_thread_locals_set(void* tls) {
  mi_thread_locals = tls;
}
