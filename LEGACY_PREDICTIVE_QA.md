# Beta.44 — legacy predictive LUT16 QA

## Native independent diagnostic

Operator-provided Ref_FOGRA55.zip, ICC v2.4 / 7CLR / Lab, CMYKOGV. Original ICC B2A grids 33/33/17; A2B1 LUT16. No reference ICC redistributed. Independent C + Python with system LittleCMS; not the compiled Rust binary.

264 recipes (zero, 7 single-ink recipes, 256 deterministic random quantized recipes) compared direct native A2B1 pipeline outputs to native device→Lab4 relative transform without optimization. With nominal factor 65280/65535, maximum absolute differences by Lab component: 0.00000294, 0.00000760, 0.00000760. Without correction: 0.389105, 0.799735, 0.884593. White native L=100, a approximately zero, b=-0.003899. Extended v2 PCS endpoints remain representable.

Independent uniform-cost bounded optimization (all weights 1, caps 100%, TAC 400%, requested tolerance 2, node tolerance 1). C implementation mirrors direct ink reduction branch of Rust algorithm, raw native B2A pipeline evaluation, floor quantization; serialized three identity-curve mft2 tables with A2B tags unchanged. Native reopened profiles; 729 RGB samples + 33 neutrals. Native relative Lab4 appearance for both profiles. Actual Rust uses its existing XYZ→Lab D50 comparison contract; numerical results below are independent diagnostics, not exact compiled Rust output.

| Intent | original grid | mean DE00 | P95 | max | mean TAC before | mean TAC after | peak sampled TAC after |
|---|---:|---:|---:|---:|---:|---:|---:|
| Perceptual | 33 | .91161 | .99690 | 1.19832 | 129.42043 | 123.37563 | 276.17609 |
| Relative | 33 | .91032 | 1.04395 | 1.20332 | 138.92044 | 131.76070 | 298.99290 |
| Saturation | 17 | .91702 | 1.04693 | 1.29897 | 131.33678 | 125.12718 | 273.95132 |
| Absolute | derived | .90988 | 1.04650 | 1.38366 | 134.81663 | 127.83456 | 295.58862 |

All four sampled maxima below 2 in this independent experiment. Uniform 17 grid: perceptual 2.95666, relative 3.47141, saturation 1.29897, absolute 3.37602. Uniform 33 grid: saturation 4.29242 (other intents below 2). Thus preserve existing 17-point intent grid when 33 selected; do not treat oversampling as monotonically improving accuracy. Raw output node TAC maxima for mixed grid: 324.42817%, 322.65965%, 305.30404%; nominal TAC 400% respected. Sampled TAC values are not global maxima. All data predicted using same A2B model, not measured print output. Full GCR/UCR behavior not independently tested on FOGRA55; synthetic legacy tests exercise black preference. Real Rust test opt-in via ICC_FOGRA55_PATH. Unverified source caps, malformed PCS, infeasible baseline or sampled tolerance failure must prevent publication. User-provided cap lower than existing separation requires a rebuild first. This does not reduce channel count, fit new ICC or validate RIP compatibility.

## Remaining
Exact-head Windows CI, real Rust FOGRA55 opt-in, other legacy corpus, RIP validation, physical measurements. #32 dimensional reduction next.
