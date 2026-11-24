# Ceramic workflow and remaining validation

## Machine matching / glaze / firing / ink batch / shade

1. Record printer, physical ink order, glaze, ink batch, kiln cycle and measurement instrument/geometry. Use the same D50 measurement condition.
2. Set verified physical ink caps and TAC in Rebuilder. Generate a direct-device test chart from Ceramic Lab. Print with no additional color conversion, then measure after firing.
3. Fill the measured CSVs and compare identical patch IDs/recipes in Ceramic Lab. This quantifies real differences; it does not by itself correct the ICC.
4. Acquire a real Argyll CMYK TI3 chart dataset for the changed condition (prefer a sufficiently dense, validated profiling chart). Use colprof in Ceramic Lab to build a new destination profile. Do not fabricate TI3 from unmeasured data.
5. Load the original reference and new measured destination. Compare gamut, Delta E and gradients. Use Image Converter or DeviceLink to separate the original design into the changed printing condition. Verify physical channel order, limits, print and firing. The link is not a guarantee of exact color matching.
6. For emergency failure of a channel, use Rebuilder with strict caps and a verified TAC, then compare and print-test. Use Selective Correction/Smoothing only as deliberate edits.

## Remaining release gates

- CI for all new modules and Windows interactive smoke test. Current source milestones are not release claims.
- Actual Durst profile and image regression (keep production data outside GitHub), verify channel order, color drift, physical dimensions, source/output ICC embedding, and high-resolution industrial workflows.
- Full streaming/BigTIFF, calibrated monitor soft proof, direct instrument integration, measured compensation validation, non-convex gamut measurement, richer controls and accessibility.
- Private authenticated update feed or public release assets by explicit owner choice. Never embed a personal token. GPUI artifacts are not Release assets.
- Audit dependency and Argyll redistribution licenses and official download checksum before a public release.

No print or measurement results are fabricated. All in-app color predictions are model-based unless explicitly comparing imported measured CSV values.
