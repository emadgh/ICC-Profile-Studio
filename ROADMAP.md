# ICC Profile Studio — development tracker

## v0.3 — migration and inspection

- [x] Existing four-channel K/channel-limiter engine, v2/v4 LUT handling and non-destructive output retained (from v0.2.2).
- [x] Shared rebuild service extracted from legacy egui frontend.
- [x] LittleCMS 2 (static Rust binding) added for profile opening, metadata, TAC estimate and CMYK transform smoke checks.
- [x] GPUI sidebar shell and first Inspector/Validator/Rebuilder screens implemented in source.
- [x] Windows CI at commit 9b264922: LittleCMS 6.3.0 resolved, 17 tests passed and GPUI EXE built/uploaded (run 45400551593).
- [ ] Launch on Windows and verify layout and real printer workflows.
- [x] GPUI About icon and shared update-via-github UI with separate Studio asset/checksum names; package Studio EXE with checksum and docs in CI.
- [ ] Private feed/authentication and stable Studio releases, verify updater, port remaining legacy controls, drag-and-drop, accessibility, promotion to default.
- [ ] Run real Durst ICC v2/v4 regression tests and print/firing checks. Proprietary customer ICCs must stay out of the repository.
- [x] Rebuilder validates all three output intents with strict channel bounds, exact affine-curve or conservative per-cell TAC bound, independent 14,739 LittleCMS pipeline samples; explicit TAC must be certifiable before save. Validator shows strict bounds or unsupported reason. Tests added; CI pending.

## v0.4 — comparison (source implementation started)

- [x] Compare two CMYK output profiles using LittleCMS transforms, media-relative D50 PCS and native CIEDE2000.
- [x] Mean, P95, maximum Delta E, worst RGB, histogram, sampled per-channel means/maxima, TAC and 33 neutral-axis samples.
- [x] Profile Comparison page in experimental GPUI frontend; background analysis and selectable 9/17/33 RGB cube.
- [x] Reference CIEDE2000 test pair and ink-statistics tests added.
- [x] CI compiled comparison and passed its unit tests at 9b264922.
- [x] Four separation rendering intents, complete 33-point neutral-axis L*/four-ink plots and rows, CSV including intent, TAC and all neutral ink percentages. Self-comparison integration test added; CI pending.
- [ ] Compare real printer profiles and measured validation.

## v0.5 — profile editing (source implementation started)

- [x] Tone gamma editor: compose 4096-point output curves on B2A, retain the physical A2B model, support simple mft2/mBA.
- [x] Channel permutation: re-index A2B CLUT axes, input curves, B2A outputs and colorant table. Reject unknown or channel-sensitive tags; no silent metadata loss.
- [x] Metadata renamer: copy with new ICC description from filename, preserve transform tags.
- [x] GPUI screens and new-file-only save, LCMS opening and CMYK transform creation before writing.
- [x] Unit tests for forward/reverse permutation and v4 LUT encoding.
- [x] CI compiled editing and passed its unit tests at 9b264922.
- [ ] Real profile regression, full transform-equivalence testing, gamma/point-curve regression; bounded multipoint editor now implemented separately, legacy gamma still requires revalidation.
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
- [ ] Actual printer files, axis tick labels, nonconvex mesh/volume and runtime checks.

## v0.7 — selective color and smoothing (source implementation)

- [x] Selective D50 Lab/LCh correction: circular hue mask, smooth falloff, lightness/chroma/hue controls and protection of near-neutrals and extreme lightness targets.
- [x] Use LittleCMS native tag Pipeline evaluation of each original B2A intent; bake separate 16-bit mBA CLUTs, keep A2B and source channel maxima. Optional explicit TAC; verify every node and output bounds. No reuse of a single rendering intent.
- [x] Bounded six-neighbor LUT smoothing, strength 5–50%, 1–5 passes. Convex averaging of interior nodes; no averaging of boundary nodes. Resampling and explicit TAC still affect boundaries.
- [x] GPUI pages, 17/33/49/65 grid, output auto-selected in shared before/after Preview, no source overwrite, LittleCMS transform smoke checks.
- [x] Five unit tests for hue wrapping, protected neutrals, locality and ink bounds.
- [ ] New CI and integration regression on a valid ICC, quantify drift outside selective band, real printer tests and compare before/after using Gradient Analyzer and Delta E00.
- [x] GPUI mouse orbit and wheel zoom in the shared gamut preview; existing button controls retained.
- [ ] Verify mouse interactions on Windows; ICC v2/other LUT structures, multiple selective masks.

