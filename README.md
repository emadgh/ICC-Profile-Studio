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
