# TODO: Broad Safe Rust API Over mimalloc

Scope: implement the safe product/API surface that is still missing from
`mechanical_mimalloc`. This file is the implementation plan, not a status
report. Each item must be completed with code, docs, tests, and the validation
listed here before it can be marked done.

The missing product/API surface is:

- Safe `Heap` API.
- Safe heap allocation and lifetime wrappers.
- Safe `StatsSnapshot`.
- Safe `Options`.
- Safe `ProcessInfo`.
- Safe callback registration and RAII wrappers.
- Safe `Allocation` resize API.
- Safe initialized byte-slice APIs for `Allocation`.
- Full broad safe Rust abstraction over mimalloc heap/options/stats.

## Status Legend

- `[ ]` Not started or not complete.
- `[~]` Partially implemented; not ready to treat as product surface.
- `[x]` Done, tested, documented, and validated by the listed checks.

Do not mark an item complete until its validation block passes. Commit each
coherent completed item or small group of related items.

## API Design Guardrails

- Safe APIs must not expose allocator metadata pointers such as `*mut mi_heap_t`
  unless the function is explicitly unsafe and has a `# Safety` section.
- Safe APIs must convert null allocation results into typed errors.
- Safe APIs must preserve ownership on failure. For example, failed resize must
  return the original allocation to the caller.
- Safe APIs must not let safe Rust read uninitialized bytes.
- Safe APIs must not let a heap be deleted while allocations tied to that heap
  are still live.
- Safe callbacks must not allow Rust panics to unwind across the C ABI.
- Raw C ABI compatibility remains under `raw`; do not remove or rename raw C
  symbols while adding the safe layer.
- If a change only adds safe wrappers, docs, or tests, do not run full allocator
  benchmarks unless it touches allocator hot-path implementation files. If a
  change touches allocator core paths, run the relevant Criterion benchmarks and
  compare against `BENCHMARK_RESULTS.md`.

## Local Validation Set

Run the full set before treating the whole plan as complete:

```sh
cargo fmt --check
cargo test --quiet
cargo test --doc --features std
cargo check --examples --features std
scripts/check-api-inventory.py
scripts/check-safe-boundary.py
scripts/check-ffi-boundary.py
scripts/check-raw-facade.sh
scripts/check-docs.sh
scripts/check-lints.sh
scripts/check-features.sh
scripts/check-layout.sh
scripts/check-abi-symbols.sh
```

For hot-path allocator changes only, also run the relevant benchmark subset
first with a low sample count, increase samples if noisy, and compare against
`BENCHMARK_RESULTS.md`.

## Target Public Safe Surface

The crate root should eventually export this safe surface, in addition to the
existing `MiMalloc` and `raw` module:

```rust
pub use safe::{
    AllocError,
    Allocation,
    CallbackError,
    DeferredFreeCallbackGuard,
    ErrorCallbackGuard,
    Heap,
    HeapAllocation,
    HeapError,
    HeapRef,
    InitializedAllocation,
    InitializedHeapAllocation,
    LayoutError,
    MainHeap,
    OptionError,
    OptionKey,
    Options,
    ProcessInfo,
    ResizeError,
    ResizeErrorKind,
    StatCount,
    StatTotal,
    StatsError,
    StatsSnapshot,
    TryAllocError,
};
```

Implementation may split code into `src/safe/allocation.rs`,
`src/safe/heap.rs`, `src/safe/stats.rs`, `src/safe/options.rs`,
`src/safe/process.rs`, and `src/safe/callbacks.rs` once `src/safe.rs` becomes
too large. Keep the crate-root exports stable and documented.

---

## Phase 1: Foundation And Error Model

### 1.1 Define shared safe API errors

- [ ] Add error types that preserve ownership and separate layout failure from
  allocator failure.

