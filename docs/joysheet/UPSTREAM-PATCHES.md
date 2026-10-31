# Upstream adaptations

The import manifest retains hashes of the original GridCraft 0.12.1 files.

| File | Adaptation | Runtime behavior |
| --- | --- | --- |
| crates/engine/src/io/tests.rs | Gate the Unix symlink test with `cfg(unix)`; it previously referenced `std::os::unix` on Windows | No production changes |
| crates/engine/src/io.rs | Preserve a pre-existing `.sctmp` file when exclusive creation fails | Save still fails; unrelated temporary data is no longer deleted. Covered by Joysheet's cross-platform save-failure regression. |
| apps/joysheet-cli | Rename the CLI, use Joysheet control token lookup, and scope the Unix-only directory identity lint exemption | CLI branding/connection adaptation; engine unchanged |
| crates/engine/src/lib.rs, cmd/extra.rs, cmd/extra/scripts.rs | Optional host ScriptControl flags and read-only script definition getter | Default execution unchanged. Joysheet can report completed steps and cancel between commands. Completed commands remain undoable; no interruption inside a command. Trust and sandbox preflight remain enforced. Cancellation regression covers no mutation before the first step. |
| crates/mcp/src/tools.rs, tests.rs | Forward window/doc targets for desktop/app tools; expose screenshot target parameters | MCP screenshot and save target the requested native window. Fake backend regression and live stdio/native test cover forwarding. |

Unlisted production core implementation files remain imported unchanged; the later performance adaptations are enumerated below. Joysheet's tests
add native-platform save failure coverage independently of the Unix symlink test.

## 2026-10-30 geometry/calculation profiling

`crates/calc/src/profile.rs`, `lib.rs`, `recalc.rs`, `recalc/pass_host.rs` and `crates/engine/src/cmd.rs` add optional per-thread numeric phase timing and counts. Formula expressions, order, values and file formats remain unchanged. The application worker starts/finishes collection per request; native scopes use a monotonic clock, disabled scopes are no-ops and WASM never starts native timing. Range reads are nested within evaluation/recalculation. Tests cover disabled collection, thread isolation and command reset. Original hashes in upstream-source.json remain the unmodified import baseline.

`crates/model/src/store.rs` adds a lazy, per-64-row-band used-range cache. Only key insertion/removal invalidates that band's extent; value changes and COW snapshots retain valid extents. Cache fields are skipped by serde and excluded from equality. The original malformed-edge behavior before rebuild is retained; normal rebuild reconstructs derived data from cells. Sparse edit/clear/shift/COW/JSON differential tests compare against a fresh full-cell oracle, including untrusted serialized cache fields. Core APIs and serialized cell layout are unchanged.

`crates/model/src/store.rs` additionally indexes whether each band contains formulas. Set/remove maintain or invalidate that derived flag; opaque mutable access invalidates it. Untrusted serialized flags are ignored. `iter_formulas` yields exactly the original filtered row-major sequence. One-column range and row iteration use a single BTreeMap lookup per row. Model differential tests cover mutation, COW, single/wide ranges, order, and JSON/rebuild.

`crates/calc/src/recalc.rs`, `recalc/pass_host.rs` use the formula-band index for graph rebuild and the formula-first portion of streaming reads. The order of formula evaluation, spill fallback and value accumulation is retained. `eval.rs`, `eval/call.rs`, `recalc/pass_host.rs` add a maximum-64-entry SUM memo scoped to one immutable PassHost. It stores only the complete numeric result of a one-argument, one-area SUM on a constant area; excludes formula cells, iterative reads, old/new spills, multi-argument/multi-area calls and errors. Eligibility is rechecked on hits (including a spill born later in the pass). No partial sums or reordered accumulation are introduced. Tests cover floating-point order, mutation between passes, formula/error/spill/iteration fallback, newly created spill, and the memory bound. Diagnostic sumCacheHits permits checking that accelerated fanout still evaluates all ten formulas.

Cold-index refinement: opaque cell mutation now marks a band as possibly containing formulas, and a bounded formula-range walk scans only the requested columns for unknown/positive bands. It does not initialize a full-width index after formatting. A completed constant streaming read supplies the memo's purity proof; it is not obtained with a redundant second scan. The workbook is immutable for the pass; existing/new spill and iterative guards are still checked on every hit. Global graph rebuild enumerates all stored positions, including unsanitized positions, matching the original iterator before normalization.

## 2026-10-30 dense sheets / spill updates

`crates/model/src/lines.rs`, `lib.rs`, `sheet.rs` wrap row/column metadata in copy-on-write `Lines` with shared immutable prefix indexes. JSON retains the original map representation. Any mutable map borrow invalidates the index; snapshots detach on mutation; default/max dimensions participate in cache lookup. `engine/src/cmd/data/sort.rs` captures hidden state before insertion to satisfy wrapper borrowing; sort semantics are unchanged. This is a source-level type adaptation of the public Sheet rows/cols fields, with Deref map access and owned/borrowed iteration retained.

`model/src/store.rs` provides ordered value-only block writeback, preserving formula and extent indexes and COW snapshots. `calc/src/recalc.rs` uses it in row-major order after evaluation. Spill invalidation queries graph range indexes and includes formulas inside the range rather than enumerating all spill coordinates; differential tests compare dirty closure with coordinate seeding. `calc/src/recalc/pass_host.rs` only traverses the overlap of new arrays with requested ranges, preserving overlay order and iterative taint; ordinary shallow calculations avoid allocating a suspension work stack. Deep-chain/cycle/iteration regressions continue using the bounded-stack path. Optional profiling adds preparation and spill-seed timings; original import hashes remain untouched.
