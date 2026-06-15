static __thread unsigned char recurse __attribute__((tls_model("initial-exec"))) = 0;

int _mi_tls_recurse_get(void) {
  return recurse != 0;
}

void _mi_tls_recurse_set(int value) {
  recurse = (value != 0);
}