Required signatures:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeErrorKind {
    Layout(LayoutError),
    Alloc(AllocError),
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResizeError<A> {
    allocation: A,
    kind: ResizeErrorKind,
}

impl<A> ResizeError<A> {
    pub fn kind(&self) -> ResizeErrorKind;
    pub fn allocation(&self) -> &A;
    pub fn allocation_mut(&mut self) -> &mut A;
    pub fn into_allocation(self) -> A;
    pub fn into_parts(self) -> (A, ResizeErrorKind);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapError {
    Create,
    MainHeapUnavailable,
    Unsupported(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsError {
    Unavailable,
    HeapUnavailable,
    BufferTooSmall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionError {
    InvalidKey,
    Unsupported(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackError {
    AlreadyRegistered,
    Poisoned,
    Unsupported(&'static str),
}
```

Implementation notes:

- `ResizeError<A>` must own the original allocation on failure.
- `AllocError` already stores `Layout`; reuse it instead of adding a second
  allocation-failure type.
- Add crate-private constructors before splitting `safe.rs`:

  ```rust
  impl AllocError {
      pub(crate) const fn new(layout: Layout) -> Self;
  }

  impl LayoutError {
      pub(crate) const fn new(size: usize, align: usize) -> Self;
  }
  ```

- Implement `Display` for every error.
- Implement `std::error::Error` behind `#[cfg(feature = "std")]`.

Pseudocode:

```rust
impl<A> ResizeError<A> {
    fn alloc(allocation: A, layout: Layout) -> Self {
        Self {
            allocation,
            kind: ResizeErrorKind::Alloc(AllocError { layout }),
        }
    }
}
```

Validation:

- Unit tests cover `Display`, `into_allocation`, and `into_parts`.
- `cargo test --quiet safe::` passes.
- `cargo test --doc --features std` passes.

### 1.2 Update API inventory before adding exports

- [ ] Update `docs/api-inventory.md`, `docs/safe-api.md`, and crate docs with
  the intended new safe types before making them public at crate root.

Validation:

- `scripts/check-api-inventory.py` passes after each export is added.
- Docs classify every new item as safe Rust SemVer surface.
- No safe docs claim `mi_heap_destroy` is wrapped safely.

---

## Phase 2: Allocation Initialization And Resize

### 2.1 Add initialized allocation wrapper

- [ ] Add `InitializedAllocation` as the only safe path to byte slices.

Required signatures:

```rust
pub struct InitializedAllocation {
    allocation: Allocation,
}

impl Allocation {
    pub fn zeroed_initialized(layout: Layout) -> Result<InitializedAllocation, AllocError>;
    pub fn zeroed_initialized_with_size_align(
        size: usize,
        align: usize,
    ) -> Result<InitializedAllocation, TryAllocError>;
    pub fn write_bytes(self, value: u8) -> InitializedAllocation;

    /// # Safety
    ///
    /// The caller must guarantee that every byte in this allocation is
    /// initialized before any safe byte-slice method is used.
    pub unsafe fn assume_initialized(self) -> InitializedAllocation;
}

impl InitializedAllocation {
    pub fn layout(&self) -> Layout;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn as_non_null(&self) -> NonNull<u8>;
    pub fn as_ptr(&self) -> *mut u8;
    pub fn as_bytes(&self) -> &[u8];
    pub fn as_bytes_mut(&mut self) -> &mut [u8];
    pub fn into_uninitialized(self) -> Allocation;
}
```

Implementation notes:

- Keep existing `Allocation::new` as uninitialized.
- Keep existing `Allocation::zeroed` behavior unless a deliberate breaking API
  change is approved. Add the new `zeroed_initialized` APIs first.
- `as_bytes` and `as_bytes_mut` must exist only on `InitializedAllocation`.
- Zero-sized allocations return empty slices using the aligned sentinel pointer.

Pseudocode:

```rust
impl Allocation {
    pub fn write_bytes(self, value: u8) -> InitializedAllocation {
        if self.layout.size() != 0 {
            unsafe { core::ptr::write_bytes(self.ptr.as_ptr(), value, self.layout.size()) };
        }
        InitializedAllocation { allocation: self }
    }
}

impl InitializedAllocation {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.allocation.ptr.as_ptr(), self.len()) }
    }
}
```

Validation:

- Compile-fail docs prove `Allocation::new(layout).unwrap().as_bytes()` does
  not compile.
- Unit tests prove `zeroed_initialized(...).as_bytes()` returns all zeroes.
- Miri-friendly tests, if available locally, should not read uninitialized
  memory.

### 2.2 Add safe resize for global allocations

- [ ] Add resize APIs for both uninitialized and initialized allocations.

Required signatures:

```rust
impl Allocation {
    pub fn resize(self, new_layout: Layout) -> Result<Self, ResizeError<Self>>;
    pub fn resize_with_size_align(
        self,
        size: usize,
        align: usize,
    ) -> Result<Self, ResizeError<Self>>;

    pub fn resize_zeroed(
        self,
        new_layout: Layout,
    ) -> Result<InitializedAllocation, ResizeError<Self>>;
}

impl InitializedAllocation {
    pub fn resize(self, new_layout: Layout) -> Result<Self, ResizeError<Self>>;
    pub fn resize_with_size_align(
        self,
        size: usize,
        align: usize,
    ) -> Result<Self, ResizeError<Self>>;

    pub fn resize_zeroed(self, new_layout: Layout) -> Result<Self, ResizeError<Self>>;
}
```

Required behavior:

- On success, preserve bytes `0..min(old_len, new_len)`.
- On `resize_zeroed`, bytes added by growth must be initialized to zero.
- On allocation failure, return `ResizeError` containing the original
  allocation.
- On invalid `size, align`, return `ResizeError` containing the original
  allocation and `ResizeErrorKind::Layout`.
- Zero-sized old or new layouts must not pass invalid pointers to raw realloc.

Pseudocode:

```rust
fn resize_impl<A>(
    allocation: A,
    old_ptr: NonNull<u8>,
    old_layout: Layout,
    new_layout: Layout,
    zero_new_bytes: bool,
) -> Result<A, ResizeError<A>> {
    if old_layout.size() == 0 {
        return allocate_new_and_drop_sentinel(allocation, new_layout, zero_new_bytes);
    }

    if new_layout.size() == 0 {
        free_old_pointer(old_ptr, old_layout);
        return Ok(rebuild_as_zero_sized(allocation, new_layout));
    }

    let new_ptr = raw_realloc_with_alignment(old_ptr, old_layout, new_layout);
    if new_ptr.is_null() {
        return Err(ResizeError::alloc(allocation, new_layout));
    }

    if zero_new_bytes && new_layout.size() > old_layout.size() {
        let start = new_ptr.add(old_layout.size());
        let len = new_layout.size() - old_layout.size();
        core::ptr::write_bytes(start, 0, len);
    }

    Ok(rebuild_with_pointer(allocation, new_ptr, new_layout))
}
```

Validation:

- Tests cover grow, shrink, same size, alignment change, zero-to-nonzero,
  nonzero-to-zero, and failure path ownership using an injected/test-only
  failing allocation path if practical.
- Tests prove initialized resize preserves initialized slice access.
- Tests prove uninitialized resize does not gain safe slice access.

---

## Phase 3: Safe Heap API And Heap Allocation Lifetimes

### 3.1 Add owned heap and heap reference types

- [ ] Add safe heap handle types that encode ownership and non-deletable main
  heap behavior.

Required signatures:

```rust
pub struct Heap {
    raw: NonNull<raw::mi_heap_t>,
    _not_send_sync: core::marker::PhantomData<*mut ()>,
}

pub struct MainHeap {
    raw: NonNull<raw::mi_heap_t>,
    _not_send_sync: core::marker::PhantomData<*mut ()>,
}

#[derive(Clone, Copy)]
pub struct HeapRef<'heap> {
    raw: NonNull<raw::mi_heap_t>,
    _lifetime: core::marker::PhantomData<&'heap ()>,
}

impl Heap {
    pub fn new() -> Result<Self, HeapError>;
    pub fn as_ref(&self) -> HeapRef<'_>;
    pub fn collect(&mut self, force: bool);
    pub fn set_numa_affinity(&mut self, numa_node: i32);

    /// # Safety
    ///
    /// `raw` must be a valid heap created by mimalloc, uniquely owned by the
    /// caller, and not be the main heap. After this call, `Heap` owns deletion.
    pub unsafe fn from_raw_owned(raw: NonNull<raw::mi_heap_t>) -> Self;

    /// # Safety
    ///
    /// The caller becomes responsible for eventually deleting the owned heap
    /// with the matching mimalloc raw API and for preventing use-after-delete.
    pub unsafe fn into_raw_owned(self) -> NonNull<raw::mi_heap_t>;
}

impl Drop for Heap {
    fn drop(&mut self);
}

impl MainHeap {
    pub fn get() -> Result<Self, HeapError>;
    pub fn as_ref(&self) -> HeapRef<'_>;
}
```

Implementation notes:

- `Heap::drop` must call `mi_heap_delete`, not `mi_heap_destroy`.
- Do not add a safe wrapper around `mi_heap_destroy`; it can invalidate live
  allocations and belongs in `raw`.
- Start with `Heap` and `MainHeap` as `!Send + !Sync` by using
  `PhantomData<*mut ()>`. Loosen only after a dedicated thread-safety audit.
- `MainHeap` must not implement `Drop` that deletes the heap.

Pseudocode:

```rust
impl Heap {
    pub fn new() -> Result<Self, HeapError> {
        let raw = unsafe { raw::mi_heap_new() };
        NonNull::new(raw)
            .map(|raw| Heap { raw, _not_send_sync: PhantomData })
            .ok_or(HeapError::Create)
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe { raw::mi_heap_delete(self.raw.as_ptr()) }
    }
}
```

Validation:

- Unit tests create and drop `Heap`.
- Compile-fail docs prove `Heap` is not `Send` or `Sync` initially.
- Tests prove `MainHeap::get()` does not delete the main heap on drop.

### 3.2 Add heap-tied allocation wrappers

- [ ] Add heap allocation wrappers whose lifetimes prevent heap deletion while
  allocations are live.

Required signatures:

```rust
pub struct HeapAllocation<'heap> {
    ptr: NonNull<u8>,
    layout: Layout,
    heap: NonNull<raw::mi_heap_t>,
    _heap: core::marker::PhantomData<&'heap ()>,
}

pub struct InitializedHeapAllocation<'heap> {
    allocation: HeapAllocation<'heap>,
}

impl<'heap> HeapRef<'heap> {
    pub fn allocate(self, layout: Layout) -> Result<HeapAllocation<'heap>, AllocError>;
    pub fn allocate_zeroed(
        self,
        layout: Layout,
    ) -> Result<InitializedHeapAllocation<'heap>, AllocError>;
    pub fn allocate_with_size_align(
        self,
        size: usize,
        align: usize,
    ) -> Result<HeapAllocation<'heap>, TryAllocError>;
    pub fn allocate_zeroed_with_size_align(
        self,
        size: usize,
        align: usize,
    ) -> Result<InitializedHeapAllocation<'heap>, TryAllocError>;
}

impl<'heap> HeapAllocation<'heap> {
    pub fn layout(&self) -> Layout;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn as_non_null(&self) -> NonNull<u8>;
    pub fn as_ptr(&self) -> *mut u8;
    pub fn write_bytes(self, value: u8) -> InitializedHeapAllocation<'heap>;

    /// # Safety
    ///
    /// The caller must guarantee every byte is initialized before safe
    /// byte-slice methods are used.
    pub unsafe fn assume_initialized(self) -> InitializedHeapAllocation<'heap>;
}

impl<'heap> InitializedHeapAllocation<'heap> {
    pub fn as_bytes(&self) -> &[u8];
    pub fn as_bytes_mut(&mut self) -> &mut [u8];
    pub fn into_uninitialized(self) -> HeapAllocation<'heap>;
}
```

Required behavior:

- `HeapAllocation<'heap>` must borrow the heap lifetime.
- Dropping `HeapAllocation<'heap>` must free the allocation exactly once.
- `HeapAllocation<'heap>` must not expose a safe way to outlive `Heap`.
- Zero-sized heap allocations use the same sentinel behavior as `Allocation`.

Pseudocode:

```rust
impl<'heap> HeapRef<'heap> {
    pub fn allocate(self, layout: Layout) -> Result<HeapAllocation<'heap>, AllocError> {
        if layout.size() == 0 {
            return Ok(HeapAllocation::dangling(layout, self.raw));
        }

        let ptr = if layout.align() <= core::mem::align_of::<usize>() {
            unsafe { raw::mi_heap_malloc(self.raw.as_ptr(), layout.size()) }
        } else {
            unsafe { raw::mi_heap_malloc_aligned(self.raw.as_ptr(), layout.size(), layout.align()) }
        };

        NonNull::new(ptr.cast::<u8>())
            .map(|ptr| HeapAllocation { ptr, layout, heap: self.raw, _heap: PhantomData })
            .ok_or(AllocError { layout })
    }
}
```

Validation:

- Compile-fail test proves `let a = heap.as_ref().allocate(...); drop(heap); drop(a);`
  does not compile.
- Unit tests cover zero-sized, aligned, zeroed, and normal heap allocations.
- Source audit confirms no safe function returns `*mut mi_heap_t`.

### 3.3 Add heap allocation resize

- [ ] Add safe resize APIs for `HeapAllocation` and
  `InitializedHeapAllocation`.

Required signatures:

```rust
impl<'heap> HeapAllocation<'heap> {
    pub fn resize(
        self,
        new_layout: Layout,
    ) -> Result<Self, ResizeError<Self>>;

    pub fn resize_zeroed(
        self,
        new_layout: Layout,
    ) -> Result<InitializedHeapAllocation<'heap>, ResizeError<Self>>;
}

impl<'heap> InitializedHeapAllocation<'heap> {
    pub fn resize(
        self,
        new_layout: Layout,
    ) -> Result<Self, ResizeError<Self>>;

    pub fn resize_zeroed(
        self,
        new_layout: Layout,
    ) -> Result<Self, ResizeError<Self>>;
}
```

Required behavior:

- Use heap-specific realloc APIs so growth remains associated with the heap.
- Preserve old allocation ownership on failure.
- Preserve initialized-state guarantees exactly like global allocation resize.

Validation:

- Tests cover grow, shrink, alignment, zero-size transitions, and failure
  ownership where practical.
- Heap lifetime compile-fail tests still pass after resize.

---

## Phase 4: Safe StatsSnapshot

### 4.1 Add owned stats snapshot type

- [ ] Add a safe, owned stats snapshot that contains no live allocator
  references.

Required signatures:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatsSnapshot {
    raw: raw::mi_stats_t,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatCount {
    pub current: i64,
    pub peak: i64,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatTotal {
    pub total: i64,
}

impl StatsSnapshot {
    pub fn current() -> Result<Self, StatsError>;
    pub fn for_heap(heap: HeapRef<'_>) -> Result<Self, StatsError>;
    pub fn reset_process();
    pub fn merge_thread();
    pub fn bin_size(bin: usize) -> Option<usize>;

    pub fn pages(&self) -> StatCount;
    pub fn reserved(&self) -> StatCount;
    pub fn committed(&self) -> StatCount;
    pub fn reset(&self) -> StatTotal;
    pub fn purged(&self) -> StatTotal;
    pub fn segments(&self) -> StatCount;
    pub fn heaps(&self) -> StatCount;
}
```

Implementation notes:

- `StatsSnapshot` owns a copy of `mi_stats_t`; it must not borrow allocator
  internals.
- Accessors should expose typed counters, not raw C layout fields.
- If a raw escape hatch is necessary, make it explicitly unsafe or keep it in
  `raw`; do not add safe `StatsSnapshot::raw()`.

Pseudocode:

```rust
impl StatsSnapshot {
    pub fn current() -> Result<Self, StatsError> {
        let mut stats = raw::mi_stats_t::default();
        let ok = unsafe { raw::mi_stats_get(&mut stats) };
        if ok {
            Ok(Self { raw: stats })
        } else {
            Err(StatsError::Unavailable)
        }
    }
}
```

Validation:

- Tests prove `StatsSnapshot::current()` succeeds after allocator init.
- Tests prove `StatsSnapshot::for_heap(heap.as_ref())` cannot outlive heap
  references.
- Docs explain that values are point-in-time snapshots and can change
  immediately after capture.

### 4.2 Add optional safe stats formatting

- [ ] Add safe formatting helpers only if they do not require exposing raw
  output callbacks.

Required signatures:

```rust
#[cfg(feature = "std")]
impl StatsSnapshot {
    pub fn to_json_string(&self) -> Result<String, StatsError>;
}
```

Implementation notes:

- Prefer using `mi_stats_as_json` on the owned snapshot.
- Grow the buffer and retry on `StatsError::BufferTooSmall`.
- Do not install global output callbacks for JSON formatting.

Validation:

- Unit tests parse the returned JSON enough to confirm it is non-empty and
  object-shaped.
- `cargo test --features std --quiet stats` passes.

---

## Phase 5: Safe Options

### 5.1 Add typed option keys

- [ ] Add `OptionKey` and a mapping to raw `mi_option_t`.

Required signatures:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OptionKey {
    ShowErrors,
    ShowStats,
    Verbose,
    ArenaEagerCommit,
    PurgeDecommits,
    AllowLargeOsPages,
    ReserveHugeOsPages,
    ReserveHugeOsPagesAt,
    ReserveOsMemory,
    PurgeDelay,
    UseNumaNodes,
    DisallowOsAlloc,
    OsTag,
    MaxErrors,
    MaxWarnings,
    DestroyOnExit,
    ArenaReserve,
    ArenaPurgeMult,
    DisallowArenaAlloc,
    RetryOnOom,
    VisitAbandoned,
    GuardedMin,
    GuardedMax,
    GuardedPrecise,
    GuardedSampleRate,
    GuardedSampleSeed,
    GenericCollect,
    PageReclaimOnFree,
    PageFullRetain,
    PageMaxCandidates,
    MaxVaBits,
    PagemapCommit,
    PageCommitOnDemand,
    PageMaxReclaim,
    PageCrossThreadMaxReclaim,
    AllowThp,
    MinimalPurgeSize,
    ArenaMaxObjectSize,
    ArenaIsNumaLocal,
}

impl OptionKey {
    fn raw(self) -> raw::mi_option_t;
    pub fn name(self) -> &'static str;
}
```

Implementation notes:

- Do not expose deprecated raw option aliases as first-class safe variants
  unless a compatibility requirement appears. Map only canonical current
  options.
- `OptionKey::raw` should stay private so safe users cannot manufacture invalid
  option keys through this API.

Validation:

- Unit test maps every `OptionKey` to the expected `mi_option_t` value.
- Unit test confirms no safe key maps to `_mi_option_last`.

### 5.2 Add process-global Options API

- [ ] Add a safe `Options` facade for process-global mimalloc options.

Required signatures:

```rust
pub struct Options;

impl Options {
    pub fn get(key: OptionKey) -> i64;
    pub fn get_clamped(key: OptionKey, min: i64, max: i64) -> i64;
    pub fn get_size(key: OptionKey) -> usize;
    pub fn set(key: OptionKey, value: i64) -> Result<(), OptionError>;
    pub fn set_default(key: OptionKey, value: i64) -> Result<(), OptionError>;
    pub fn is_enabled(key: OptionKey) -> bool;
    pub fn set_enabled(key: OptionKey, enabled: bool) -> Result<(), OptionError>;
    pub fn set_enabled_default(key: OptionKey, enabled: bool) -> Result<(), OptionError>;
}
```

Required behavior:

- Document that options are process-global and may affect other threads.
- Document initialization-order behavior: `set_default` affects defaults only
  until a key is initialized or explicitly set.
- Do not add `OptionOverride` RAII yet. Restoring process-global options in
  `Drop` can clobber another thread's update and needs a separate design.

Pseudocode:

```rust
impl Options {
    pub fn get(key: OptionKey) -> i64 {
        unsafe { raw::mi_option_get(key.raw()) as i64 }
    }

    pub fn set(key: OptionKey, value: i64) -> Result<(), OptionError> {
        unsafe { raw::mi_option_set(key.raw(), value as core::ffi::c_long) };
        Ok(())
    }
}
```

Validation:

- Tests save and restore only low-risk diagnostic options in a serial test.
- Docs warn that option mutation is global and should not be done in
  concurrently running tests without serialization.
- `scripts/check-safe-boundary.py` confirms no raw `mi_option_t` appears in
  safe signatures.

---

## Phase 6: Safe ProcessInfo

### 6.1 Add owned process info snapshot

- [ ] Add `ProcessInfo` as an owned value returned from `mi_process_info`.

Required signatures:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessInfo {
    elapsed_millis: usize,
    user_millis: usize,
    system_millis: usize,
    current_rss: usize,
    peak_rss: usize,
    current_commit: usize,
    peak_commit: usize,
    page_faults: usize,
}

impl ProcessInfo {
    pub fn current() -> Self;
    pub fn elapsed_millis(self) -> usize;
    pub fn user_millis(self) -> usize;
    pub fn system_millis(self) -> usize;
    pub fn current_rss(self) -> usize;
    pub fn peak_rss(self) -> usize;
    pub fn current_commit(self) -> usize;
    pub fn peak_commit(self) -> usize;
    pub fn page_faults(self) -> usize;
}
```

Implementation notes:

- The raw C API returns through out-pointers and does not report failure. The
  safe API can return `Self` directly.
- Keep units in method names. Do not expose ambiguous fields like `elapsed`.
- Values are snapshots and may be zero on platforms where a field is not
  available.

Pseudocode:

```rust
impl ProcessInfo {
    pub fn current() -> Self {
        let mut info = Self::default();
        unsafe {
            raw::mi_process_info(
                &mut info.elapsed_millis,
                &mut info.user_millis,
                &mut info.system_millis,
                &mut info.current_rss,
                &mut info.peak_rss,
                &mut info.current_commit,
                &mut info.peak_commit,
                &mut info.page_faults,
            );
        }
        info
    }
}
```

Validation:

- Tests call `ProcessInfo::current()` before and after a small allocation.
- Docs explain platform variability and units.
- `scripts/check-safe-boundary.py` confirms no out-pointers appear in the safe
  signature.

---

## Phase 7: Safe Callback Registration And RAII Guards

### 7.1 Add safe output callback guard

- [ ] Add a safe RAII wrapper for `mi_register_output`.

Required signatures:

```rust
#[cfg(feature = "std")]
pub trait OutputCallback: Sync + std::panic::RefUnwindSafe {
    fn write(&self, message: &core::ffi::CStr);
}

#[cfg(feature = "std")]
pub struct OutputCallbackGuard {
    _private: (),
}

#[cfg(feature = "std")]
impl OutputCallbackGuard {
    pub fn register(callback: &'static dyn OutputCallback) -> Result<Self, CallbackError>;
}
```

Required behavior:

- Only one safe output callback guard may be active at a time.
- The callback reference must be `'static`; this avoids dangling callback
  context if another thread is still inside the callback during guard drop.
- The trampoline must catch unwinding panics and mark the callback system
  poisoned. It must not unwind across the C ABI.
- `Drop` must unregister the safe callback by calling `mi_register_output(None,
  null_mut())`.
- Document that raw code can overwrite the global callback and the safe guard
  cannot restore raw callbacks it did not install.

Pseudocode:

```rust
unsafe extern "C" fn output_trampoline(msg: *const c_char, arg: *mut c_void) {
    if msg.is_null() {
        return;
    }

    let callback = unsafe { &*(arg as *const &'static dyn OutputCallback) };
    let result = std::panic::catch_unwind(|| {
        callback.write(unsafe { core::ffi::CStr::from_ptr(msg) });
    });

    if result.is_err() {
        CALLBACK_POISONED.store(true, Ordering::Release);
    }
}
```

Validation:

- Tests register a static callback, emit output through a raw print API, and
  observe the callback.
- Tests prove a second registration returns `CallbackError::AlreadyRegistered`.
- Panic test proves no unwind crosses FFI and later registration reports
  `CallbackError::Poisoned` or documented recovery behavior.

### 7.2 Add safe error callback guard

- [ ] Add a safe RAII wrapper for `mi_register_error`.

Required signatures:

```rust
#[cfg(feature = "std")]
pub trait ErrorCallback: Sync + std::panic::RefUnwindSafe {
    fn on_error(&self, error: i32);
}

#[cfg(feature = "std")]
pub struct ErrorCallbackGuard {
    _private: (),
}

#[cfg(feature = "std")]
impl ErrorCallbackGuard {
    pub fn register(callback: &'static dyn ErrorCallback) -> Result<Self, CallbackError>;
}
```

Required behavior:

- Same single-registration, `'static`, panic-catching, and unregister-on-drop
  behavior as `OutputCallbackGuard`.
- Do not allocate inside the trampoline.
- Document that allocator errors can be emitted from allocator paths and the
  callback must tolerate reentry.

Validation:

- Tests cover registration exclusivity and drop unregister behavior.
- If triggering an allocator error is not deterministic, use a test-only raw
  call path or isolate behavior to registration state tests.

### 7.3 Add deferred-free callback guard only after reentry audit

- [ ] Add a safe RAII wrapper for `mi_register_deferred_free` after documenting
  why the callback body cannot violate allocator reentry expectations.

Required signatures:

```rust
#[cfg(feature = "std")]
pub trait DeferredFreeCallback: Sync + std::panic::RefUnwindSafe {
    fn on_deferred_free(&self, force: bool, heartbeat: u64);
}

#[cfg(feature = "std")]
pub struct DeferredFreeCallbackGuard {
    _private: (),
}

#[cfg(feature = "std")]
impl DeferredFreeCallbackGuard {
    pub fn register(callback: &'static dyn DeferredFreeCallback) -> Result<Self, CallbackError>;
}
```

Required behavior:

- Same guard semantics as output and error callbacks.
- Trampoline must not allocate.
- Docs must explicitly warn that this callback can run during allocator
  activity and should be minimal.

Validation:

- Registration/drop tests pass.
- Reentry notes are added to `docs/abi.md` and `docs/safe-api.md`.
- No benchmark is required unless the implementation changes allocator
  callback hot paths.

---

## Phase 8: Broad Safe Abstraction Integration

### 8.1 Organize the safe module

- [ ] Split `src/safe.rs` only when needed and keep exports explicit.

Required shape if split:

```rust
// src/safe/mod.rs
mod allocation;
mod callbacks;
mod heap;
mod options;
mod process;
mod stats;

pub use allocation::{
    AllocError, Allocation, InitializedAllocation, LayoutError, ResizeError,
    ResizeErrorKind, TryAllocError,
};
pub use callbacks::{
    CallbackError, DeferredFreeCallbackGuard, ErrorCallbackGuard, OutputCallbackGuard,
};
pub use heap::{
    Heap, HeapAllocation, HeapError, HeapRef,
    InitializedHeapAllocation, MainHeap,
};
pub use options::{OptionError, OptionKey, Options};
pub use process::ProcessInfo;
pub use stats::{StatCount, StatTotal, StatsError, StatsSnapshot};
```

Validation:

- `scripts/check-api-inventory.py` is updated with every new crate-root export.
- `cargo doc --no-deps --features std` shows the broad safe surface without
  implementation modules.

### 8.2 Add examples that demonstrate intended use

- [ ] Add examples for the broad safe API.

Required examples:

```text
examples/safe_allocation.rs
examples/safe_heap.rs
examples/safe_stats.rs
examples/safe_options.rs
examples/safe_process_info.rs
examples/safe_callbacks.rs
```

Each example must:

- Use only safe APIs unless it is demonstrating an explicitly unsafe escape
  hatch.
- Avoid depending on exact allocator stats values.
- Avoid changing process-global options without restoring them in the example
  body.

Validation:

- `cargo check --examples --features std` passes.

### 8.3 Add compile-fail documentation for safety boundaries

- [ ] Add rustdoc compile-fail examples for the main safe invariants.

Required compile-fail cases:

```rust
// Allocation::new does not expose safe byte reads.
let allocation = Allocation::new(layout).unwrap();
let _ = allocation.as_bytes();

// Heap allocation cannot outlive the heap.
let allocation = {
    let heap = Heap::new().unwrap();
    heap.as_ref().allocate(layout).unwrap()
};
drop(allocation);

// Heap cannot be sent to another thread until explicitly audited.
std::thread::spawn(move || drop(heap));
```

Validation:

- `cargo test --doc --features std` passes.

### 8.4 Final docs and compatibility update

- [ ] Update safe API docs so they no longer say these product surfaces are
  intentionally missing.

Required doc updates:

- `docs/safe-api.md` describes the completed heap, options, stats, process,
  callbacks, resize, and initialized-slice APIs.
- `docs/compatibility.md` classifies the new safe API as Rust SemVer surface.
- `CHANGELOG.md` records the broad safe API addition.
- `README.md` shows at least one safe heap allocation example and one stats or
  process-info example.

Validation:

- `scripts/check-docs.sh` passes.
- `scripts/check-api-inventory.py` passes.
- No docs claim `Heap::destroy` exists as a safe API.

### 8.5 Final performance signoff

- [ ] Confirm the broad safe API work did not regress allocator hot paths.

Required decision process:

- If the implementation only adds wrappers, docs, examples, and tests, record
  that no benchmark run was required because allocator core paths were not
  changed.
- If any allocator core file was changed, run a targeted Criterion subset first
  with 3 samples. If noisy, rerun with 5 samples. Use 10 samples only for final
  confirmation or disputed results.
- Compare hot-path and overall scores against `BENCHMARK_RESULTS.md`.

Validation note template:

```text
Benchmark decision:
- Core allocator files changed: yes/no
- Benchmarks run: <commands or "not required">
- Hot-path result versus BENCHMARK_RESULTS.md: <summary>
- Overall score versus BENCHMARK_RESULTS.md: <summary>
```

### 8.6 Final release-quality signoff

- [ ] Complete the broad safe API signoff.

Required commands:

```sh
cargo fmt --check
cargo test --quiet
cargo test --doc --features std
cargo check --examples --features std
scripts/check-api-inventory.py
scripts/check-safe-boundary.py
scripts/check-ffi-boundary.py
scripts/check-raw-facade.sh
scripts/check-docs.sh
scripts/check-lints.sh
scripts/check-features.sh
scripts/check-layout.sh
scripts/check-abi-symbols.sh
```

Final completion criteria:

- Every item in this TODO is marked `[x]`.
- All public safe APIs have rustdoc with examples or deliberate no-example
  notes.
- Every unsafe function has a `# Safety` section.
- No safe signature exposes raw allocator metadata pointers.
- Safe resize preserves ownership on failure.
- Safe initialized byte slices cannot read uninitialized memory.
- Heap lifetimes prevent heap deletion while heap allocations are live.
- Callback guards cannot unwind across C ABI.
- Benchmark decision is recorded.
