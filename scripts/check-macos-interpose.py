#!/usr/bin/env python3
"""Build and inject the macOS dylib into unlinked native C and C++ consumers."""

import os
import pathlib
import platform
import re
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
# libc and Itanium C++ allocation entry points (Mach-O symbols carry a leading underscore).
ALLOCATOR_ALIAS = re.compile(
    r"_(?:malloc|calloc|realloc|reallocf|reallocarray|free|cfree|vfree|valloc|pvalloc"
    r"|memalign|posix_memalign|aligned_alloc|strdup|strndup|realpath"
    r"|malloc_size|malloc_good_size|malloc_usable_size|_Zn[wa]\w*|_Zd[la]\w*)"
)


def run(command, **kwargs):
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(command, cwd=ROOT, check=True, timeout=300, **kwargs)


def main():
    if platform.system() != "Darwin":
        raise SystemExit("macOS interposition validation requires a native Darwin host")
    target = ROOT / "target/macos-interpose"
    include = ROOT / "mimalloc-3.3.2/include"
    # build.rs keeps shim warnings non-fatal for downstream SDKs; keep them fatal
    # here, at each architecture's Rust deployment target.
    for arch, rust_target in [("arm64", "aarch64-apple-darwin"), ("x86_64", "x86_64-apple-darwin")]:
        printed = subprocess.check_output(
            ["rustc", "--print", "deployment-target", "--target", rust_target], text=True)
        version = printed.strip().removeprefix("MACOSX_DEPLOYMENT_TARGET=")
        run([os.environ.get("CC", "cc"), "-arch", arch, f"-mmacosx-version-min={version}",
             "-std=c11", "-fsyntax-only", "-Wall", "-Wextra", "-Werror", "-I", str(include),
             str(ROOT / "src/macos_interpose.c")])
    with tempfile.TemporaryDirectory(prefix="mechanical-mimalloc-dyld-") as temporary:
        work = pathlib.Path(temporary)
        plugin = work / "plugin.dylib"
        run([os.environ.get("CC", "cc"), "-dynamiclib", "-O2", "-fno-builtin",
             str(ROOT / "tests/macos/interpose_plugin.c"), "-o", str(plugin)])
        consumers = []
        for language, compiler, standard in [("c", "CC", "c11"), ("cpp", "CXX", "c++17")]:
            binary = work / f"consumer-{language}"
            run([os.environ.get(compiler, "cc" if language == "c" else "c++"),
                 f"-std={standard}", "-O2", "-fno-builtin", "-Wall", "-Wextra", "-Werror",
                 "-I", str(include),
                 str(ROOT / f"tests/macos/interpose.{language}"), "-o", str(binary)])
            consumers.append(binary)
        for features in ["interpose", "interpose,secure,guarded"]:
            # Cargo must know the final crate type to select proper fat LTO.
            run(["cargo", "rustc", "--target-dir", str(target), "--release", "--lib",
                 "--features", features, "--crate-type", "cdylib"])
            dylib = target / "release/libmechanical_mimalloc.dylib"
            exports = subprocess.check_output(["nm", "-gU", str(dylib)], text=True)
            names = {line.split()[-1] for line in exports.splitlines() if line.split()}
            aliases = sorted(name for name in names if ALLOCATOR_ALIAS.fullmatch(name))
            assert not aliases, (
                f"interposition targets must resolve to libmalloc/libc++, not this dylib: {aliases}"
            )
            environment = dict(os.environ, DYLD_INSERT_LIBRARIES=str(dylib), MIMALLOC_SHOW_ERRORS="0")
            environment.pop("DYLD_FORCE_FLAT_NAMESPACE", None)
            for binary in consumers:
                arguments = [str(binary)]
                if binary.name == "consumer-c":
                    arguments.append(str(plugin))
                run(arguments, env=environment)
            # The Apple process constructor registers teardown with atexit.
            stats = subprocess.run([str(consumers[0]), str(plugin)], cwd=ROOT, check=True,
                                   timeout=300, capture_output=True, text=True,
                                   env=dict(environment, MIMALLOC_SHOW_STATS="1"))
            assert "subproc 0" in stats.stderr, "MIMALLOC_SHOW_STATS printed nothing at exit"
    print("ok: native macOS DYLD injection passed in release and secure/guarded modes")


if __name__ == "__main__":
    main()
