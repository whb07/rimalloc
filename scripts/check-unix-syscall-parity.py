#!/usr/bin/env python3
"""Check Linux mapping ownership and syscall parity against C mimalloc."""

from __future__ import annotations

import os
import pathlib
import re
import shutil
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/unix-syscall-parity"

PROBE_C = r"""
#define _GNU_SOURCE
#include <mimalloc.h>
#include <mimalloc/prim.h>
#include <mimalloc/types.h>

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

int main(void) {
  size_t page = _mi_os_page_size();
  void* p = NULL;
  bool is_large = true;
  bool is_zero = false;
  mi_memid_t memid = _mi_memid_none();

  fprintf(stderr, "@@BEGIN_UNIX_SYSCALL_PROBE\n");

  if (_mi_os_alloc(page * 4, &memid) == NULL) return 10;
  _mi_os_free(memid.mem.os.base, memid.mem.os.size, memid);

  int err = _mi_prim_alloc(NULL, page * 4, page, false, false, &is_large, &is_zero, &p);
  if (err != 0 || p == NULL) return 11;
  if (_mi_prim_commit(p, page * 4, &is_zero) != 0) return 12;
  if (_mi_prim_reset(p, page * 4) != 0) return 13;
  bool needs_recommit = false;
  if (_mi_prim_decommit(p, page * 4, &needs_recommit) != 0) return 14;
  if (needs_recommit && _mi_prim_commit(p, page * 4, &is_zero) != 0) return 15;
  if (_mi_prim_protect(p, page, true) != 0) return 16;
  if (_mi_prim_protect(p, page, false) != 0) return 17;
  _mi_prim_free(p, page * 4);

  fprintf(stderr, "@@END_UNIX_SYSCALL_PROBE\n");
  return 0;
}
"""


SYSCALL_RE = re.compile(r"^(?:\[[^\]]+\]\s+)?(?P<name>mmap|munmap|mprotect|madvise)\(")
MPROTECT_RE = re.compile(r"mprotect\([^,]+,\s*[^,]+,\s*(?P<prot>[^)]+)\)")
MMAP_RE = re.compile(r"mmap\([^,]+,\s*[^,]+,\s*(?P<prot>[^,]+),")
MADVISE_RE = re.compile(r"madvise\([^,]+,\s*[^,]+,\s*(?P<advice>[^)]+)\)")


