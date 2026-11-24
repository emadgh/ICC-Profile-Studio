# ICC Profile Studio — development tracker

## v0.3 — migration and inspection

- [x] Existing four-channel K/channel-limiter engine, v2/v4 LUT handling and non-destructive output retained (from v0.2.2).
- [x] Shared rebuild service extracted from legacy egui frontend.
- [x] LittleCMS 2 (static Rust binding) added for profile opening, metadata, TAC estimate and CMYK transform smoke checks.
- [x] GPUI sidebar shell and first Inspector/Validator/Rebuilder screens implemented in source.
- [x] Windows CI at commit 9b264922: LittleCMS 6.3.0 resolved, 17 tests passed and GPUI EXE built/uploaded (run 45400551593).
- [ ] Launch on Windows and verify layout and real printer workflows.
- [ ] Port all legacy controls, updater, progress, drag-and-drop, accessibility and About icon to GPUI before promoting it to the default release.
- [ ] Run real Durst ICC v2/v4 regression tests and print/firing checks. Proprietary customer ICCs must stay out of the repository.
- [ ] Add independent LCMS transform checks for generated profiles; enforce numerical bounds through the strict LUT validator as well as sampled transforms.

## v0.4 — comparison (source implementation started)

- [x] Compare two CMYK output profiles using LittleCMS transforms, media-relative D50 PCS and native CIEDE2000.
- [x] Mean, P95, maximum Delta E, worst RGB, histogram, sampled per-channel means/maxima, TAC and 33 neutral-axis samples.
- [x] Profile Comparison page in experimental GPUI frontend; background analysis and selectable 9/17/33 RGB cube.
- [x] Reference CIEDE2000 test pair and ink-statistics tests added.
- [x] CI compiled comparison and passed its unit tests at 9b264922.
- [ ] Compare real printer profiles; add export, intent selector, complete neutral curves, measured validation and gamut visualization.

## v0.5 — profile editing (source implementation started)

- [x] Tone gamma editor: compose 4096-point output curves on B2A, retain the physical A2B model, support simple mft2/mBA.
- [x] Channel permutation: re-index A2B CLUT axes, input curves, B2A outputs and colorant table. Reject unknown or channel-sensitive tags; no silent metadata loss.
- [x] Metadata renamer: copy with new ICC description from filename, preserve transform tags.
- [x] GPUI screens and new-file-only save, LCMS opening and CMYK transform creation before writing.
- [x] Unit tests for forward/reverse permutation and v4 LUT encoding.
- [x] CI compiled editing and passed its unit tests at 9b264922.
- [ ] Real profile regression, full transform-equivalence testing, custom editable multi-point curves, strict TAC and prior cap revalidation after tone changes.
- [ ] Add explicit adapters for Durst/vendor tags only after their structure is verified; current reorder can refuse proprietary profiles.

## v0.6 — persistent gamut preview (source implementation started)

- [x] Collapsible right-side preview shared across GPUI sections; opening a reference resets stale comparison data.
- [x] Independent Before/After visibility and overlay, 2D a*/b* projection with optional L* ±5 slice, 3D orthographic Lab point cloud, yaw/pitch, zoom and reset.
- [x] LittleCMS effective separation sampling (Lab targets → B2A → device → A2B → Lab), shared reference/candidate grid and axes. Sampled points, not exact surface or gamut volume.
- [x] Auto-select newly saved ICC as candidate after rebuild/edit; refresh after file selection and saving when panel is open; manual refresh and 9/17/33 sampling.
- [x] Grid/projection unit tests and RGB rejection test added.
- [x] GPUI Windows CI and artifact verified at 9b264922; 17 tests passed.
- [x] Gradient Analyzer: 13 × 256 sRGB ramps, adjacent ΔE00, P95, plateau/spike heuristic, maximum ink step, paired graphs.
- [x] Gamut target coverage: paired D50 Lab errors at selectable ΔE00 thresholds, lost/gained/both/neither counts and lost share.
- [x] Parry3D convex hull wireframe and approximate convex envelope volume, separate from coverage. Point cloud fallback for degenerate shapes or L* slices.
- [x] Analytics/hull CI passed at 21bcef07: 21 unit tests and Windows GPUI artifact 14923084302.
- [ ] Actual printer files, mouse drag/wheel, axis tick labels, nonconvex mesh/volume and runtime checks.

