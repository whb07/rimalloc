#!/usr/bin/env python3
"""Build and inject the macOS dylib into unlinked native C and C++ consumers."""

import os
import pathlib
import platform
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]


def run(command, **kwargs):
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(command, cwd=ROOT, check=True, timeout=300, **kwargs)


def main():
    if platform.system() != "Darwin":
        raise SystemExit("macOS interposition validation requires a native Darwin host")
    target = ROOT / "target/macos-interpose"
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
                 str(ROOT / f"tests/macos/interpose.{language}"), "-o", str(binary)])
            consumers.append(binary)
        for features in ["interpose", "interpose,secure,guarded"]:
            # Cargo must know the final crate type to select proper fat LTO.
            run(["cargo", "rustc", "--target-dir", str(target), "--release", "--lib",
                 "--features", features, "--crate-type", "cdylib"])
            dylib = target / "release/libmechanical_mimalloc.dylib"
            exports = subprocess.check_output(["nm", "-gU", str(dylib)], text=True)
            names = {line.split()[-1] for line in exports.splitlines() if line.split()}
            targets = {"_malloc", "_calloc", "_realloc", "_free", "__Znwm", "__ZdlPv"}
            assert not names.intersection(targets), (
                "interposition targets must resolve to libmalloc/libc++, not this dylib"
            )
            environment = dict(os.environ, DYLD_INSERT_LIBRARIES=str(dylib), MIMALLOC_SHOW_ERRORS="0")
            environment.pop("DYLD_FORCE_FLAT_NAMESPACE", None)
            for binary in consumers:
                arguments = [str(binary)]
                if binary.name == "consumer-c":
                    arguments.append(str(plugin))
                run(arguments, env=environment)
    print("ok: native macOS DYLD injection passed in release and secure/guarded modes")


if __name__ == "__main__":
    main()