## v0.8 — image processing / v0.9 DeviceLink (source implementation)

- [x] Batch RGB PNG/JPEG or interleaved RGB/CMYK TIFF 8/16 input to 16-bit CMYK TIFF using LCMS and tiff/image codecs; embed destination ICC, explicit DPI, four rendering intents, reject untagged input unless explicit assumption.
- [x] Export relative sRGB before/after previews, four reduced-size separation PNGs and TAC heatmap; show in GPUI with sampled full-image ink stats and reject pixels over selected TAC without silent clipping.
- [x] Native LCMS CMYK-to-CMYK DeviceLink v4.3 export, reopen and validate transform creation, no overwrite.
- [x] Batch per-file error/progress and staged result directories, basic TIFF round-trip tests.
- [ ] CI and real ICC/image regression; real large-image regression, DPI metadata preservation, exact calibrated monitor proof, alpha, tiled/planar and multipage workflows.
- [x] DeviceLink sampled numeric comparison against direct conversion: 6,561 CMYK samples, maximum per-channel difference, sampled maximum channels and TAC.
- [ ] DeviceLink strict bounds proof, black-preserving link modes and direct DeviceLink image application.

## v0.9 / v1.0 — ink recipes and ceramic lab (source implementation)

- [x] Color Recipe Finder: LCMS forward predictions, D50 Lab target, channel/TAC constraints, relative ink cost weights, ΔE00 tolerance, coarse search and local refinement; reports original, closest and lowest-cost feasible recipe found.
- [x] Ceramic direct-ink TIFF chart with selected caps/TAC, 3/5/7 levels, patch layout CSV and blank measured Lab template; no simulated measurement data.
- [x] Actual measured machine/glaze/firing/ink-batch comparison using matched patch IDs and recipes, ΔE00 mean/P95/max and signed mean Lab bias.
- [x] Argyll colprof integration for an actual supplied CMYK TI3; independent new printer ICC output and LCMS class check.
- [x] Whole-profile bounded ink optimization using original LCMS B2A and A2B1 pipelines, lower relative weighted cost subject to predicted per-node ΔE00 tolerance; v4 mBA, 17/33 grid, preserve source caps and refuse source over selected TAC.
- [ ] CI and real print trials, validate imported measurement-file workflow, output profile quality, black generation validation, numerical drift between optimized nodes and real ink cost. Measured compensation/machine matching requires real datasets and validation; comparison alone is not compensation.
- [x] Local profile library with CSV line/printer/glaze/ink batch/firing/notes metadata; browse reference/candidate without modifying ICCs; template export and rescan.
- [x] CSV export of profile comparison and gradient analysis.
- [x] Local settings persistence (preview, grids, DPI, streaming, proof options, monitor/library/output paths); explicit save, startup restore, validation and atomic replacement. Device-specific caps/TAC, assumptions and jobs are not restored.
- [ ] Full project/session persistence, certified device limits and UI migration completion.

## v1.0 — advanced separation (source implementation)

- [x] Black Generation / GCR / UCR preference editor: onset, target K maximum, gamma, strength, chroma-sensitive scope, per-node LCMS predicted ΔE00 tolerance. Local constrained solver; K0 stays K0, original caps preserved and baseline over TAC rejected.
- [x] Monotone five-point per-channel output correction, bounded v4 mBA resampling, strict source caps and explicit TAC.
- [x] Unit tests for K0, UCR saturated-color protection and point monotonicity.
- [ ] New CI, complete real-profile checks, richer editable curves, black generation interpolation validation; this is a predictive preference, not measured profiling.

## v1.0 — large TIFF workflow (source implementation)

- [x] Interleaved single-image RGB/CMYK 8/16-bit strip TIFF streaming, 2 MP per strip and up to 2 billion pixels total; per-strip LCMS conversion, strict TAC rejection, staged output, thumbnails and stats.
- [x] Explicit classic/BigTIFF selection for streaming, optional full-resolution 16-bit grayscale separation TIFFs, per-strip progress and cancellation (in-memory jobs cancel between files).
- [x] Synthetic native ICC/BigTIFF, native selective correction and DeviceLink integration tests passed in Windows GPUI run 45409608575 (45 tests overall).
- [ ] CI and end-to-end real large-file/RIP regression; tiled TIFF, planar input, metadata preservation and calibrated monitor proof. No claim of unrestricted file-size support.

