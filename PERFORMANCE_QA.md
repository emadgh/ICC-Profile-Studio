# Beta.45 performance QA

## Independent FOGRA55 exact-neighbor scan diagnostic

Source: operator-provided Ref_FOGRA55.zip / FOGRA55.txt; 4,884 rows, 4,272 unique 7-channel integer recipe keys; FNV1a fold 0, 3,405 training, 867 held-out. C implementation compiled with GCC -O3, contiguous uint32 arrays; compares full squared distance against early rejection once partial sum strictly exceeds current worst top-33 distance, same increasing-index tie rule. Instrumented component counter included in timing. Eight runs per algorithm, median of last seven. This is NOT the Rust binary benchmark.

| Metric | Full | Bounded |
|---|---:|---:|
| Channel-distance component evaluations | 20,664,945 | 10,228,229 |
| Median instrumented scan elapsed | 11.087867 ms | 11.089670 ms |
| Top-33 results | reference | identical, all 867 queries |

50.5044% fewer component evaluations; no meaningful measured wall-clock improvement in this isolated experiment. Branch overhead, compiler vectorization and data arrangement can offset arithmetic reductions. Existing model, weight and regression equations unchanged. Rust test compares complete ordered neighbor pairs on tied data, 2/4/7/15 axes; ignored release timing test `cargo test --release --no-default-features benchmark_neighbor_scan -- --ignored --nocapture` (synthetic).

## Allocation design accounting (not measured RSS)

- Removed full mesh copy from each visible profile render: 36 bytes/triangle logical payload, e.g. 100,000 triangles = 3.43 MiB avoided temporary copy per profile, plus point-list copy 12 bytes/point. Borrow original geometry instead.
- Fixed three-element stack arrays for each edge intersection; reuse scanline scratch, remove duplicate intermediate slice collection.
- Wire geometry cache capped at 65,536 segments/profile (24 bytes/segment logical payload; 1.5 MiB/profile, 3 MiB for two, excluding allocator overhead). Cache is a deliberate speed/memory tradeoff. Oversized grids stream without full segment buffer; first preparation may still allocate up to cap. Cache per preview and chart revision; prepared vs streaming pixel test. Raster still runs on UI thread; no FPS guarantee.
- Arc chart snapshots avoid full deep copy of patch data for each worker; explicit fit is copy-on-write and stages results before commit. Filtered paired cloud cached by revision. Source bounds/TAC computed once on load, not per control redraw; write path retains fresh validation.
- Contiguous BGRA conversion avoids per-pixel coordinate lookup; still allocates output buffer.

## Required real Windows benchmark

Use same build settings, hardware, ICC and chart inputs, resolution and display modes. Record cold/warm preview time, orbit/zoom median/P95 frame time, working set and peak private bytes, source-load time, characterization audit runtime, output hash / color metrics. Compare beta.44 and beta.45 with real 4/7/15-channel corpus; measure at 100% and high DPI, wire-only/solid/both, two profiles. Keep cache tradeoff explicit. No measured end-to-end Windows speed/RAM or manual UI sign-off yet. CI confirms tests/build only.

## CI #113 fix and follow-up memory reductions

CI #113 failed compilation with E0282 on wire callback destructuring; explicit [[f32;3];2] annotation added. Camera coefficients now computed once per render (same individual sin/cos operations), back-facing/occluded fragments rejected before RGB interpolation; equal depth still overwrites as before. Camera equivalence test added. Gamut sampling retains only one L* plane's valid XYZ, index, Lab and native output scratch at a time instead of the entire adaptive interior. At high quality n=57, upper bound 55^3=166375 interior samples vs 55^2=3025: 98.18% fewer maximum simultaneously held sample records. With 80 bytes logical payload per record on 64-bit (24 XYZ + 8 index + 24 Lab + 24 actual XYZ), theoretical upper-bound scratch drops from 12.69 MiB to 0.231 MiB, not including field, mesh, input device buffers, allocator or actual valid-sample count. Target/error order and coordinate math preserved; existing batch-equivalence and native gamut tests remain. Visible analysis progress polling avoids redraws when phase/completed/total/cancellation state (or indeterminate checkpoint count) is unchanged. Raster still runs synchronously on UI thread; no measured FPS or process RSS claim. Single combined CI rerun requested after these fixes.

## Beta.46 asynchronous preview architecture

CPU raster, wire geometry and BGRA packing moved from render callbacks to the background executor. Each of two independent view slots (main, characterization) owns at most one active request and a single desired key. Camera changes replace the desired key. No per-event frame queue or mesh copy: Arc snapshots share immutable data. The main gamut page and docked panel share a slot. Same-dataset intermediate frames are displayed with a pending label for continuous interaction; old dataset results are rejected and old image cleared on new request. A worker may run to completion even after its dataset is obsolete. A background job temporarily owns the existing bounded wire cache. GUI thread still creates RenderImage and presents it; cloud/filter assembly and GPUI layout still execute on the UI thread. Background PNG export may run alongside preview workers and is explicitly a write job, no-clobber.

Pure gate tests cover 1000 camera requests while one frame runs (one next request, no queue), final convergence, old dataset rejection, absent/reappearing data and duplicate completion. This is architecture/test coverage, not an end-to-end benchmark. Compare actual Windows UI latency/FPS/private bytes and cold/warm raster times against Beta.45 under same profiles and camera paths; report intermediate-frame behavior separately from final-quality latency. No GPU renderer introduced.

## Beta.47 raster hot paths
- Opaque alpha=255 uses direct RGB assignment; alpha=0 is a no-op, preserving exact integer blending. Wire samples outside the viewport or rejected by the same inclusive depth test skip RGB interpolation and solid-wire dimming. When both profile layers are absent, omit the 640×640 (409,600 pixel) final composition traversal; the original grid and fitting scale are retained. These are avoided operations, not measured wall-clock gains.
- Add previous-implementation parity tests for clipped/overlapping wires and depth ties in both modes, and all 256 alpha levels. Keep Beta.46 background frame gate and existing rendering tests. Windows CI and end-to-end performance benchmark still required.

## Beta.49 ICC serialization allocation accounting
- Disk save previously materialized an additional full ICC Vec before writing. It now borrows existing tag payloads and writes to the temporary file using only a directory buffer (132 + 12 bytes per tag; at 4,096 tags, 49,284 bytes), plus up to three padding bytes per tag. With a 64 MiB serialized profile, this removes approximately 64 MiB of temporary serialized payload allocation, not including existing parsed tags, directory, file-system caches or allocator overhead. This is design accounting, not measured process RSS.
- bytes() still returns a complete in-memory ICC for native consumers, but reserves the exact computed length in one allocation request. Previous encoder parity, write-error propagation and no-clobber publication tested. Profile::read still reads the original bytes before parsing; do not claim a loading memory improvement yet. Benchmark actual large profiles and Windows peak private bytes before performance sign-off.
