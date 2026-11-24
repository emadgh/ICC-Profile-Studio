# ICC Profile Studio — active roadmap

## Product scope (beta.11 owner decision)

Lean, vendor-independent GPUI application for CMYK and multichannel output ICCs and print DeviceLinks. Focus on profile color editing, validation and analysis. No library/catalog, projects/session recovery, dedicated tag manager, standalone metadata administration, image editing/conversion, RGB/Gray/input/display/abstract editing, vendor-specific features or egui. Simple rename in Inspector and read-only Windows installed profile selection remain. Keep unknown ICC tag payloads safe where supported; internal parsing is not a user-facing tag management tool.

## Implemented source

- Inspector and Validator, stored media white point, native LittleCMS checks, simple profile rename.
- Rebuilder for supported CMYK and selected four-channel structures, strict channel/TAC bounds.
- Tone gamma and bounded multipoint curves, channel reorder, multi-region color correction, LUT smoothing, predictive black generation and ink/cost optimization.
- Profile Comparison, Gradient Analyzer, Gamut R5 (BGRA-only, 2D outline/opacity, 3D quad-style wire, profile colors, floor footprint, shared scale, resizable pane and expanded in-window view).
- Restricted structural ICC v2.4/v4.3 conversion, PCS encoding adaptation and four-intent equivalence checks; explicit Relative/Absolute compatibility mode.
- Print DeviceLink generation; characterization chart, measured CSV comparison and measured TI3 profiling.
- System profile picker next to disk selection in reference/comparison/gamut, local display preferences and updater.

## Remaining work (priority order)

1. Broaden supported printer LUT structures: sampled/parametric curves, mft1/mft2, mAB/mBA, varying CLUT axes and supported mpet elements. Reject unsupported transformations explicitly, preserve unknown tags safely. Test native interpolation and round-trip accuracy.
2. Multichannel output profiles: dynamic channel count and names, 2–15 channels where supported by ICC and the underlying engine; generalize device sampling, bounds, TAC, curves, comparison, gamut, rebuilding and constrained correction in stages. Avoid treating arbitrary inks as CMYK or assuming a black channel.
3. Advanced print DeviceLink generation: black preservation where black is defined, multichannel links where supported, numerical validation.
4. Validate nonconvex volume estimate convergence, rendering performance and Windows interactive checks.
5. Measured profile correction and general characterization using real matched measurements. No brand or industry-specific workflow modules.
6. Streamline controls, drag/drop ICC, improve responsiveness, update feed integrity, release packaging and license/checksum review.
7. Release candidate: broad real-profile regressions, interactive Windows smoke tests, physical validation where needed and documentation. No unsupported accuracy claims.

## Build and history

- beta.10 commit 2707bbe3f0572a5a8d3654cee9c5b5567ede2c67: Windows GPUI success, 75 backend tests passed (run 45472945269).
- beta.11: remove library, project/session and tag manager code and UI, retire standalone Dashboard and merge simple rename into Inspector. Remove direct sha2 dependency used by projects; package generic characterization documentation instead of ceramic-specific guide. Existing user project/checkpoint/library files are not touched; old preference keys are ignored. CI pending.
- Previous tracker and retired modules recoverable in Git history at https://github.com/emadgh/color-convert-rust/tree/2707bbe3f0572a5a8d3654cee9c5b5567ede2c67.
- Current computational support is primarily four-channel. CI results do not guarantee real-profile color equivalence, physical print accuracy or visual interaction.

## beta.12 — neutral UI and high-quality gamut
- Grouped collapsible sidebar (Inspection, Editing, Separation, Analysis, Application), monochrome navigation symbols, charcoal UI and orange accent, by emadghasemi.ir and reorganized About with links/technology/version/update section.
- Five-language UI dictionary: English, Persian, Italian, Arabic, Russian, persisted language; translated navigation, headings and mapped controls, English fallback for unmapped descriptions and engine diagnostics. RTL navigation order only; complete RTL layout and translation coverage pending.
- Removed convex hull renderer, report and dependency. Fixed high quality 33³ target grid / 57³ adaptive field. Approximate nonconvex Lab³ volume via cell-corner occupancy at ΔE00 ≤ 2, not exact mesh or physical volume. Test sphere approximation; convergence and runtime verification pending.
- beta.11 Windows GPUI CI passed (45476241161); beta.12 CI pending. Next: parametric curves and broader LUT structures, dynamic multichannel.

