# ICC Profile Studio

Vendor-independent, portable Windows x64 application for ICC/ICM inspection, color analysis, editing and construction. GPUI only. Scope: CMYK and multichannel printer output profiles and print DeviceLinks. Advanced tools currently primarily support four-channel CMYK/Lab LUTs. Broader multichannel support is planned.

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
