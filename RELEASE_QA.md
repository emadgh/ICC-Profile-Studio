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