## Monitor proofing

- [x] LittleCMS native proofing transform with explicitly selected RGB display ICC, relative/absolute media simulation, optional relative BPC, RGB/CMYK source ICC handling and paired tagged PNG exports. Proof viewed externally in a color-managed application; GPUI raw texture preview is not calibrated.
- [x] Test of monitor ICC embedding and proof pipeline added.
- [ ] CI, real calibrated monitor visual comparison, large-file proof sampling, integrated color-managed GPUI display and out-of-gamut overlay.

## Gamut preview rendering repair

- User reported no rendered gamut in either 2D or 3D on Windows. Root cause not yet reproduced on Windows; investigate render/layout separately from data.
- [x] Independently tested three local printer ICCs (original, K0, K10) with native LittleCMS: 3,440 valid Lab targets each; 3,279 / 1,338 / 1,348 distinct Lab points. Proprietary files not committed.
- [x] Replaced GPUI custom Canvas drawing with a CPU-rendered PNG via existing image crate and fixed 320×320 GPUI image element; always display axes, auto-fit common scale, brighter points, visible counts, 2D outline and 3D wireframe, separate error status and PNG export. Existing mouse controls and before/after toggles retained. Plot positioned immediately below view toggles; cache keyed by sample revision and view parameters avoids regenerating images on unrelated renders.
- [x] Added raster pixel-change, toggles/slice, and synthetic native ICC-to-raster tests. CI and actual Windows rendering still pending.

## Release engineering

- [x] Studio-specific version file (0.9.0-beta.1), immutable separate Studio prerelease packaging with EXE, SHA256 and ZIP; legacy release kept independent.
- [x] Manual opt-in publication on main only after the Studio CI tests/build succeed; prerelease is not marked latest. Existing updater cannot automatically consume the private beta feed.
- [ ] Run and verify the new CI, optionally invoke workflow_dispatch with publish_preview=true, Windows interactive smoke tests, and real print regression before a stable release.

## Next milestones

- v0.4: Complete validation/export of Profile Comparison, Delta E2000, neutral axis and ink distribution.
- v0.5: Complete runtime validation of Tone Curves, channel mapping/reordering, profile metadata editing.
- v0.6: Validate new analytics and hull build; actual printer regression, richer controls and nonconvex gamut analysis.
- v0.7: Verify selective Lab/LCh correction and smoothing, add multi-region controls and verify mouse orbit.
- v0.8: Verify image conversion and batch, calibrated proof, streaming and full-size separations.
- v0.9: Verify DeviceLink, add constrained ink saving and cost optimization.
- v1.0: Ceramic profiling, reference charts, measured glaze/firing/ink batch compensation, machine matching, profile library.

## Dependency-first architecture

- **LittleCMS 2**: use mature library for ICC parsing, supported transforms (v2/v4), intent handling, proofing, color metrics and DeviceLink construction. Rust crate `lcms2` is statically linked to keep the Windows portable distribution simple; do not ship random DLLs.
- **ArgyllCMS**: keep official external tools for supported v2 revfix, measurement/profiling and diagnostics when required. Download official packages only, with trusted checksum if available. Do not route unsupported v4 through revfix.
- **Custom Rust**: retain only constrained separation optimization, strict per-channel bounds and missing functionality. Replace homemade forward evaluation and serialization with LCMS-backed calls where proven equivalent; never merely relabel v4 or clip existing output as if reprofiled.
- **GPUI**: requested “pgui” interpreted as GPUI from Zed. New experimental frontend shares backend with egui; legacy release stays default until migration is verified. Prefer existing component crates where compatible.
- **rfd**: native open/save dialogs. **update-via-github**: existing updater, private-repository limitation remains.
- Physical print correction requires measured data. No claim that simulated color accuracy is verified by print tests.

## Explicit scope exclusions

- Direct connection to spectrophotometers/colorimeters, instrument drivers, SDKs and live measurement acquisition are out of scope by owner request. Retain external CSV/TI3 measurement import and chart generation.
- Profile Inspector: display stored mediaWhitePointTag (wtpt) as XYZ, derived D50 Lab and approximate sRGB swatch; no fabricated white when missing. Output profiles use Paper / Substrate Color label. Source implementation and unit tests added; CI pending.

