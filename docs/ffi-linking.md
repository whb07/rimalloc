# FFI Linking

The production-supported C ABI artifact for the current release policy is the
explicit cdylib build. Default Cargo library builds produce an `rlib` for Rust
integration and do not produce a linkable C library.

Build the dynamic C ABI/interposition artifact with:

```sh
cargo rustc --features interpose --lib --release -- --crate-type cdylib
```

Static archives are preview-only. They are used by repository parity tests,
source-build experiments, and static archive audits, but they are not a
production-supported FFI distribution artifact until the static symbol
contract, collision tests, and checked static manifests are complete.

If a release candidate includes static archives for evaluation, the artifact
name must include a preview marker such as
`mechanical_mimalloc-static-preview`, and the release notes must state that the
archive is not a stable static ABI.
