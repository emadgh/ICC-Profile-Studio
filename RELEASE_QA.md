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
