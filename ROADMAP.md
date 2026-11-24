# ICC Profile Studio — active development tracker

## Product scope (owner decision, beta.5)

ICC/ICM profile inspection, validation, editing, comparison, construction and characterization only. GPUI is the sole application frontend and default executable.

- [x] Remove Image Converter, batch image conversion, image separation previews, image TAC heatmaps, image Soft Proof and related settings from the active application and build.
- [x] Remove image conversion/streaming source modules; earlier implementations remain in Git history at 32e6b7899b42e8cbe3a0942795aa0cc02a3de19f. No active image-processing roadmap.
- [x] Retire egui executable, eframe dependency, egui updater UI and legacy Windows workflow. Keep shared ICC engines and GPUI updater integration.
- [x] Retain ICC DeviceLink *generation/validation*, not applying DeviceLinks to images. Retain synthetic characterization chart TIFF export, CSV/TI3 import, profile-analysis graphs and gamut PNG export; these serve ICC work, not image editing.
- Out of scope: image editing, TIFF/BigTIFF image conversion, image metadata/alpha/multipage handling, image proofing/out-of-gamut overlay, direct instrument connectivity. Revisit only at owner request.

## Implemented profile tools

Checkbox indicates source implementation; runtime/print validation remains separate.

- [x] Inspector: ICC tags, class, version, PCS, channel names, media white point (wtpt) XYZ/Lab/display swatch; missing values not fabricated.
- [x] Rebuilder: supported v2 mft2 and v4 mAB/mBA, per-channel limits, TAC, custom solver and supported Argyll revfix. Strict channel/TAC bounds and 14,739 native B2A samples. New output only.
- [x] Profile Comparison: four intents, CIEDE2000, neutral-axis graphs, ink distribution/TAC, CSV.
- [x] Gradient Analyzer and Gamut target coverage analysis.
- [x] Tone gamma, bounded five-point curves, channel permutation (reject unknown vendor tags), metadata rename.
- [x] Single-region LCh correction, LUT smoothing, predictive GCR/UCR preference and bounded ink optimization; recipe finder.
- [x] DeviceLink ICC generator and 6,561-sample numeric comparison, no direct image application.
- [x] Gamut R5: 2D/3D, shared scale, BGRA-only rendering, actual Lab-derived approximate sRGB, nonconvex sampled Delta E00 <= 2 surface, optional convex hull, reference solid/candidate wireframe, labeled grid, floor contours, orbit, zoom, PNG export.
- [x] Profile Library, CSV metadata, persistent preferences, GPUI About and shared updater.
- [x] Ceramic Lab: characterization chart, patch CSV comparison and colprof on real external TI3 data. No direct measurement instrument integration.

## Remaining active work

### Profile editing and analysis
- [x] Source: multi-region selective color correction (up to six; order-independent normalized overlap, original PCS masks, single resampling).
- [x] Source: editable gamma, bounded multipoint and requested black-generation curves; numeric controls and bounds retained.
- [ ] Windows interaction and real-profile/interpolation validation of all curve editors.
- [x] Source: restricted ICC v2.4/v4.3 structural conversion of simple CMYK/Lab mft2 and mAB/mBA, PCS encoding adaptation and four-intent native equivalence checks before saving.
- [ ] Verify real Durst conversion, unsupported-tag policy and broader LUT/text structures.
- [ ] Support more LUT/tag structures and explicit Durst vendor tag adapters only with verified specifications; preserve unknown tags safely.
- [ ] Improve constrained separation continuity, numerical comparison with LCMS, interpolation validation, full-profile optimization regression.
- [ ] Black-preserving DeviceLink generation and strict bounds/accuracy validation.
- [ ] Nonconvex gamut volume estimate, resolution convergence, performance and real Windows rendering checks.

### Workspace and UX
- [ ] Save/load projects and explicit session recovery; bind device settings to profile identity, avoid silent restoration.
- [ ] Drag/drop ICC/ICM, richer Profile Library search/filter/metadata editing, accessibility, remaining GPUI controls.
- [ ] Refine update feed and ensure only compatible Studio EXE assets with verified checksum can install.

### Ceramic profile work
- [ ] Measured profile correction, machine matching, glaze/firing/ink batch/shade compensation with real matched datasets and validation.
- [ ] Actual Durst profile and printed characterization chart regression. Keep proprietary production data outside GitHub.

### Release gates
- [ ] Verify beta.5 sole GPUI CI and artifact, Windows interactive smoke tests, printer regression, dependency/license review and release candidate before stable 1.0.
- [ ] Validate Argyll download checksum/redistribution terms, update feed behavior and package contents.
- [ ] No physical color accuracy or exact gamut-volume claims without appropriate evidence.

## Build evidence

