# ICC Profile Studio

Portable Windows x64 **GPUI-only** application for inspecting, editing, validating, comparing and building ICC/ICM color profiles, with a focus on four-channel ceramic printing.

## Current scope

No image import/conversion/editing, image soft proof, batch conversion or image separation tools. Previous image-processing code and egui app are retained only in Git history at 32e6b7899b42e8cbe3a0942795aa0cc02a3de19f. Profile-level DeviceLink generation remains; applying it to images must be done externally. ICC characterization chart TIFF and gamut PNG exports remain because they support profile work.

## Run

Download the Studio artifact from [GitHub Actions](https://github.com/emadgh/color-convert-rust/actions/workflows/studio.yml) or a compatible Studio prerelease from [Releases](https://github.com/emadgh/color-convert-rust/releases). Extract to a writable folder and launch ICC-Profile-Studio.exe. Verify the SHA256. Open an ICC/ICM and verify physical ink channel order; logical CMYK labels are not necessarily ceramic ink names. Choose limits and save a new profile. Compare gamut, gradients, separations and predicted Delta E before print testing. Source files are never overwritten.

## Tools

- Profile Inspector and Validator, stored media white point (wtpt) with derived D50 Lab and approximate sRGB swatch.
- Profile Rebuilder, per-channel/TAC limits, strict LUT and native LittleCMS validation.
- Comparison, neutral-axis plots, Gradient Analyzer, sampled gamut coverage and CSV reports.
- Tone curves, channel reorder, metadata, selective LCh correction, LUT smoothing, black generation and ink/cost optimization.
- Collapsible 2D/3D Gamut R5: colorized solid reference, wireframe candidate, BGRA only, shared Lab grid, floor contours, optional convex hull, nonconvex sampled surface. Surface is a Delta E00 <= 2 round-trip estimate, not a measured physical boundary. Display colors are approximate sRGB.
- ICC DeviceLink generator with sampled validation. No direct image application.
- Profile Library and Ceramic Profiling Lab: generate direct-device characterization charts, import measured CSV/TI3, compare measured patches and build profiles with Argyll colprof. No instrument connection.
- Settings and About/Updates. Local preferences retain preview/grid/chart DPI and library directory, not device-specific limits or jobs. Older image settings are ignored.

## Engines and limitations

LittleCMS 2 is linked statically. Custom constrained LUT rebuild supports selected ICC v2 Lab mft2 and ICC v4 Lab mAB/mBA profiles with equal grids and supported sampled curves; unsupported matrices, parametric curves or alternate B2D transforms can be rejected. It regenerates B2A intents using relative-colorimetric inversion, not true perceptual/saturation rendering. Channel/TAC certification is mathematical only for supported structures; native samples are independent checks, not physical printer limits. Channel reorder rejects unknown channel-sensitive tags. Use verified ink limits, then print and measure after firing. Profile editing is not a substitute for characterization.

ArgyllCMS revfix is available for supported v2 CMYK, and colprof builds from measured TI3. Official Windows Argyll 3.5.0 is downloaded when needed into a writable app folder; a trusted optional ARGYLL_SHA256 can be supplied. Upstream checksum not independently verified; no Argyll binaries bundled. See https://www.argyllcms.com/doc/revfix.html and https://www.argyllcms.com/downloadwin.html.

## Build

Stable Rust and Visual Studio Build Tools (Desktop development with C++) on Windows:

    cargo test --lib --no-default-features --release
    cargo build --release
    cargo run --release

Default and only executable: target/release/icc-profile-studio.exe. The optional studio feature is enabled by default; disabling default features is for headless library tests only. The single Windows Studio workflow tests and packages the executable, SHA256 and documentation. STUDIO_VERSION tracks prerelease version; Cargo package version remains the historical core version. Preview publishing remains manual and does not overwrite releases. Older Rebuilder releases are historical, not actively built. The shared updater's latest-release/private-feed limitations remain; use compatible Studio release assets, not Rebuilder EXEs.

See [ROADMAP.md](ROADMAP.md) for the active scope and remaining work, and [CERAMIC_WORKFLOW.md](CERAMIC_WORKFLOW.md) for characterization. CI does not establish print accuracy or interactive Windows runtime quality.

R5 adds a resizable preview, expanded in-window view, compact controls, quad-style surface section grid, 2D outline/fill opacity, one projected footprint per visible profile and reusable two-column draggable curve editors. The wire grid is not an all-quad remeshing of the solid triangles.