def rust_staticlib(*, release: bool = False, secure: bool = False) -> pathlib.Path:
    env = os.environ.copy()
    rustflags = env.get("RUSTFLAGS", "")
    env["RUSTFLAGS"] = f"{rustflags} --cfg mi_internal_c_probe".strip()
    if release:
        # The external C linker needs native archive members, as in the release
        # behavioral-parity probe; fat-LTO archives can contain LLVM bitcode.
        env["RUSTFLAGS"] += " -C lto=off -C linker-plugin-lto=no -C embed-bitcode=no"
    suffix = ("-release" if release else "") + ("-secure" if secure else "")
    target_dir = BUILD_DIR / f"rust-target{suffix}"
    subprocess.run(
        [
            "cargo",
            "rustc",
            "--target-dir",
            str(target_dir),
            *(["--release"] if release else []),
            *(["--features", "secure"] if secure else []),
            "--lib",
            "--",
            "--crate-type",
            "staticlib",
        ],
        cwd=ROOT,
        env=env,
        check=True,
    )
    libs = sorted(
        (target_dir / ("release" if release else "debug") / "deps").glob("librimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def compile_binary(
    cc: str, output: pathlib.Path, objects: list[pathlib.Path], *, wrap_memory: bool = False
) -> None:
    subprocess.run(
        [
            cc,
            "-std=c11",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            *[str(obj) for obj in objects],
            *(["-Wl,--wrap=mmap", "-Wl,--wrap=munmap"] if wrap_memory else []),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(output),
        ],
        cwd=ROOT,
        check=True,
    )


def c_mimalloc_object(cc: str, *, release: bool = False, secure: bool = False) -> pathlib.Path:
    suffix = ("-release" if release else "") + ("-secure" if secure else "")
    obj = BUILD_DIR / f"mimalloc-static{suffix}.o"
    subprocess.run(
        [
            cc,
            "-std=c11",
            "-DMI_MALLOC_OVERRIDE=0",
            *(["-O2", "-DNDEBUG", "-DMI_DEBUG=0"] if release else []),
            *(["-DMI_SECURE=4"] if secure else []),
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            "-I",
            str(ROOT / "mimalloc-3.3.2/src"),
            "-c",
            str(ROOT / "mimalloc-3.3.2/src/static.c"),
            "-o",
            str(obj),
        ],
        cwd=ROOT,
        check=True,
    )
    return obj


def normalize_syscalls(stderr: str) -> list[str]:
    events: list[str] = []
    in_probe = False
    for line in stderr.splitlines():
        if "@@BEGIN_UNIX_SYSCALL_PROBE" in line:
            in_probe = True
            continue
        if "@@END_UNIX_SYSCALL_PROBE" in line:
            break
        if not in_probe:
            continue
        match = SYSCALL_RE.match(line)
        if match is None:
            continue
        name = match.group("name")
        if name == "mmap":
            mmap = MMAP_RE.search(line)
            events.append(f"mmap:{mmap.group('prot') if mmap else '?'}")
        elif name == "mprotect":
            mprotect = MPROTECT_RE.search(line)
            events.append(f"mprotect:{mprotect.group('prot') if mprotect else '?'}")
        elif name == "madvise":
            madvise = MADVISE_RE.search(line)
            advice = madvise.group("advice") if madvise else "?"
            if advice in {"MADV_FREE", "MADV_DONTNEED"}:
                events.append("madvise:purge")
            else:
                events.append(f"madvise:{advice}")
        else:
            events.append(name)
    return events


def run_strace(binary: pathlib.Path) -> list[str]:
    completed = subprocess.run(
        [
            "strace",
            "-e",
            "trace=mmap,munmap,madvise,mprotect",
            str(binary),
        ],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return normalize_syscalls(completed.stderr)


def assert_required(events: list[str], label: str) -> None:
    required = {
        "mmap:PROT_READ|PROT_WRITE",
        "mmap:PROT_NONE",
        "mprotect:PROT_READ|PROT_WRITE",
        "mprotect:PROT_NONE",
        "madvise:purge",
        "munmap",
    }
    missing = sorted(required - set(events))
    if missing:
        raise AssertionError(f"{label} missing syscall classes: {', '.join(missing)}")


def main() -> int:
    if sys.platform != "linux":
        print("skipping: Linux strace syscall parity only")
        return 0
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")
    if shutil.which("strace") is None:
        raise SystemExit("strace not found")

    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    probe = BUILD_DIR / "unix-syscall-probe.c"
    probe.write_text(PROBE_C)

    c_bin = BUILD_DIR / "unix-syscall-c"
    rust_bin = BUILD_DIR / "unix-syscall-rust"
    c_object = c_mimalloc_object(cc)
    rust_library = rust_staticlib()
    compile_binary(cc, c_bin, [probe, c_object])
    compile_binary(cc, rust_bin, [probe, rust_library])

    mapping_probe = ROOT / "tests/unix/hinted_mapping.c"
    mapping_failures = []
    for label, library in (("C", c_object), ("Rust", rust_library)):
        binary = BUILD_DIR / f"hinted-mapping-{label.lower()}"
        compile_binary(cc, binary, [mapping_probe, library], wrap_memory=True)
        for outcome in ("success", "failure"):
            print(f"{label} hinted mapping ({outcome}):", flush=True)
            result = subprocess.run([str(binary), outcome], cwd=ROOT, check=False)
            if result.returncode != 0:
                mapping_failures.append(f"{label} retry {outcome}")
    if mapping_failures:
        raise AssertionError("mapping ownership failed: " + ", ".join(mapping_failures))
    print("ok: C and Rust hinted mappings balance on retry success and failure")

    c_events = run_strace(c_bin)
    rust_events = run_strace(rust_bin)
    assert_required(c_events, "C")
    assert_required(rust_events, "Rust")

    if c_events != rust_events:
        print("C syscall classes:", file=sys.stderr)
        print("\n".join(c_events), file=sys.stderr)
        print("Rust syscall classes:", file=sys.stderr)
        print("\n".join(rust_events), file=sys.stderr)
        return 1

    print(f"ok: {len(c_events)} normalized Unix memory syscall classes match")
    decommit_probe = ROOT / "tests/unix/decommit.c"
    decommit_failures = []
    for secure in (False, True):
        mode = "release-secure" if secure else "release"
        c_library = c_mimalloc_object(cc, release=True, secure=secure)
        rust_library = rust_staticlib(release=True, secure=secure)
        for label, library in (("C", c_library), ("Rust", rust_library)):
            binary = BUILD_DIR / f"decommit-{mode}-{label.lower()}"
            compile_binary(cc, binary, [decommit_probe, library])
            print(f"{label} decommit ({mode}):", flush=True)
            result = subprocess.run([str(binary), str(int(secure))], cwd=ROOT, check=False)
            if result.returncode != 0:
                decommit_failures.append(f"{label} {mode}")
    if decommit_failures:
        raise AssertionError("decommit protection failed: " + ", ".join(decommit_failures))
    print("ok: release and secure-release decommit protections and recommit match")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
