# Studio release validation

This is a preview, not a production-certified printer profiler.
CI runs only on the explicit `ci:run` label or manual dispatch. Every accepted
PR must have successful CI for its current head. The package verification gate
checks archive membership, version, Windows AMD64 PE header, executable SHA-256
and build provenance. It is not Authenticode verification or a UI runtime test.

## Manual acceptance checklist (not yet executed)

- Launch the extracted EXE on Windows 10/11 x64 without Rust installed.
- Test real CMYK v2/v4 and nCLR printer profiles, embedded names and corrupt files.
  Compare original file hashes before and after editing.
- Verify all reverse intents, ink ceilings, zero-ink channels and TAC; compare
  gradients, gamut and Delta E. Test supported reorder and version conversions.
- Check all five languages, especially Persian/Arabic mixed Latin numbers,
  paths, long labels and wrapping. Test compact/wide windows, zoom, orbit,
  expanded gamut, sliders and profile dropdown.
- Import actual CGATS/CxF; confirm measurement conditions and round trips.
  Compare independent holdout measurements; fit is not independent validation.
- Verify RIP mapping and ink limits before printing separated TIFF charts.
- Verify the Argyll distribution independently. ARGYLL_SHA256 is an optional
  trusted operator-supplied digest; without it the downloader checks HTTPS,
  not an independently authenticated checksum. Existing local binaries are
  not attested.
- Audit dependency licenses, required notices and advisory reports; CIE
  attribution is included separately. Test release/update behavior.
- Verify preferences do not restore device-specific ink limits or running jobs.

## Publishing

Manual workflow dispatch on main with publish_preview enabled creates a
prerelease only after successful tests, compilation and archive verification.
Regular CI uploads one ZIP instead of duplicating the EXE. Release assets
retain the standalone EXE and checksum. No stable release or automatic beta
update is implied by CI success.

## beta.28.1 workflow regression checks (pending)
- Open Profiling Lab with embedded target, then a profile without one; verify automatic color grid and clear measured/predicted provenance. Change reference while the automatic task is pending; reject stale result. Check all tabs, advanced filters and explicit exports.
- Change reference from each active page via both file dialog and installed profiles; verify navigation is retained, prior data invalidated, and Comparison candidate retained.
- Compare same and different channel counts with overlay and side-by-side modes, identical curves, shared axes and missing channels. Switch optimizer tabs; verify weights/tolerance are shared and three results aligned.
- CI generates Cargo.lock only if absent and embeds it for audit; pin the reviewed lockfile in the repository before claiming reproducible dependencies.

### beta.29 editor and update QA (manual)
- [ ] Click sidebar logo/name from every workspace; About opens and update check appears first.
- [ ] Startup check is nonblocking; newer compatible public Studio beta displays a once-per-version prompt and sidebar shortcut. Later dismisses prompt but retains shortcut. Manual check, unavailable network/private release, checksum failure, verified download and explicit restart behavior.
- [ ] Confirm native title, Alt+Tab/taskbar label, Task Manager Processes display and executable Properties. Windows may retain cached pinned shortcut names; Details tab normally shows the executable filename.
- [ ] Check all five redesigned pages with CMYK and nCLR, narrow windows, RTL, sliders, drag controls, and candidate exports. Compare same input/settings against prior validated builds.

### beta.32 multichannel acceptance (pending)
- Test synthetic regression matrix in CI, then run the opt-in real/reference corpus harness for each available count. Record successful and skipped operations separately; do not treat profile inventory as edit validation.
- Inspect embedded versus predicted channel colors in every workspace and test all language settings. Verify preview is not started when analysis is unsupported.
- Confirm 10% channel cap and zero channels after reopen, independently verify RIP mapping and four-intent color drift before printing.

### beta.33 performance acceptance (pending)
- Compare sparse and full search outputs; measure wall time on the same licensed 3–15-channel corpus, grid, caps, TAC, hardware and build. Report peak process RAM separately from scratch-buffer estimates.
- Cancel Custom rebuild before the first slice, mid-inversion and before publication. Confirm the original is unchanged, no output is created and controls recover. If publication wins the race, do not claim successful cancellation. Argyll revfix is not cancellable here.
- Switch Fast/Balanced/High gamut quality with the same profiles; confirm both compared profiles use the same target grid, saved preferences restore it, and output ICC resolution is unaffected. Check 2D/3D zoom, orbit, fullscreen and RTL.
- Run `cargo test --lib --no-default-features --release sparse_rebuild_benchmark -- --ignored --nocapture` for the synthetic microbenchmark. No elapsed-time target is asserted in CI.

### beta.34 legacy correction acceptance (pending)
- Verify v2 mft2 profiles enable bounded points, selective regions and smoothing only when all reverse intents are supported. Confirm v2 black generation/whole-profile optimization disabled with reason. Compare all four intents; test extended PCS endpoints, source caps, zero inks, explicit TAC, A2B bytes and no-clobber saves.
- Run opt-in FOGRA55 Rust regression with operator-supplied ICC; compare against independent native diagnostic and print/RIP validation. No physical color approval from structural checks alone.