- Expanded locale tables to additional editing/analysis labels and descriptions; detailed diagnostics and some dynamic strings remain English. Navigation and key action icons use lightweight monochrome glyphs; no icon-font dependency.

## beta.13 — native LUT support and multichannel analysis
- beta.12 Windows GPUI CI successful (45477839103), 72 tests.
- Custom CMYK/4CLR Rebuilder evaluates native A2B1 pipeline (LCMS) without re-encoding it. Supports valid parametric curves, single-gamma curves, nonuniform CLUT axes and supported mAB stages in native forward pipeline; v2 mft1/mft2 input. Rebuilds B2A as bounded identity-curve tables. Original A2B payloads remain unchanged. Existing strict parser still rejects unsupported reverse curve/axis structures; standalone Tone/Reorder/Version editors not generalized. Regression tests cover parametric forward and nonuniform axes. Native interpolation is used rather than custom multilinear forward prediction.
- Dynamic native 16-bit CMYK/nCLR format mapping (2–15 channels), gamut roundtrip for supported printer profiles and four-intent inspection with dynamic ink maxima, channel names, simple equal-grid sampled-LUT strict mathematical bounds. Multichannel bounds use conservative global envelope for nonlinear curves; no guessed black channel. Tests exercise 6CLR native transforms, zero channel, bounds and gamut. Other counts mapped but not transform-tested individually; LCMS can reject unsupported structures. Comparison/gradient/editing and multichannel Rebuilder remain pending. Full i18n/RTL and Windows runtime checks pending.
- References: https://www.color.org/icc-1_specification/ and https://docs.rs/lcms2/latest/lcms2/struct.PixelFormat.html. CI pending.

## beta.14 — channel visualization and RTL display
- beta.13 Windows GPUI success, 77 tests (45482641438).
- Header switches between embedded clrt colorant names and standard CMYK / numbered nCLR labels. Display-only, no channel mapping or ICC changes. Valid v4 output clrt PCS values supply approximate sRGB swatches; fallback standard CMYK or neutral nCLR. v2 swatch decoding deliberately not assumed.
- Rebuilder live channel bars, closed/open states, K presets and TAC combined limit visualization. Requested caps, not simulated separations. Reorder before-save color palette, Tone Curves swatches, selective correction illustrative Lab target comparison and recipe result bars. Center content capped at 900 px. Gradient step and lightness line plots with aligned before/after scales, every sample retained; comparison histogram remains appropriate for ΔE distribution.
- Reviewed MIT dosu-core (RustNegar), terminal-specific UAX #9 visual reordering, no contextual shaping. Use arabic_reshaper and unicode-bidi directly for display-only labels, Persian/Arabic shaping + visual ordering, not whole terminal engine. GPUI still wraps preordered strings; full per-line layout, editable bidirectional text, diacritics and real Windows rendering require further verification. English fallback remains for untranslated diagnostics. No metadata or file path mutation. CI pending.

- Refined Rebuilder into two-column channel cards with compact wrapped controls and K presets 0/5/10/25/50/100. Inspector palette includes nCLR names. Comparison explicitly labels by reference index, not assumed identical candidate order. Windows CI pending.

## beta.15 — multichannel constrained Rebuilder
- Source: independent 2–15 channel nCLR output inversion (four-channel nCLR retains existing four-channel path in UI); native A2B1 + B2A1 seed, bounded deterministic seed pool, coordinate and pair-transfer search, no exponential Cartesian seed grid, no assumed K. All three reverse LUTs use rebuilt relative prediction; source A2B preserved. Per-channel/TAC proof with identity post-curves, native 17³ samples per intent and exact zero checks. v2 mft1/mft2 and v4 mAB forward; reject alternate D2B/B2D and unsupported pipelines. UI dynamic channel cards, TAC scale up to N×100%, async generation and comparison candidate. Tests six-channel and zero-ink three-channel fixtures; CI pending. Real high-channel profiles, color drift and runtime remain unverified; not a guarantee of color equivalence. CMYK engine unchanged. Other editing modules not generalized.
