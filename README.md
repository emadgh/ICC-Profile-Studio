# ICC Profile Rebuilder (Rust)

Portable Windows x64 utility for making a new four-channel printer ICC/ICM profile with limited ink channels, without editing the source image or overwriting the reference profile.

## Run

1. Download the **ICC-Profile-Rebuilder-Windows-x64** ZIP artifact from a successful Windows x64 build in [Actions](https://github.com/emadgh/color-convert-rust/actions/workflows/windows.yml). Sign in with repository access.
2. Extract it to a writable folder and launch ICC-Profile-Rebuilder.exe.
3. Open or drag a printer ICC/ICM into the window. Verify the physical channel order. The program reads the ICC header and the colorant table (clrt) when available; CMYK fallback names are not a guarantee of physical ceramic ink names.
4. Disable channels or enter their maximum percentage. K presets: 0, 5, 10, 100%. The edited names are display labels, not channel reordering.
5. Select an engine and a new output filename, then Generate and verify ICC. An existing file is never overwritten.

### Engines

- **ArgyllCMS revfix**: ICC v2 standard CMYK, independent K limit and optional total ink limit. Downloads the pinned official Windows x64 ArgyllCMS 3.5.0 ZIP on first use, extracts executables and DLLs into argyll-3.5.0 beside the application. No administrator rights needed; the folder must be writable. HTTPS is used; the upstream SHA256 was not independently verified. An optional trusted SHA256 can be supplied via environment variable ARGYLL_SHA256 for verification before extraction. For offline use, extract the official ZIP into argyll-3.5.0 manually.
- **Custom constrained LUT** (experimental): four-channel CMYK/4CLR, ICC v2 Lab PCS with A2B1 mft2 and identity matrix, or ICC v4 Lab PCS with A2B1 mAB, equal-sized CLUT and sampled curv curves (no matrix/M curves). Independently constrains all four channels and total ink, using coarse sampling and bounded local optimization of the reference forward LUT. Output B2A LUTs are newly generated rather than clipping old separations. Uses approximate Delta E76 optimization and multilinear forward interpolation; not identical to an industrial profiling engine.
- Both rebuild B2A0, B2A1, B2A2 using relative-colorimetric inversion, not true perceptual or saturation rendering. Alternate B2D transforms are rejected. Output validation checks conservative device-channel maxima over all three supported LUTs (including output curves); unsupported LUT structures fail closed. In Argyll mode the TAC is delegated to revfix, not independently certified by the validator. In custom mode the TAC is constrained at CLUT nodes and identity output curves preserve that bound under convex interpolation. Calibration and the printer/RIP can change final physical ink application.
- The default K cap for CMYK is 0%; custom-mode initial TAC is estimated from the original B2A1 nodes after output curves, or 300% if unsupported. This is a starting point, not a certified ink limit or recommended ceramic calibration. Use the actual safe TAC. Print charts and measure after firing before production. Real Durst ICC v4.3 structure inspected; actual printed output remains untested.

## Build

Install stable Rust and Visual Studio Build Tools with Desktop development with C++ on Windows, then:

    cargo test --release
    cargo build --release

Output: target/release/color-convert-rust.exe

GitHub Actions builds the Windows x64 EXE and uploads the portable artifact with its SHA256 after tests.

## Sources

- https://www.argyllcms.com/doc/revfix.html
- https://www.argyllcms.com/downloadwin.html

ArgyllCMS is a separately downloaded third-party program subject to its own license. No Argyll binaries are bundled in this repository.

## About and updates (v0.2.0)

About / Updates contains the version, icon, project links, and the shared emadgh/update-via-github updater (pinned git revision). Manual update checks, download progress, required SHA-256 verification, and restart/install are implemented. Installation is disabled during ICC generation. The current repository is private, and the upstream updater has no GitHub authentication; a 404 is shown as no accessible release, not confirmed up to date. Use signed-in GitHub Releases for manual downloads. Do not embed a personal access token in the application. A public update feed or authenticated upstream updater is necessary for automatic private-repository updates.

Successful main builds publish a new version from Cargo.toml if that version has not been released; matching version tags also trigger publication. Existing releases are never overwritten. This publishes the EXE, matching .sha256 and portable ZIP to Releases after tests. Artifact uploads remain best-effort because GitHub Actions storage quota may be exhausted. Releases preserve the repository's access controls. The app icon is embedded into the EXE and displayed in the About dialog. Custom LUT generation displays slice progress; external revfix and dependency download show an indeterminate indicator rather than fabricated percentages.

## v0.2.1 — ICC v4 support

ICC v4.3 mAB/mBA profiles are automatically routed to the custom engine. Input and output curve sets are parsed, the original v4 header and four-channel color space are retained, and all three B2A tags are regenerated as mBA with identity curves. Matrix/M and parametric curves fail closed. Default grid for v4 is 33. No header downgrading or post-conversion K clipping. Gamut metadata gbd0–gbd3 is removed with stale gamut tags.

## ICC Profile Studio migration

See [ROADMAP.md](ROADMAP.md) for completed work, remaining milestones and the dependency-first architecture. Experimental GPUI frontend (not the default release yet): `cargo run --features studio --bin icc-profile-studio`. The stable egui executable remains the default while the new frontend is validated. LittleCMS 2 is statically linked through the lcms2 Rust crate for inspection and transform smoke tests; ArgyllCMS remains available for supported revfix operations.

### Comparison preview

Select a reference and candidate CMYK printer ICC in GPUI Studio. LittleCMS predicts both separations and their D50 media-relative Lab round trips for a uniform sRGB grid. The report uses native CIEDE2000 with unit weights, sampled ink means/maxima and 33 neutral samples. This is not a measured print result or a gamut-volume metric.

### Editing preview

GPUI Studio now includes per-ink gamma curves, physical channel reordering and metadata renaming. The gamma editor modifies output separations rather than recalibrating the printer; previous TAC/caps must be revalidated. Channel reorder permutes both transform directions and rejects unknown vendor/calibration tags rather than risking corruption. Metadata rename uses the new file's stem as its internal description. All edits write a new file after LittleCMS opening and CMYK transform creation. These features are experimental pending CI and printer validation.

### Persistent Gamut Preview

The GPUI right pane is collapsible, available on every page and accepts a separate candidate ICC. Newly saved edits/rebuilds become the candidate. Use Before/After independently, 2D a*/b* projection or L* ±5 slice, or a rotatable 3D orthographic Lab point cloud with zoom. LittleCMS samples both effective separation transforms on the same grid of D50 Lab targets represented through XYZ, then plots round-trip Lab. The result is not an exact physical gamut boundary, a volume estimate, or a measured print test. Controls and build remain experimental pending Windows CI.

### Gradient and gamut analysis

Gradient Analyzer runs 13 sRGB ramps with 256 samples per ramp, using LCMS predicted Lab and CIEDE2000. Gamut Viewer compares shared valid D50 target errors at ΔE00 thresholds 1/2/3/5. Parry3D computes a convex hull of the sampled points for an optional wireframe and convex-envelope volume. The envelope may include unattainable colors; it is not the exact gamut or a measured result. New analytics are pending Windows CI.

### Selective Color Correction / LUT Smoothing

Initial support is deliberately restricted to ICC v4 CMYK / Lab printer profiles with three supported mBA B2A tags. LittleCMS evaluates the original pipelines independently. Selective hue correction uses a smooth circular Lab/LCh mask, protecting near-neutral and extreme lightness targets. Smoothing averages six neighbors in the 3D separation lattice using convex weights. Both resample to a chosen 17/33/49/65 grid, preserve the A2B model, enforce original channel bounds and an explicit TAC cap (default 400% adds no extra TAC reduction), then verify the generated identity-curve CLUT. Resampling and gamut mapping may affect unselected colors. Use the reference TAC estimate only as a starting point, compare both output profiles, and print-test before production. New code remains pending CI.

### Image Processing Studio / DeviceLink preview

Image conversion uses LittleCMS with image and tiff crates. Output is real 16-bit interleaved CMYK TIFF with embedded destination ICC and chosen DPI, plus reduced-size proof/separation/coverage PNGs and a text report. Reference ICC is the before simulation; selected candidate is destination (reference if none). Input is limited to 24 MP; untagged images rejected unless an explicit assumption is enabled. TIFF metadata other than the output ICC and DPI is not preserved. DeviceLink is generated using LCMS, with no guarantee of black preservation or strict device bounds. New functionality is pending CI.

### Ceramic Lab and recipe search

The Ink Optimizer searches for an individual device recipe using the existing ICC model and user-defined relative weights, caps, TAC and ΔE00 tolerance; it is not a guaranteed global optimum or whole-profile ink saving. Ceramic Lab exports direct-device TIFF charts and blank measurement templates, compares actual D50 Lab measurements on identical patches, and invokes official Argyll colprof on supplied CMYK TI3 data. Print with device values preserved, measure after firing and validate profiles. No physical compensation is inferred without data.