## v0.7 — selective color and smoothing (source implementation)

- [x] Selective D50 Lab/LCh correction: circular hue mask, smooth falloff, lightness/chroma/hue controls and protection of near-neutrals and extreme lightness targets.
- [x] Use LittleCMS native tag Pipeline evaluation of each original B2A intent; bake separate 16-bit mBA CLUTs, keep A2B and source channel maxima. Optional explicit TAC; verify every node and output bounds. No reuse of a single rendering intent.
- [x] Bounded six-neighbor LUT smoothing, strength 5–50%, 1–5 passes. Convex averaging of interior nodes; no averaging of boundary nodes. Resampling and explicit TAC still affect boundaries.
- [x] GPUI pages, 17/33/49/65 grid, output auto-selected in shared before/after Preview, no source overwrite, LittleCMS transform smoke checks.
- [x] Five unit tests for hue wrapping, protected neutrals, locality and ink bounds.
- [ ] New CI and integration regression on a valid ICC, quantify drift outside selective band, real printer tests and compare before/after using Gradient Analyzer and Delta E00.
- [ ] ICC v2/other LUT structures, multiple selective masks, interactive mouse orbit and wheel zoom.

## Next milestones

- v0.4: Complete validation/export of Profile Comparison, Delta E2000, neutral axis and ink distribution.
- v0.5: Complete runtime validation of Tone Curves, channel mapping/reordering, profile metadata editing.
- v0.6: Validate new analytics and hull build; actual printer regression, richer controls and nonconvex gamut analysis.
- v0.7: Verify selective Lab/LCh correction and smoothing, add multi-region controls and mouse orbit.
- v0.8: ICC image conversion, batch conversion, separations, soft proof, ink coverage.
- v0.9: DeviceLink generation, constrained ink saving and cost optimization.
- v1.0: Ceramic profiling, reference charts, measured glaze/firing/ink batch compensation, machine matching, profile library.

## Dependency-first architecture

- **LittleCMS 2**: use mature library for ICC parsing, supported transforms (v2/v4), intent handling, proofing, color metrics and DeviceLink construction. Rust crate `lcms2` is statically linked to keep the Windows portable distribution simple; do not ship random DLLs.
- **ArgyllCMS**: keep official external tools for supported v2 revfix, measurement/profiling and diagnostics when required. Download official packages only, with trusted checksum if available. Do not route unsupported v4 through revfix.
- **Custom Rust**: retain only constrained separation optimization, strict per-channel bounds and missing functionality. Replace homemade forward evaluation and serialization with LCMS-backed calls where proven equivalent; never merely relabel v4 or clip existing output as if reprofiled.
- **GPUI**: requested “pgui” interpreted as GPUI from Zed. New experimental frontend shares backend with egui; legacy release stays default until migration is verified. Prefer existing component crates where compatible.
- **rfd**: native open/save dialogs. **update-via-github**: existing updater, private-repository limitation remains.
- Physical print correction requires measured data. No claim that simulated color accuracy is verified by print tests.

## Status rules

Check boxes mean source implementation, not verified runtime unless explicitly marked. Update this tracker in every feature commit. CI must pass before marking a build verified. Never overwrite the input profile.

## Build evidence

- Successful baseline: https://github.com/emadgh/color-convert-rust/actions/runs/45400551593 — 17 tests; artifact 14924101531. Legacy build also succeeded.
- Successful analytics: https://github.com/emadgh/color-convert-rust/actions/runs/45402521624 — 21 tests; artifact 14923084302.
- Current v0.7 source: new CI not yet verified. Convex hull is an approximation and sampled target loss is not exact gamut volume.