## Status rules

Check boxes mean source implementation, not verified runtime unless explicitly marked. Update this tracker in every feature commit. CI must pass before marking a build verified. Never overwrite the input profile.

## Build evidence

- Successful baseline: https://github.com/emadgh/color-convert-rust/actions/runs/45400551593 — 17 tests; artifact 14924101531. Legacy build also succeeded.
- Successful analytics: https://github.com/emadgh/color-convert-rust/actions/runs/45402521624 — 21 tests; artifact 14923084302.
- v0.7 and all modules through integrated commit b3e71c44 compiled successfully (36 tests; run 45406351743, artifact 14924639806). Convex hull is an approximation and sampled target loss is not exact gamut volume.

- v0.7 legacy build succeeded at 71fb5180. GPUI failed on mouse event type inference (E0282); explicit event types added in the subsequent source. Complete Studio build succeeded at b3e71c44.
- v0.8 legacy/core build succeeded at 64e71891; GPUI compiled at b3e71c44; runtime and end-to-end checks remain pending.

- Studio release workflow initially failed YAML validation at 310ad675; corrected the malformed PowerShell regex line and artifact block. New release workflow CI pending.
- GPUI build at 10af67e9 passed 45 tests and uploaded artifact 14929160937: https://github.com/emadgh/color-convert-rust/actions/runs/45409608575.
- Legacy/core build at 6bfdc843 (paper/substrate color) succeeded: https://github.com/emadgh/color-convert-rust/actions/runs/45410491959. GPUI verification pending.
- Soft Proof initial build bba58f12 failed because Rust binding uses SOFT_PROOFING rather than SOFTPROOFING. Corrected at aa1e1f54; integrated GPUI CI passed all 53 tests, artifact 14930171063 (run 45411400981). Gamut rendering repair awaits new CI and interactive Windows confirmation.

## Gamut R2 runtime report

- Owner reports previous raster build still shows no gamut or grid. Previous commit 797551e7 passed both Windows workflows (Studio run 45414370996). Runtime failure is not resolved by CI alone.
- [x] Pin plot above and outside settings scroll, add fixed-size frame, independent colored test swatches, visible point and native span counts, render ID R2 in header, duplicate plot on Gamut Viewer page.
- [x] Default native colored Div spans (no Canvas or image path); alternate synchronous predecoded BGRA RenderImage bypasses PNG asset decoding. Both use identical cached raster. Add tests for nonempty native axes with no ICC and changing span output when profile visibility toggled.
- [ ] Confirm Windows visual display in both paths, at common display scales, with actual printer ICC. Do not mark as fixed until confirmed.

## Gamut R3 — profile colors and nonconvex surface

- Owner confirmed BGRA display works in R2; keep both paths, default BGRA in R3. R2 Windows CI succeeded (run 45419934663).
- [x] Source: Profile Colors (D50 Lab to approximate sRGB, clipped to display) and Compare Tint; points, wireframe, solid and solid + wire; separate per-profile depth buffer and overlap blending. Same raster for BGRA/native and exported PNG.
- [x] Source: adaptive 25³/41³/57³ Lab lattice, LCMS round-trip ΔE00 ≤ 2 isosurface using marching tetrahedra, no convex closure. Keep optional convex geometry and label tolerance-based approximation; 2D projection and mesh plane slices; points fallback reported if no surface is found. No exact gamut-volume claim.
- [x] Source: persist new settings, direct BGRA default, show triangle counts, tests for disjoint regions, color conversion, depth and slice.
- [ ] Windows CI and user visual verification, performance and surface convergence on real printer profiles. Surface field box/padding and tolerance can affect extent; not a physical measured gamut. No measurement instrument integration.

- Local independent LCMS round-trip checks for all three Durst ICCs at the 41³ adaptive field: 2,565 original / 3,020 K0 / 2,944 K10 lattice nodes within ΔE00 ≤ 2, no sampled outer-box face hits. These are reproducibility-grid counts for different adaptive boxes, not directly comparable volumes. Proprietary ICCs were not uploaded. R3 Rust tests and Windows display remain separate verification gates.
- Preserve black pixels in native-cell fallback and test filled solid triangle interior; distinguish profile-color legend from comparison tint. Quality labels Draft/Normal/High map to 25³/41³/57³ surface field.
