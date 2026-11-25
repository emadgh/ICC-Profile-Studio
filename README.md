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
