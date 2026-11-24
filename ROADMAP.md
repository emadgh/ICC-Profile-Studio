# ICC Profile Studio — development tracker

## Current milestone: v0.3 — migration and inspection

- [x] Existing four-channel K/channel-limiter engine, v2/v4 LUT handling and non-destructive output retained (from v0.2.2).
- [x] Shared rebuild service extracted from legacy egui frontend.
- [x] LittleCMS 2 (static Rust binding) added for profile opening, metadata, TAC estimate and CMYK transform smoke checks.
- [x] GPUI sidebar shell and first Inspector/Validator/Rebuilder screens implemented in source.
- [ ] Verify new code in Windows CI, run application and check layout. Initial CI failed because lcms2 6.2.0 was yanked; pinned 6.1.1 and rerun required.
- [ ] Port all legacy controls, updater, progress, drag-and-drop, accessibility and About icon to GPUI before promoting it to the default release.
- [ ] Run real Durst ICC v2/v4 regression tests and print/firing checks. Proprietary customer ICCs must stay out of the repository.
- [ ] Add independent LCMS transform checks for generated profiles; enforce numerical bounds through the strict LUT validator as well as sampled transforms.

## v0.4 — comparison (source implementation started)

- [x] Compare two CMYK output profiles using LittleCMS transforms, media-relative D50 PCS and native CIEDE2000.
- [x] Mean, P95, maximum Delta E, worst RGB, histogram, sampled per-channel means/maxima, TAC and 33 neutral-axis samples.
- [x] Profile Comparison page in experimental GPUI frontend; background analysis and selectable 9/17/33 RGB cube.
- [x] Reference CIEDE2000 test pair and ink-statistics tests added.
- [ ] Verify CI and compare real printer profiles; add export, intent selector, complete neutral curves, measured validation and gamut visualization.

## Next milestones

- v0.4: Complete validation/export of Profile Comparison, Delta E2000, neutral axis and ink distribution.
- v0.5: Tone Curves, channel mapping/reordering, profile metadata editing.
- v0.6: Gamut analysis and 3D visualization, gradient continuity.
- v0.7: Selective Lab/LCh color correction, controlled LUT smoothing.
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