- Historical R4 commit 32e6b7899b42e8cbe3a0942795aa0cc02a3de19f: GPUI and legacy Windows CI passed, 67 tests, Studio run 45454105352. Earlier tests included now-retired image features; test count is expected to change.
- R2 BGRA display confirmed by owner. R4 visual confirmation still pending.
- beta.5 ICC-only GPUI CI succeeded (45458550421).
- Previous detailed tracker and image modules are recoverable from Git history: https://github.com/emadgh/color-convert-rust/blob/32e6b7899b42e8cbe3a0942795aa0cc02a3de19f/ROADMAP.md

## Architecture and status rules

LittleCMS (static Rust binding) handles ICC parsing/transforms, color metrics and DeviceLinks. Official Argyll tools are used for supported revfix/colprof. Custom Rust handles constrained separation optimization, validated LUT serialization, and gamut geometry. GPUI and rfd are the only frontend/dialog stack; image crate is retained for generated gamut graphics and UI resources, tiff only for characterization chart export. All ICC edits create a new file. Update this roadmap with each feature commit; distinguish CI from runtime and physical validation.

## Gamut R5 and curve UI (beta.6)

- [x] beta.5 ICC-only GPUI CI succeeded (45458550421).
- [x] Source: BGRA only; remove native cell rendering and settings. Fix yaw sign, resize right pane via drag handle, expanded in-app gamut view, responsive plot size, compact grouped controls and collapsed advanced options.
- [x] Source: preserve RGB hue in Both (uniform brightness multiplier), quad-style wire grid via constant L* rings and hue meridians (not an all-quad mesh), 2D projected/sliced outline and 0–100% fill opacity, union floor silhouette per visible profile, clear/select candidate and active-profile count.
- [x] Source: reusable Curve graph with two-column gamma and bounded five-point cards, draggable handles, reset and numeric steps; endpoints and monotonicity preserved.
- [ ] CI and actual Windows interactive validation of resizer/orbit/expanded mode/curves and display quality. Expanded mode is within the app window, not OS-exclusive fullscreen. BGRA raster remains 640px and scales to fit. Floor can have disconnected contours where the projection is disconnected.

## beta.7 — multi-region and curve completion

- R5 library tests completed, GPUI failed E0308 because gamut_plot returned Div instead of Stateful<Div> (run 45462752947). Fixed return type; new CI pending.
- [x] Source: 1–6 region editor with add/remove/select/reset and hue swatches; combine active regions using original Lab weights, normalize overlap and preserve near-neutrals/extremes; independent B2A resampling, original A2B and source caps preserved. Tests for overlap/order, disjoint masks, invalid settings and native ICC K0 regression.
- [x] Source: reuse curve graph for black preference, draggable midpoint gamma and maximum K; existing start, tolerance, strength, UCR/GCR controls retained. Graph is requested preference before constraints, not measured separation.
- [x] beta.7 GPUI CI passed 65 tests (45464642389).
- [ ] Windows rendering/input; actual Durst profile and print regression. Next: structural ICC version conversion, project/session and vendor-tag support.

## beta.8 — structural ICC version conversion

- Source: six intent tags retained with actual LUT type re-encoding, PCS Lab legacy/full-scale adjustment in PCS-side curves, shared known metadata retained and ASCII text recoded. No source overwrite. Optional/unhandled tags listed and require explicit omission consent; alternate D2B/B2D always refused. Unknown vendor data not assumed compatible. Restrict to supported equal-grid sampled LUTs and single en-US ASCII text.
- Native four-intent forward ΔE00 and reverse channel/TAC checks with user-set limits, source mathematical bounds and exact zero channel preservation. Reject before save on failure. Output selected as candidate. Tests for round-trip types, K0, metadata consent and no overwrite. CI/runtime and real RIP verification pending.
- Reference: https://www.color.org/v2issues/ (legacy Lab encoding). Next: project/session and safe vendor tag inspection.

- Independent local Python implementation of identical PCS-side conversion, native system LCMS (not Rust executable): actual Durst v4 original/K0/K10 have unequal 4096/2/2 forward curves. Harmonize via LCM of segment counts, retaining original knots. Four-intent verification with 6,561 CMYK and 3,541 valid PCS samples per intent: max ΔE00 Relative 0.0067, Absolute 0.0057 for all three; reverse max channel difference Relative 0.1053 pp and Absolute 0.0809 pp across all three. Perceptual max ΔE00 9.882 and Saturation 32.3727; large reverse differences. Default strict all-intent conversion must refuse these profiles. Explicit Relative/Absolute-only compatibility mode reports all intents including failures and warns not to use them. No proprietary data uploaded; this is not a verified production RIP result.

- Legacy identity PCS output curves can contain unused values above 0xFF00. v2→v4 composes the clipped scaling as a sampled curve when needed (not just endpoint scaling); verification still rejects actual transform drift. Added regression for legacy identity curves.

- Initial beta.8 run 45467850591: 68 tests passed, one all-intent equivalence assertion failed (synthetic Perceptual ΔE00 19.0045). This confirmed the strict gate; regression now explicitly expects rejection and separately tests the opt-in Relative/Absolute mode, retaining all four measured intent rows. Compatibility filename defaults to _REL_ABS_ONLY. Latest Windows CI pending.
