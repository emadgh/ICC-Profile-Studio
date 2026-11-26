# ICC Profile Studio

Vendor-independent, portable Windows x64 application for ICC/ICM inspection, color analysis, editing and construction. GPUI only. Scope: CMYK and multichannel printer output profiles and print DeviceLinks. CMYK and 2–15 channel nCLR output profiles are supported subject to per-tool structural and native preflight. Existing-profile editing is not N-color fitting.

## Workflow

Open a profile from disk or choose **System** to select an eligible installed ICC from the Windows color directory. Select a tool, adjust its parameters, and save a new ICC. Compare predicted colors, ink usage, gradients and gamut with the original. Never overwrite the source; verify physical ink mapping and print conditions separately.

## Tools

- Profile Inspector / Validator: ICC class, version, space, PCS, media white point, channel limits and native LittleCMS checks. Simple copy/rename in Inspector.
- Rebuilder, per-channel ink caps and TAC, Tone Curves, Channel Reorder, multi-region Color Correction, LUT Smoothing, Black Generation / GCR / UCR, Ink Optimizer.
- Profile Comparison, Gradient Analyzer, Gamut Viewer with 2D/3D, profile-derived display colors, solid/outline/quad-style wireframe, shared scale, resizable panel, expanded view, PNG/CSV analysis exports.
- ICC v2/v4 structural conversion for supported CMYK/Lab LUTs: four-intent sampled validation by default, explicitly restricted Relative/Absolute mode if requested. Unhandled tags require omission consent; no automatic private-tag interpretation.
- DeviceLink generation and validation (no application to images).
- Profiling Lab: direct-device chart generation, measured CSV comparison, measured TI3 profile creation using Argyll colprof.
- Local display preferences and About/Updates.

No profile library, project files, session management, dedicated tag manager, image import/conversion/editing, RGB/Gray/input/display/abstract editing, vendor-specific tools or egui. Historical code remains in Git history. The system profile picker is read-only, not a catalog or OS association editor.

## Engine limits

LittleCMS 2 is statically linked. Current advanced editors are restricted to supported equal-grid sampled LUT structures. Unsupported profiles are not silently converted. Rebuilds can regenerate B2A intents from relative-colorimetric inversion; color equivalence is sampled, not guaranteed. Gamut surface represents approximate round-trip coverage (Delta E00 <= 2), not measured physical gamut. Display colors are approximate sRGB. ICC characterization charts and reports are not image-editing features. Existing basic inspector can identify other profiles without implying editing support.

ArgyllCMS revfix/colprof use official Windows Argyll 3.5.0 installed when required in a writable application folder. Optional trusted ARGYLL_SHA256; upstream checksum/redistribution review remains pending. No Argyll binaries bundled.

## Build and download

