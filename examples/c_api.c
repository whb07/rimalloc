/*
Build the Rust cdylib first:

  cargo build --release

Then compile and link this example against the generated library:

  cc examples/c_api.c \
    -Imimalloc-3.3.2/include \
    -Ltarget/release \
    -lmechanical_mimalloc \
    -Wl,-rpath,"$PWD/target/release" \
    -o target/c_api_example

  target/c_api_example
*/

#include <assert.h>
#include <mimalloc.h>
#include <stddef.h>

int main(void) {
  void* p = mi_malloc(64);
  assert(p != NULL);
  assert(mi_usable_size(p) >= 64);
  mi_free(p);
  return 0;
}