### beta.35 N-color model acceptance (pending)
- Open FOGRA55 ICC and use Patch explorer / Import CGATS to load the matching external data, then Evaluate N-color baseline. Verify grouped counts, comparison bars, ΔE00, TAC and nearest-recipe distances. Replacing source/chart clears previous results; virtual/missing/mixed measurements must fail explicitly. Test other channel counts and five UI languages.
- Run optional `fogra55_ncolor_baseline` with local data. Independent Python reference values in NCOLOR_MODEL_QA.md are a comparison baseline, not compiled Rust results. Do not treat holdout results as independent physical print validation or export any ICC from this experiment.

### beta.36 acceptance (pending)
Verify all five folds, pooled counts, local ridge error bars, active-channel histogram, optional stress report (not available for insufficient data), clear-on-import, worker/UI responsiveness, five languages. Run opt-in FOGRA55 reference assertions. No ICC output or production accuracy claims.

### beta.37 acceptance (pending)
Verify inverse solver with closed channels, zero and fractional caps/TAC, impossible-to-match targets (report not reached, not proven impossible), unsupported coverage, deterministic repeat, local sensitivity and settings snapshots. Run forward continuity audit and five-point L* ramp; verify all rows, adjacent changes, stale result warning, source/chart resets and five languages. Real FOGRA55 Rust test opt-in with ICC_FOGRA55_DATA. Python mirror is diagnostic only. No ICC serialization or production approval.

### beta.38 acceptance (pending)
Compare independent and refined ramps in Profiling Lab. Check lambda=0 identical baseline, invalid weight rejected, pointwise tolerance guard, zero and fractional ink caps/TAC, deterministic refinement, stale settings, chart resets and five languages. Run synthetic tests and optional FOGRA55 reference test. Assess dark ramp separately; no ICC production claims.

### beta.39 acceptance (pending)
Run 3D lattice audit in Profiling Lab. Check 27 nodes, 18 edges per axis for complete cube, eight complete cell centers, missing nodes not zero-filled, invalid spans/PCS/budget rejection, lambda=0 identity, caps/TAC and pointwise tolerance, stale results and L* slice navigation, five languages. Opt-in real FOGRA55 test; no claims of whole-cell or global gamut coverage, no ICC output.

### beta.40 acceptance (pending)
Verify per-ink grid controls, preset, product overflow and node/work budget, chart resets and stale reports, full-domain warning, no unsupported-corner fill, shared holdout coverage and model comparison, five languages. Synthetic ordinary tests; optional FOGRA55 reference assertions. Raw table size excludes ICC overhead. No ICC export or claim of CMM equivalence.

## Current release checklist (2026-11-25; all unchecked until evidence is recorded)

- [ ] #24 exact-head CI success after fix; previous #106 had 184 passed/4 ignored backend tests but failed GUI build.
- [ ] #25 Persian/Arabic full UI layout, mixed-script/Latin paths and numeric fields, both DPI sizes; no reversed scientific axes or colorant order.
- [ ] #26 All five languages, dynamic parameterized diagnostics and raw native details; no missing static strings in priority workflows.
- [ ] #27 Sliders, +/- controls, direct numeric entries, keyboard and clamping; compare chart scales, legend/ink order, zero and missing data; busy guards.
- [ ] #28 Progress reflects actual phases/counts or indeterminate; cancellation before/mid/after sampling, no stale reports or partial ICC output, controls recover. Custom rebuild publication race correctly reported.
- [ ] #29 About opens from logo/name; top update action, startup nonblocking, available shortcut/prompt, offline/invalid checksum, verified download/explicit restart, title/taskbar/Task Manager/version.
- [ ] #30 Gamut: single reference with each of four intents; compare same intent, toggle separate and choose different intents, turn off and both use reference; re-enable starts aligned; hidden candidate control without candidate. Verify matched targets, cache invalidation, 2D/3D/expanded panel, analysis page, busy guard, native errors and five locales. Relative default; session-local initially. No source changes; cross-intent caveat and native fallback displayed. Native synthetic test is not Windows UI acceptance.
- [ ] #33 Licensed real-profile matrix and opt-in Rust FOGRA55; independent native comparison, RAM/time/gamut convergence, Windows 10/11 smoke tests, RIP mapping and print validation where relevant.
- [ ] #33 Reviewed Cargo.lock, dependency/license/security review, Argyll provenance, portable ZIP/version/SHA256, update and RC verification.
- [ ] #32 Advanced ICC compatibility remains conditional; document unsupported structures/channel-count conversion and DeviceLink constraints.
- [ ] #31 New N-color profiling remains experimental; no serialization until full domain coverage, reverse continuity, media normalization, native CMM and independent physical tests satisfy approved gates.

