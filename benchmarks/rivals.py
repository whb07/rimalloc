"""Pinned rival allocators built as shared libraries for the benchmark suites."""
import json
from pathlib import Path

import applications
from common import file_sha256

# glibc is the system allocator and is not built; the others build from pinned releases.
NAMES = ("glibc", "jemalloc", "tcmalloc")
SOURCES = {
    "jemalloc": ("5.3.0", "https://github.com/jemalloc/jemalloc/releases/download/5.3.0/jemalloc-5.3.0.tar.bz2",
                 "2db82d1e7119df3e71b7640219b6dfe84789bc0537983c3b7ac4f7189aecfeaa"),
    "gperftools": ("2.16", "https://github.com/gperftools/gperftools/releases/download/gperftools-2.16/gperftools-2.16.tar.gz",
                   "f12624af5c5987f2cc830ee534f754c3c5961eec08004c26a8b80de015cf056f"),
}
# The harness dlopens each library with RTLD_LOCAL, so a rival's own calls to malloc/free
# (jemalloc's C++ operators, for example) would bind to glibc's; -Bsymbolic-functions binds
# them to the library itself, as when it is the process allocator. jemalloc keeps its default
# initial-exec TLS: --disable-initial-exec-tls made every TLS access a __tls_get_addr call
# (about 24% slower on glibc-thread) and crashed forked Nginx workers. Native workers instead
# reserve static TLS for it (STATIC_TLS_TUNABLE). tcmalloc_minimal omits the profiler and libunwind.
SYMBOLIC = "LDFLAGS=-Wl,-Bsymbolic-functions"
STATIC_TLS_TUNABLE = "glibc.rtld.optional_static_tls=65536"
BUILDS = {
    "jemalloc": ("jemalloc", [SYMBOLIC], "build_lib_shared", "lib/libjemalloc.so.2"),
    "tcmalloc": ("gperftools", ["--enable-minimal", "--disable-static", SYMBOLIC], "libtcmalloc_minimal.la",
                 ".libs/libtcmalloc_minimal.so"),
}


def build(names, artifacts, jobs, driver):
    """Return {name: library path} for the requested rivals; glibc maps to "glibc"."""
    cache = Path(artifacts["cache"]) / "rivals"
    sources = driver.ROOT / "target/bench-app-sources"
    result = {"libraries": {}, "sources": {}, "logs": [], "cache": str(cache)}
    for name in names:
        if name == "glibc":
            result["libraries"][name] = "glibc"
            continue
        package, options, target, output = BUILDS[name]
        version, url, digest = SOURCES[package]
        result["sources"][package] = {"version": version, "url": url, "sha256": digest}
        local = cache / f"{package}-{version}"
        applications.copy_tree(applications.source(package, sources, SOURCES[package]), local)
        # Options live here, not in the cache key, so compare them before reusing a configure.
        marker = local / "bench-configure.json"
        if not (local / "Makefile").exists() or not marker.exists() or json.loads(marker.read_text()) != options:
            driver.execute(["./configure", *options], cache / f"{name}-configure.log", cwd=local)
            marker.write_text(json.dumps(options))
            result["logs"].append(f"{name}-configure.log")
        driver.execute(["make", "-C", str(local), "-j", str(jobs), target], cache / f"{name}-build.log")
        result["logs"].append(f"{name}-build.log")
        library = (local / output).resolve()
        if not library.is_file():
            raise driver.BenchError(f"{name} build did not produce {library}")
        result["libraries"][name] = str(library)
    result["sha256"] = {n: file_sha256(p) for n, p in result["libraries"].items() if p != "glibc"}
    cache.mkdir(parents=True, exist_ok=True)
    driver.write_json(cache / "artifacts.json", result)
    return result
