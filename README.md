# ICC Profile Rebuilder (Rust)

Portable Windows x64 utility for making a new four-channel printer ICC/ICM profile with limited ink channels, without editing the source image or overwriting the reference profile.

## Run

1. Download the **ICC-Profile-Rebuilder-Windows-x64** ZIP artifact from a successful Windows x64 build in [Actions](https://github.com/emadgh/color-convert-rust/actions/workflows/windows.yml). Sign in with repository access.
2. Extract it to a writable folder and launch ICC-Profile-Rebuilder.exe.
3. Open or drag a printer ICC/ICM into the window. Verify the physical channel order. The program reads the ICC header and the colorant table (clrt) when available; CMYK fallback names are not a guarantee of physical ceramic ink names.
4. Disable channels or enter their maximum percentage. K presets: 0, 5, 10, 100%. The edited names are display labels, not channel reordering.
5. Select an engine and a new output filename, then Generate and verify ICC. An existing file is never overwritten.

### Engines

- **ArgyllCMS revfix**: standard CMYK, independent K limit and optional total ink limit. Downloads the pinned official Windows x64 ArgyllCMS 3.5.0 ZIP on first use, extracts executables and DLLs into argyll-3.5.0 beside the application. No administrator rights needed; the folder must be writable. HTTPS is used; the upstream SHA256 was not independently verified. An optional trusted SHA256 can be supplied via environment variable ARGYLL_SHA256 for verification before extraction. For offline use, extract the official ZIP into argyll-3.5.0 manually.
- **Custom constrained LUT** (experimental): four-channel CMYK/4CLR, ICC v2/v4 Lab PCS with A2B1 mft2 and identity matrix. Independently constrains all four channels and total ink, using coarse sampling and bounded local optimization of the reference forward LUT. Output B2A LUTs are newly generated rather than clipping old separations. Uses approximate Delta E76 optimization and multilinear forward interpolation; not identical to an industrial profiling engine.
- Both rebuild B2A0, B2A1, B2A2 using relative-colorimetric inversion, not true perceptual or saturation rendering. Alternate B2D transforms are rejected. Output validation checks conservative device-channel maxima over all three supported LUTs (including output curves); unsupported LUT structures fail closed. In Argyll mode the TAC is delegated to revfix, not independently certified by the validator. In custom mode the TAC is constrained at CLUT nodes and identity output curves preserve that bound under convex interpolation. Calibration and the printer/RIP can change final physical ink application.
- The default K cap for CMYK is 0%; custom-mode initial TAC is 300% as a UI starting point, not a recommended ceramic calibration. Use the actual safe TAC. Print charts and measure after firing before production. No real Durst profile has been tested.

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