Evidence per check: build commit, input profile signature/version/channel count, operation/intent, settings, outcome, logs/screenshots or measured data and remaining limitations. Do not mark completion on CI alone when manual/physical acceptance is required. Track issues at https://github.com/emadgh/color-convert-rust/issues.

### RTL and diagnostics hardening — partial, pending CI/manual QA
- [ ] Switch en/fa/it/ar/ru during a job; exact known status updates in current language; unknown native errors preserve details and file paths, no translated value is sent to an engine or storage.
- [ ] Verify right-aligned workspace, sidebar and preview, mirrored system picker/header and bottom status; compare mixed-script, numbers and paths with LTR mode. Axis orientation, slider direction, channel ordering and 3D orbit unchanged.
- [ ] Test mixed-script wrapping, punctuation, editable input, focus and Windows DPI; GPUI display-only Arabic shaping is not full native bidi layout.
- [ ] Inventory remaining dynamic success/status messages, technical descriptions and raw native diagnostics; replace with stable typed IDs/parameters in future batches. Static literal translation checks are not proof of complete UI translation.

### Beta.41 consolidated CI and manual gates (pending)
- [ ] One exact-head Windows CI for consolidated #35, including #34; previous CI #108 had already been launched before consolidation. No per-commit CI.
- [ ] Numeric value click, type ASCII/Persian/Arabic digits, decimal, minus, Enter/Escape/Backspace/Delete; invalid/out-of-range values rejected, step snapping and endpoints, busy guard, focus changes/page switches. Manual keyboard/IME and clipboard acceptance outstanding.
- [ ] Read-only jobs: cancel before/mid/after checkpoint, no stale/cancelled result publication, correct progress phase count, UI recovery. Verify gamut in expanded mode, comparison, gradient, optimizer and all N-color audit workers. Rebuild uses separate publication-safe control; other write/external jobs not covered.
- [ ] Gamut shared/separate intent selectors, same target set, all four intents, 2D/3D/panel and expanded mode, caveat for cross-intent comparison, native errors.
- [ ] RTL text alignment, picker/header and bottom status, mixed-script paths and labels; full native wrapping/IME/focus still requires manual work. Five languages and raw diagnostic details.
- [ ] About/update state transitions, actual Windows offline/private feeds, checksum failure, verified download, explicit restart and title/taskbar metadata. No physical or stable-release approval inferred from CI.

### Beta.42 acceptance (pending)
- [ ] All prior Beta.41 checks; verify localized dynamic status headings and unchanged exact details/path, switch languages, invalid numeric entry and locale separators (reject exponents and percent syntax).
- [ ] Gamut progress counts completed native batches and completed mesh slices, phases reset independently; cancel during mesh extraction or after final checkpoint without publishing result. Test both profiles and expanded view; native transform batch may finish before cancellation.
- [ ] Explicitly save/load preferences with shared/separate intents, legacy settings default Relative, invalid intent code rejected, no device-specific job state persisted. Verify overlay labels and matching target grid.
- [ ] Windows interactive keyboard/IME, mixed-script wrapping, actual updater and real-profile/print validation remain manual. No stable release or physical QA claims.

### Beta.43 manual acceptance
- [ ] Click a numeric value, use Ctrl+V with ۱۲٫۵ / ١٢,٥ / 12.5 as permitted by field range; verify the edit does not change actual setting until Enter, Escape discards, Ctrl+C copies current buffer, Ctrl+A clears; reject multiline, units, exponent, direction controls and out-of-range values. Verify keyboard/focus at both DPIs and five locales. Native IME is not fully implemented.
- [ ] In About, check a newer compatible release; deliberately interrupt a download, verify Failed state and Retry download, then retry and verify checksum, Ready and explicit restart. Check a failed feed request does not offer retry of an obsolete download; checking afresh clears retry metadata. No live updater test claims from CI alone.
- [ ] Record exact build SHA, Windows version/DPI, language, ICC version/channel count, input and observed output; attach screenshot/log and pass/fail/blocked. Use a nonproduction profile and output path. All previous manual and physical verification gates still apply.

### Beta.44 legacy predictive editing (pending CI)
- [ ] v2 LUT16 structural preflight vs mft1, malformed/missing A2B1/B2A or alternate transforms; preserve all forward tags and profile version, independent B2A intents, cap/TAC certificate and no source overwrite.
- [ ] Verify v2 nominal 0xff00 Lab scaling and valid extended raw values; compare native CMM output. Confirm closed channels remain zero and reject infeasible baseline rather than clipping.
- [ ] Select grid 17/33; verify native 17 grid retained for legacy intent when 33 requested. Half tolerance at nodes; reject output if any of four intents fails native sampled 762-color acceptance. Test optimizer and physical-black preference on synthetic legacy fixtures; opt-in real FOGRA55 optimizer test.
- [ ] Compare before/after weighted ink cost, TAC, max Delta E and actual print measurements; clearly label predicted vs measured. No guarantee between samples. No claim of K=0 conversion to CMY.
