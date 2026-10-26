#!/usr/bin/env python3
"""Check preview staticlib link behavior and representative collisions."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
BUILD_DIR = ROOT / "target/static-link-collisions"
INTERNAL_COLLISION_SYMBOL = "_mi_tls_theap_default_get"


def run(cmd: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def rust_staticlib() -> pathlib.Path:
    subprocess.run(
        ["cargo", "rustc", "--lib", "--", "--crate-type", "staticlib"],
        cwd=ROOT,
        check=True,
    )
    libs = sorted(
        [
            *(ROOT / "target/debug/deps").glob("librimalloc-*.a"),
            *(ROOT / "target/debug/deps").glob("librimalloc.a"),
            *(ROOT / "target/debug").glob("librimalloc.a"),
        ],
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def write_sources() -> tuple[pathlib.Path, pathlib.Path, pathlib.Path]:
    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    smoke = BUILD_DIR / "static-link-smoke.c"
    support = BUILD_DIR / "nonconflicting-support.c"
    collision = BUILD_DIR / "internal-collision.c"

    smoke.write_text(
        """\
#include <mimalloc.h>
#include <stdlib.h>

extern int support_identity(void);

int main(void) {
  if (support_identity() != 7) return 10;

  void* p = mi_malloc(64);
  if (p == NULL) return 20;
  if (!mi_check_owned(p)) return 21;
  mi_free(p);

  void* q = malloc(32);
  if (q == NULL) return 30;
  if (!mi_check_owned(q)) return 31;
  free(q);

  return 0;
}
"""
    )
    support.write_text(
        """\
int support_identity(void) {
  return 7;
}

int representative_nonconflicting_symbol(void) {
  return 11;
}
"""
    )
    collision.write_text(
        f"""\
void* {INTERNAL_COLLISION_SYMBOL}(void) {{
  return 0;
}}
"""
    )
    return smoke, support, collision


def compile_object(cc: str, source: pathlib.Path, output: pathlib.Path) -> None:
    run([cc, "-std=c11", "-c", str(source), "-o", str(output)])


def link_static_smoke(
    cc: str, smoke: pathlib.Path, support_o: pathlib.Path, staticlib: pathlib.Path
) -> None:
    binary = BUILD_DIR / "static-link-smoke"
    run(
        [
            cc,
            "-std=c11",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            str(smoke),
            str(support_o),
            str(staticlib),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ]
    )
    subprocess.run([str(binary)], cwd=ROOT, check=True)


def assert_internal_symbol_is_present(staticlib: pathlib.Path) -> None:
    nm = run(["nm", "-g", "--defined-only", str(staticlib)])
    if INTERNAL_COLLISION_SYMBOL not in nm.stdout:
        raise SystemExit(
            f"expected preview internal static symbol missing: {INTERNAL_COLLISION_SYMBOL}"
        )


def assert_collision_fails(
    cc: str,
    smoke: pathlib.Path,
    support_o: pathlib.Path,
    collision_o: pathlib.Path,
    staticlib: pathlib.Path,
) -> None:
    binary = BUILD_DIR / "static-link-collision"
    result = run(
        [
            cc,
            "-std=c11",
            "-I",
            str(ROOT / "mimalloc-3.3.2/include"),
            str(smoke),
            str(support_o),
            str(collision_o),
            str(staticlib),
            "-pthread",
            "-ldl",
            "-lm",
            "-o",
            str(binary),
        ],
        check=False,
    )
    output = f"{result.stdout}\n{result.stderr}"
    if result.returncode == 0:
        raise SystemExit("static collision fixture linked successfully but should fail")
    if INTERNAL_COLLISION_SYMBOL not in output:
        raise SystemExit(
            "static collision fixture failed for an unexpected reason:\n" + output
        )


def main() -> int:
    cc = os.environ.get("CC") or shutil.which("cc") or shutil.which("gcc")
    if cc is None:
        raise SystemExit("no C compiler found; set CC")

    staticlib = rust_staticlib()
    smoke, support, collision = write_sources()
    support_o = BUILD_DIR / "nonconflicting-support.o"
    collision_o = BUILD_DIR / "internal-collision.o"
    compile_object(cc, support, support_o)
    compile_object(cc, collision, collision_o)

    assert_internal_symbol_is_present(staticlib)
    link_static_smoke(cc, smoke, support_o, staticlib)
    assert_collision_fails(cc, smoke, support_o, collision_o, staticlib)

    print(
        "ok: static preview link smoke passes and representative internal "
        f"{INTERNAL_COLLISION_SYMBOL} collision fails as documented"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
