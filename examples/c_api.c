/*
This crate's default release library is an rlib for Rust consumers. C consumers
must build an explicit C ABI artifact or use scripts/build-release-artifacts.py.

Dynamic C ABI example:

  rm -rf target/c-api-example
  cargo rustc --features c-abi --lib --release \
    --target-dir target/c-api-example/dynamic \
    -- --crate-type cdylib
  cc examples/c_api.c \
    -Imimalloc-3.3.2/include \
    target/c-api-example/dynamic/release/deps/libmechanical_mimalloc-*.so \
    -Wl,-rpath,"$PWD/target/c-api-example/dynamic/release/deps" \
    -pthread -ldl -lm \
    -o target/c-api-example/c_api_dynamic
  target/c-api-example/c_api_dynamic

Static C ABI preview example:

  RUSTFLAGS='-C lto=off -C linker-plugin-lto=no -C embed-bitcode=no' \
  cargo rustc --features c-abi --lib --release \
    --target-dir target/c-api-example/static \
    -- --crate-type staticlib
  cc examples/c_api.c \
    -Imimalloc-3.3.2/include \
    target/c-api-example/static/release/deps/libmechanical_mimalloc-*.a \
    -pthread -ldl -lm \
    -o target/c-api-example/c_api_static
  target/c-api-example/c_api_static
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
