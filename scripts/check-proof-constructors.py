#!/usr/bin/env python3
"""Compile valid proof handoffs and reject the corresponding invalid callers."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
PROBES = [
    (
        "arena-claim-authorizes-matching-bitmap-updates",
        "src/types/arena.rs",
        "fn probe(mut claim: ClaimedArenaSlices<'_, '_>) { let _ = claim.mark_dirty(); "
        "let _ = claim.committed_count(); claim.mark_committed(None); claim.clear_committed(); }",
        None,
    ),
    (
        "arena-claim-rejects-independent-arena-and-range",
        "src/types/arena.rs",
        "fn probe(mut claim: ClaimedArenaSlices<'_, '_>, arena: ArenaPtr, "
        "range: crate::core_types::SliceRange) { let _ = claim.mark_dirty(arena, range); }",
        "E0061",
    ),
    (
        "arena-claim-cannot-outlive-borrowed-access",
        "src/types/arena.rs",
        "unsafe fn probe(arena: ArenaPtr, count: SliceCount) { let claim; "
        "{ let access = unsafe { ArenaSliceAccess::from_raw(arena) }; "
        "claim = access.try_claim(count, crate::core_types::ThreadSeq::new(0)); } "
        "drop(claim); }",
        "E0597",
    ),
    (
        "arena-publication-consumes-the-rollback-claim",
        "src/types/arena.rs",
        "fn probe(claim: ClaimedArenaSlices<'_, '_>) { "
        "let _ = claim.publish(); drop(claim); }",
        "E0382",
    ),
    (
        "terminal-release-requires-detached-all-free-page",
        "src/allocator/page.rs",
        "unsafe fn probe(p: crate::core_types::AllFreePage<crate::core_types::Active>) { "
        "let _ = unsafe { _mi_page_free_after_remove(p) }; }",
        "E0308",
    ),
    (
        "all-free-membership-consumed-on-detach",
        "src/types/page_queues.rs",
        "fn probe(p: AllFreeQueued<'_, NormalQueue>) { "
        "let _ = p.remove(); let _ = p.remove(); }",
        "E0382",
    ),
    (
        "detached-all-free-page-cannot-escape",
        "src/types/page_queues.rs",
        "unsafe fn probe(q: NormalPageQueue, h: super::super::ThreadHeapLocal, "
        "p: super::super::AllFreePage<super::super::Active>) { let mut escaped = None; "
        "unsafe { PageQueue::normal(q).with_all_free(h, p, |p| { escaped = Some(p.remove()); }); } "
        "let _ = escaped; }",
        "E0521",
    ),
    (
        "detached-collection-retains-exclusive-owner-loan",
        "src/types/page_batch.rs",
        "fn probe(mut owner: PageCollection<'_>) { "
        "let a = owner.detach_remote(); let _b = owner.detach_remote(); let _ = a.collect(); }",
        "E0499",
    ),
    (
        "detached-collection-rejects-independent-destination",
        "src/types/page_batch.rs",
        "fn probe(chain: DetachedForPage<'_, '_>, other: PageCollection<'_>) { "
        "let _ = chain.collect(other); }",
        "E0061",
    ),
    (
        "detached-collection-consumes-chain",
        "src/types/page_batch.rs",
        "fn probe(chain: DetachedForPage<'_, '_>) { "
        "let _ = chain.collect(); let _ = chain.collect(); }",
        "E0382",
    ),
    (
        "local-batch-borrows-destination-until-publication",
        "src/types/page_batch.rs",
        "fn probe(mut owner: PageCollection<'_>) { "
        "let batch = owner.detach_local_free_batch(); "
        "let _ = owner.recycle_local(false); drop(batch); }",
        "E0499",
    ),
    (
        "local-batch-rejects-independent-available-list",
        "src/types/page_batch.rs",
        "fn probe(batch: LocalFreeBatch<'_, '_>, other: AvailableFreeList) { "
        "batch.prepend_to_available(other); }",
        "E0061",
    ),
    (
        "remote-publication-handoff-is-safe",
        "src/types/validation.rs",
        "fn probe<'s>(p: FreePage<'s, ForeignPage>, b: ScopedFreeBlock<'s>) -> RemotePublication { "
        "p.publish_remote(b) }",
        None,
    ),
    (
        "remote-publication-consumes-storage",
        "src/types/validation.rs",
        "fn probe<'s>(p: FreePage<'s, ForeignPage>, b: ScopedFreeBlock<'s>) { "
        "let _ = p.publish_remote(b); let _ = b.block(); }",
        "E0382",
    ),
    (
        "remote-publication-consumes-page-access",
        "src/types/validation.rs",
        "fn probe<'s>(p: FreePage<'s, ForeignPage>, b: ScopedFreeBlock<'s>) { "
        "let _ = p.publish_remote(b); let _ = p.owner(); }",
        "E0382",
    ),
    (
        "remote-publication-rejects-cross-request-pairing",
        "src/types/validation.rs",
        "unsafe fn probe(a: ValidatedFree, b: ValidatedFree) { unsafe { "
        "a.with_foreign(|p, _| b.with_foreign(|_, b| { let _ = p.publish_remote(b); })); } }",
        "E0521",
    ),
    (
        "remote-collection-rejects-independent-page-and-head",
        "src/allocator/free.rs",
        "unsafe fn probe(p: crate::core_types::OwnedAbandonedPage, "
        "h: crate::core_types::PublishedRemoteHead) { "
        "let _ = unsafe { collect_after_remote_free(p, h) }; }",
        "E0061",
    ),
    (
        "remote-claim-pair-cannot-be-forged",
        "src/allocator/free.rs",
        "fn probe(p: crate::core_types::OwnedAbandonedPage, "
        "h: crate::core_types::PublishedRemoteHead) { "
        "let _ = crate::types::validation::ClaimedRemoteFree { page: p, head: h }; }",
        "E0451",
    ),
    (
        "queue-membership-authorizes-safe-removal",
        "src/types/page_queues.rs",
        "fn remove(p: QueuedPage<'_, NormalQueue>) -> PagePtr { p.remove().into_raw() } "
        "unsafe fn probe(q: NormalPageQueue, h: super::super::ThreadHeapLocal, "
        "p: super::super::ActivePage) { unsafe { PageQueue::normal(q).with_member(h, p, remove); } }",
        None,
    ),
    (
        "queue-membership-cannot-escape",
        "src/types/page_queues.rs",
        "unsafe fn probe(q: NormalPageQueue, h: super::super::ThreadHeapLocal, "
        "p: super::super::ActivePage) { let mut escaped = None; "
        "unsafe { PageQueue::normal(q).with_member(h, p, |p| { escaped = Some(p); }); } "
        "let _ = escaped; }",
        "E0521",
    ),
    (
        "detached-page-cannot-escape",
        "src/types/page_queues.rs",
        "unsafe fn probe(q: NormalPageQueue, h: super::super::ThreadHeapLocal, "
        "p: super::super::ActivePage) { let mut escaped = None; "
        "unsafe { PageQueue::normal(q).with_member(h, p, |p| { escaped = Some(p.remove()); }); } "
        "let _ = escaped; }",
        "E0521",
    ),
    (
        "queue-membership-consumed-on-detach",
        "src/types/page_queues.rs",
        "fn probe(p: QueuedPage<'_, NormalQueue>) { "
        "let _ = p.remove().into_raw(); let _ = p.remove().into_raw(); }",
        "E0382",
    ),
    (
        "queue-transition-rejects-independent-destination",
        "src/types/page_queues.rs",
        "fn probe(p: QueuedPage<'_, NormalQueue>, to: FullPageQueue) { "
        "let _ = p.into_full(to); }",
        "E0061",
    ),
    (
        "queue-cursor-cannot-escape",
        "src/types/page_queues.rs",
        "unsafe fn probe(q: NormalPageQueue, h: super::super::ThreadHeapLocal) { "
        "let mut escaped = None; unsafe { PageQueue::normal(q).with_cursor(h, |c| "
        "{ escaped = Some(c); }); } let _ = escaped; }",
        "E0521",
    ),
    (
        "borrowed-block-bytes-prevent-raw-escape",
        "src/types/local_page.rs",
        "fn probe<'s>(mut block: AllocatedBlockStorage<'s>, page: &PageAccess<'s, ActivePage>) { "
        "let bytes = &mut block.bytes[..]; let _ = block.finish(page, Zeroed::Yes); "
        "bytes.fill(MaybeUninit::new(0)); }",
        "E0505",
    ),
    (
        "allocation-loan-handoff",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { unsafe { "
        "p.with_allocation_access(r, core::ptr::null_mut(), |p, r| { let (b, p) = p.pop(r); "
        "let _ = b.track_undefined(&p).finish(&p, Zeroed::Yes); }); } }",
        None,
    ),
    (
        "allocation-rejects-cross-page-request",
        "src/types/local_page.rs",
        "unsafe fn probe(a: AvailablePage, b: AvailablePage, r: PaddedSize) { unsafe { "
        "a.with_allocation_access(r, core::ptr::null_mut(), |a, _| b.with_allocation_access(r, core::ptr::null_mut(), |_, r| "
        "{ let _ = a.pop(r); })); } }",
        "E0521",
    ),
    (
        "allocation-storage-cannot-escape",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { let mut escaped = None; unsafe { "
        "p.with_allocation_access(r, core::ptr::null_mut(), |p, r| { escaped = Some(p.pop(r).0); }); } "
        "let _ = escaped; }",
        "E0521",
    ),
    (
        "allocation-storage-consumed-on-finish",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { unsafe { "
        "p.with_allocation_access(r, core::ptr::null_mut(), |p, r| { let (b, p) = p.pop(r); "
        "let b = b.track_undefined(&p); let _ = b.finish(&p, Zeroed::Yes); "
        "let _ = b.finish(&p, Zeroed::Yes); }); } }",
        "E0382",
    ),
    (
        "allocation-storage-requires-tracking-transition",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { unsafe { "
        "p.with_allocation_access(r, core::ptr::null_mut(), |p, r| { let (b, p) = p.pop(r); "
        "let _ = b.finish(&p, Zeroed::Yes); }); } }",
        "E0599",
    ),
    (
        "local-field-loan",
        "src/types/local.rs",
        "unsafe fn probe(h: ThreadHeapLocal) { unsafe { h.with_local_access(|a| { "
        "a.stats().malloc_normal.current += 1; }); } }",
        None,
    ),
    (
        "local-field-loans-cannot-overlap",
        "src/types/local.rs",
        "unsafe fn probe(h: ThreadHeapLocal) { unsafe { h.with_local_access(|a| { "
        "let stats = a.stats(); let _random = a.random(); "
        "stats.malloc_normal.current += 1; }); } }",
        "E0499",
    ),
    (
        "local-field-loan-cannot-escape",
        "src/types/local.rs",
        "unsafe fn probe(h: ThreadHeapLocal) { let mut escaped = None; "
        "unsafe { h.with_local_access(|a| { escaped = Some(a.stats()); }); } "
        "let _ = escaped; }",
        "E0521",
    ),
    (
        "scoped-free-handoff",
        "src/allocator/free.rs",
        "unsafe fn probe(v: crate::core_types::ValidatedFree) { unsafe { "
        "v.with_local(|page, block| mi_free_block_local(page, block, "
        "crate::core_types::LocalFreeMode::TrackOnly)); } }",
        None,
    ),
    (
        "scoped-free-rejects-independent-page-and-block",
        "src/allocator/free.rs",
        "unsafe fn probe(p: crate::core_types::LocalPage, "
        "b: crate::core_types::BlockForFree) { unsafe { "
        "mi_free_block_local(p, b, crate::core_types::LocalFreeMode::TrackOnly); } }",
        "E0308",
    ),
    (
        "scoped-free-rejects-cross-request-pairing",
        "src/allocator/free.rs",
        "unsafe fn probe(a: crate::core_types::ValidatedFree, "
        "b: crate::core_types::ValidatedFree) { unsafe { "
        "a.with_local(|page, _| b.with_local(|_, block| "
        "mi_free_block_local(page, block, crate::core_types::LocalFreeMode::TrackOnly))); } }",
        "E0521",
    ),
    (
        "compact-chunk-access-requires-unsafe",
        "src/internal/bitmap.rs",
        "fn probe(p: *const mi_bitmap_t) { let _ = mi_bitmap_chunk(p, 0); }",
        "E0133",
    ),
    (
        "metadata-request-from-bounded-constructor",
        "src/allocator/arena_meta.rs",
        "fn probe(size: usize) { let _ = MetaRequest::new(size); }",
        None,
    ),
    (
        "metadata-request-fields-confined-to-owner",
        "src/allocator/mod.rs",
        "fn probe() { let _ = arena_meta::MetaRequest { size: 0, "
        "block_count: core::num::NonZeroU32::MIN }; }",
        "E0603",
    ),
    (
        "validated-pointer-handoff",
        "src/types/validation.rs",
        "unsafe fn probe(u: UserPtr, p: PagePtr, l: PointerLocation) { "
        "let _ = unsafe { PointerLookup::after_validation(u, p, l) }; }",
        None,
    ),
    (
        "validated-pointer-requires-unsafe",
        "src/types/validation.rs",
        "fn probe(u: UserPtr, p: PagePtr, l: PointerLocation) { "
        "let _ = PointerLookup::after_validation(u, p, l); }",
        "E0133",
    ),
    (
        "old-safe-pointer-constructor-absent",
        "src/types/validation.rs",
        "fn probe(u: UserPtr, p: PagePtr, l: PointerLocation) { "
        "let _ = PointerLookup::valid(u, p, l); }",
        "E0599",
    ),
    (
        "huge-alignment-from-checked-alignment",
        "src/types/layout.rs",
        "fn probe(a: Alignment) { let _ = MaybeHugeAlignment::for_overalloc(a); }",
        None,
    ),
    (
        "huge-alignment-rejects-raw-integer",
        "src/types/layout.rs",
        "fn probe(a: usize) { let _ = MaybeHugeAlignment::for_overalloc(a); }",
        "E0308",
    ),
    (
        "old-raw-alignment-constructor-absent",
        "src/types/layout.rs",
        "fn probe() { let _ = MaybeHugeAlignment::new(1); }",
        "E0599",
    ),
    (
        "flags-require-restricted-alignment",
        "src/types/layout.rs",
        "fn probe(a: MaybeAlignment) { let _ = MallocGenericFlags::new(Zeroed::No, a); }",
        "E0308",
    ),
]


def main():
    with tempfile.TemporaryDirectory(prefix="mimalloc-proof-checks-") as directory:
        checkout = Path(directory)
        shutil.copytree(ROOT / "src", checkout / "src")
        for name in ["Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml"]:
            shutil.copy2(ROOT / name, checkout / name)
        env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target/proof-constructors"))
        for name, relative, code, expected in PROBES:
            path = checkout / relative
            original = path.read_text()
            first_line = original.count("\n") + 1
            try:
                path.write_text(original + "\n#[allow(dead_code)] mod proof_probe {\nuse super::*;\n" + code + "\n}\n")
                run = subprocess.run(
                    ["cargo", "check", "--locked", "--lib", "--features", "std", "--message-format=json"],
                    cwd=checkout, env=env, capture_output=True, text=True,
                )
                errors = [json.loads(line)["message"] for line in run.stdout.splitlines()
                          if line.startswith('{"reason":"compiler-message"')]
                matched = [error for error in errors if error.get("code")
                           and error["code"]["code"] == expected
                           and any(span["is_primary"] and span["file_name"] == relative
                                   and span["line_start"] >= first_line for span in error["spans"])]
                if (expected is None and run.returncode) or (expected and (not run.returncode or not matched)):
                    raise RuntimeError(f"{name}: unexpected compiler result\n{run.stdout}\n{run.stderr}")
                print(f"{name}: passed", flush=True)
            finally:
                path.write_text(original)


if __name__ == "__main__":
    main()