Download the [Windows artifact](https://github.com/emadgh/color-convert-rust/actions/workflows/studio.yml) and verify SHA256. Extract and run ICC-Profile-Studio.exe.

    cargo test --lib --no-default-features --release
    cargo build --release
    cargo run --release

Default and sole executable: icc-profile-studio. Manual prerelease publishing is immutable. See [ROADMAP.md](ROADMAP.md) and [PROFILE_WORKFLOW.md](PROFILE_WORKFLOW.md). CI success does not establish Windows interaction or physical print accuracy.

Beta.12: grouped navigation, neutral charcoal/orange theme, developer credit and About, five-language UI dictionary with English fallback. High-quality nonconvex gamut only; approximate sampled occupancy volume. Full translation/RTL and multichannel expansion remain in development.

Beta.13: native LittleCMS forward pipeline inversion in the four-channel Rebuilder supports native-decoded parametric and nonuniform A2B LUTs, preserving original A2B bytes. CMYK and nCLR output profiles can use dynamic 16-bit native gamut and inspection, and simple-LUT mathematical channel/TAC bounds. Full multichannel editing and unsupported reverse structures are not yet supported.

Beta.14 adds a display-only standard/embedded channel-name switch, stored v4 colorant-table swatches, live channel/TAC limit bars, reorder palette, gradient line plots, bounded content width and Arabic/Persian display shaping with Unicode BiDi. RTL soft wrapping and Windows visual behavior remain to be checked.

Beta.15: dynamic nCLR Rebuilder adds per-channel caps, total ink ceiling, native forward inversion with bounded search and independent strict/native output verification. All three output intents are regenerated from relative A2B1; validate color drift and print behavior. Other multichannel editing modules remain limited.

Beta.16 extends Profile Comparison and Gradient Analyzer to CMYK/nCLR profiles, including unequal channel counts. Ink statistics and trajectories are independent per profile; no inferred physical channel mapping. CSV reports include all channels. The reference RGB sample grid is internal, not RGB-profile editing.

Beta.17: gamma editing and channel reordering use dynamic CMYK/nCLR channel counts (2–15, subject to LUT/native support), with per-ink graphs, swatches and mapping controls. Supported sampled equal-grid mft1/mft2 and simple mAB/mBA only; mft1 is upgraded to mft2. Gamma can increase ink use: validate TAC and caps. Bounded multipoint, color correction and DeviceLink remain limited. Reorder must match RIP physical ink mapping; native interpolation equivalence is not guaranteed.

Beta.18 improves the interface with drag/click sliders plus fine-step buttons, a floating installed-profile selector, explicit persisted name-mode selection, and darker backgrounds with contrast-safe ink graphics. Missing embedded colorant names fall back to standard names; no names are invented. Adds 48 localization entries across five languages. Dynamic nCLR bounded point curves and selective correction support simple v4 Lab mBA profiles at grids 17/33, with source caps, explicit TAC, independent reverse intents and native checks. CI and visual testing remain required.

Beta.19 adds bounded nCLR LUT smoothing and native numeric verification for channel reorder. Smoothing uses six-neighbor averaging, 5–50% strength and 1–5 passes with independent B2A intents, original A2B, source channel bounds and explicit TAC. Reorder checks four intents with deterministic device and PCS samples; maximum ΔE00, per-channel deviation and TAC deviation must each be ≤0.5 (ink quantities in percentage points) before save. Sampled validation is not exhaustive or a physical print guarantee. CI pending.

Beta.20 adds mixed CMYK/nCLR DeviceLink generation with native 16-bit conversion, explicit/automatic bounded CLUT grid, one selected rendering intent, configurable sampled channel tolerance, independent source and destination colorant metadata, and per-channel/TAC/ΔE00 approximation report. 2–15 channel signatures are accepted subject to native and grid support. Links are validated against the direct profile chain, not against measured print. No special K preservation, BPC or certified physical ink cap. CI and real-profile validation pending.

Beta.21 adds CMYK/nCLR ink recipe search with dynamic weights and strict source channel caps, TAC constraints and user-selected ΔE00 tolerance. Whole-profile optimizer supports simple ICC v4 Lab mBA, using independent B2A intent tables and original A2B1 predicted appearance. Search is bounded and approximate; costs are relative indices, not physical ink mass or currency. Whole-profile results report 729 relative sRGB samples, not guaranteed global accuracy or real savings. CI pending.

Beta.22 extends predictive black generation to compatible CMYK/nCLR ICC v4 Lab profiles. The physical black channel must be explicitly selected per profile; names and swatches do not establish ink identity. Uses GCR/UCR-inspired preference curves, source/Rebuilder cap intersection, TAC and per-node predicted ΔE00. Closed channels cannot be reopened. Reports 729 RGB and 33 neutral samples before/after. The curve is a soft preference, not a hard black cap or measured GCR/UCR calibration. CI pending.

Beta.23 generalizes structural v2/v4 conversion to compatible CMYK/nCLR Lab output profiles. Preserves device channel indexing and CLUT values, validates all four intents with bounded native samples, preserves valid clrt/clro bytes and reports omissions. v2 colorant metadata may be extensions; RIP compatibility and complete ICC conformance are not guaranteed. Build and real profile tests pending.

Beta.24 adds lossless structural mAB/mBA parsing and channel permutation for supported unequal-grid v4 printer LUTs with sampled, gamma or parametric device curves and PCS-side matrix/M stages. Original curve records, CLUT precision and untouched stage bytes are preserved. Inspector displays stage structure. Sampled device-side reverse curves support conservative ink bounds, including complex PCS-side stages. Analytic post-curve bounds and conversion/editing of complex stages remain limited. CI pending.

Beta.25 adds analytic evaluation of supported normalized ICC parametric curves (types 0–4) and gamma curveType, interval extrema and nondecreasing checks. Reverse ink bounds no longer require sampling analytic output curves. Supported mBA gamma edits preserve the CLUT/PCS bytes and untouched output records, reject unsafe or unrepresentable edits, and independently compare native reverse pipeline samples. Version conversion of complex LUTs is not enabled. CI and production tests pending.

Beta.26 adds the first vendor-neutral Characterization Data Explorer: CxF3/ZXML original CMYK targets, matched reflectance records with physical coordinates and wavelength metadata, paged ICC-predicted swatches, selected-patch channel and spectral details, generic CMYK/nCLR virtual target and CSV export. Embedded CGATS/targ, measured spectral-to-Lab conversion, measured ΔE, TIFF and full multichannel profile building remain pending. Preview is not a measured color. CI pending.

Beta.26.1 fixes the CI #65 malformed mAB fixture and missing optimizer cost format argument. Characterization supports single-table text targ/CGATS/CTI3 import with explicit device columns, spectral D50/2° estimates on specified 10 nm grids, separate relative/absolute ICC predictions and opt-in absolute characterization fit (ΔE00). Spectral method uses legacy ITU-T T.42 Table I.1 weights; it is not ISO 13655:2017 certification, nor a conversion from M0 to M1. Unsupported conditions remain unavailable rather than guessed. CI unconfirmed.

Beta.26.2 extends Characterization Data Explorer with channel/TAC/error filters, fixed-category Delta E00 heatmap, paired absolute Lab point clouds, measured CGATS export and guarded untagged CMYK16 page TIFF export. The point cloud is not a full gamut surface. TIFF geometry is newly defined (8 mm patches, 2 mm gaps); ink recipes are quantized to 16-bit, never silently clipped to limits. Native nCLR TIFF export remains pending. CI pending.

Beta.26.3 adds direct CIE 1 nm integration using verified official observer/illuminant tables (D50/D65/A, 2°/10°). The installed build embeds the data, with attribution in CIE_DATA.md and About. Alternate viewing conditions are selected per patch; ICC fit remains D50/2°. Endpoint extension and lack of bandpass correction are explicit. Run `pwsh -File scripts/fetch-cie.ps1` for a complete local build. Legacy D50 estimator remains labelled fallback when tables are unavailable. CI pending.

Beta.26.4 adds untagged 16-bit separated TIFF output for CMYK and 2–15 nCLR channels. InkSet distinguishes CMYK from generic inks; generic channel names CH1..CHn preserve ordinal mapping without assuming identity. Streamed scanlines, full-page ink-cap/TAC validation, no overwrite and no implicit conversion. Reader/RIP compatibility must be verified; not a recovered physical chart size. Also improves CIE CSV parser diagnostics after CI #78 failed one reference-table test (all dataset checksums and GPUI build passed). New CI pending.

Beta.26.5 adds bounded CMYK/nCLR characterization target preparation: deterministic single-channel ramps, pair mixtures and distributed samples, multi-page untagged separated TIFF, exact 16-bit layout manifest and blank measurement CSV. Measured condition comparison supports 2–15 physical channels and rejects mismatched recipes. Requires an existing printer ICC to establish physical channel count/order and configured safe limits. Argyll colprof remains CMYK-only in Studio; no N-color fitting claimed. CI is now requested once per batch using the ci:run PR label (remove/reapply to rerun), or manual workflow_dispatch; no automatic synchronize/push builds.

Beta.27 refines the native GPUI workspace: charcoal surfaces, compact stacked profile controls, mirrored sidebar/preview for Persian and Arabic, responsive expanded gamut below 1100 px, calibrated slider endpoints and single-step controls where previously absent, continuous finite-sample line rendering, localized characterization/profiling labels and status indicator. No ICC processing behavior changed. Interactive Windows and RTL rendering still require visual QA; CI checks compilation and pure UI geometry tests.

Beta.27.1 fixes GPUI precise step button ID typing and consolidates CGATS export: device-only targets remain unmeasured, homogeneous measured data uses round-trip precision, with atomic no-clobber saving and a bounded table. Windows build pending.

Beta.28 stabilizes ICC parsing and saving, CGATS metadata validation, locked builds and ZIP verification (version, AMD64 PE, SHA256, build provenance). CI uploads a single ZIP for seven days. See RELEASE_QA.md for outstanding Windows/RTL, RIP, actual measurement, licensing and update checks. No stable release claimed.

Beta.28.1 reorganizes Profiling Lab into four task tabs, loads embedded patches automatically or shows explicitly predicted virtual patches, preserves the active page across reference changes, retains the candidate when switching reference on Comparison, and adds aligned side-by-side / overlay comparison plots and compact optimizer cards. Comparison curves use blue reference and orange candidate (different stroke widths); physical channel identity is never inferred. Shared optimizer weights and tolerance remain editable in both tabs. CI #89 failed before compilation because Cargo.lock was absent; the same-job lockfile preparation fixes this and packages the generated lockfile with its checksum. Dependency resolution is not pinned across builds until Cargo.lock is committed. Interactive UI and Windows validation remain pending.

## Beta.31 — capability-aware multichannel preview

Shared custom CMYK/nCLR rebuilding and bounded correction paths, structural/native capability report in Inspector, incompatible workspace warnings, and optional native single-ink swatches when embedded colorant coordinates are unavailable. Swatches are predicted, not physical measurements. ICC v4 complex mBA reverse stages can be sampled for bounded correction when device bounds and native pipelines are supported; output reverse tables are resampled, not lossless. Unsupported mpet and alternate D2B/B2D editing remain rejected. Real-profile QA and Windows CI pending. See [MULTICHANNEL_QA.md](MULTICHANNEL_QA.md).

Beta.32 adds persisted synthetic multichannel rebuild regressions, complex reverse LUT smoothing tests, consistent channel swatch provenance, recipe capability preflight and a subset-selectable real profile validation harness. See MULTICHANNEL_QA.md. CI and external profile validation pending.

Beta.33 adds active-channel-only Custom rebuilding, cooperative cancellation before ICC publication, reuse of verified rebuild results, bounded native roundtrip scratch memory and selectable Fast / Balanced / High gamut sampling (9/17/33). Default gamut resolution remains 33. No measured speedup or Windows interactive acceptance is claimed until benchmarks and manual QA are run.

Beta.34 extends bounded multipoint curves, selective color correction and LUT smoothing to compatible ICC v2 Lab LUT16 (mft2) output profiles. It retains the source version, independent reverse intents, A2B and channel identity; v2 black generation/whole-profile optimization, LUT8 and alternate pipelines remain unsupported. See MULTICHANNEL_QA.md for the FOGRA55 diagnostic and opt-in Rust test.

Beta.35 introduces an experimental N-color forward-model evaluation in Profiling Lab / Patch explorer: compare nearest-neighbor and eight-neighbor inverse-distance Device→Lab predictions on held-out recipe groups. It does not build or export ICC profiles. Use declared, homogeneous D50/2° Lab measurements with explicit M0–M3; predicted ICC and derived spectral Lab are excluded. See NCOLOR_MODEL_QA.md.

Beta.36 extends N-color evaluation with a local linear ridge baseline (32 neighbors), pooled five-fold grouped validation, active-ink coverage, and optional >=5-active-ink stress holdout. Not an ICC generator. See NCOLOR_MODEL_QA.md.

Beta.37 adds experimental measurement-based N-color inverse recipe search and forward/adjacent inverse continuity diagnostics in Profiling Lab / Patch explorer. It uses quantized ink caps/TAC and a coverage heuristic, reports model-only tolerance, and compares hard/tapered ridge without replacing the existing inverse model. It does not create an ICC. See NCOLOR_MODEL_QA.md.

Beta.38 adds experimental continuity-regularized inverse ramp comparison in Profiling Lab / Patch explorer. It penalizes neighboring ink differences while enforcing existing TAC/caps, coverage distance and per-point predicted color tolerance. Adjustable continuity weight, independent baseline vs refined ink bars, objective and error diagnostics. No ICC generation. See NCOLOR_MODEL_QA.md.

Beta.39 adds experimental 3×3×3 local Lab lattice audit with per-axis spans, independent and neighbor-regularized recipes, per-axis ink jumps, incomplete-node reporting, and cell-center model checks. No ICC is generated. See NCOLOR_MODEL_QA.md.

Beta.40 adds an experimental forward CLUT audit in Profiling Lab / Patch explorer: independent per-ink grid sizes, checked raw Lab16 payload estimate, grouped fold-0 holdout, direct vs multilinear interpolation errors on common samples, and explicit missing-corner coverage. The 8,192-node diagnostic budget does not support 14–15 dimensions even at two points per axis. No ICC generated. See NCOLOR_MODEL_QA.md.

## Active development tracker

See [ROADMAP.md](ROADMAP.md), [RELEASE_QA.md](RELEASE_QA.md) and [GitHub Issues](https://github.com/emadgh/color-convert-rust/issues) for tracked P0/P1/P2 work. Priority is editing/analysis reliability, full Persian/Arabic RTL, complete diagnostics localization, controls, cancellable progress, updater and real Windows/profile acceptance. Measurement-based N-color ICC construction is an experimental workstream, not yet complete. Gamut intent controls (shared or separate Reference/Candidate) use requested native round-trip rendering intents and matched targets; cross-intent coverage is not physical gamut. CI/manual verification required.

Beta.41 consolidates the UI stabilization work: shared/separate gamut rendering intents, partial RTL and localized diagnostics, direct numeric keyboard entry and cooperative read-only analysis cancellation with phase progress. Existing file-publication safety remains separate; manual Windows/RTL/updater and real-profile acceptance are pending. See active roadmap and release QA.

Beta.42 adds cancellable gamut mesh extraction and per-phase progress, dynamic localized status headings, explicit gamut intent preference persistence, numeric validation feedback and regression tests. Manual Windows and real-profile acceptance remain pending.

Beta.43 adds validated numeric clipboard input and a retry action for failed update downloads. Windows interaction and live updater acceptance remain pending. See RELEASE_QA.md.

Beta.44 enables predictive whole-profile ink optimization and physical-black preference for compatible ICC v2 LUT16 Lab profiles, with corrected PCS encoding and strict native sampled color acceptance across all four intents. See LEGACY_PREDICTIVE_QA.md. Advanced LUT8/mpet and true channel-count reduction remain unsupported.

Beta.45 reduces temporary gamut raster allocations and repeated UI computation, caches bounded wire geometry, shares immutable characterization snapshots, prunes exact neighbor distance scans and polishes compact/RTL navigation. See PERFORMANCE_QA.md; actual Windows frame-time/peak-RAM and final UI acceptance remain pending.

Beta.46 reduces redundant clipped/occluded wireframe interpolation, uses exact fast paths for fully opaque/transparent blending, and avoids the empty-scene compositing pass. Regression tests compare depth/color and integer blending against the previous implementation. CI and actual Windows frame-time validation pending.
