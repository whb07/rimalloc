use super::*;

#[path = "platform_primitive_boundaries/macos.rs"]
mod macos;
#[path = "platform_primitive_boundaries/shared_policy.rs"]
mod shared_policy;
#[path = "platform_primitive_boundaries/unix_wasi_emscripten.rs"]
mod unix_wasi_emscripten;
#[path = "platform_primitive_boundaries/windows.rs"]
mod windows;
