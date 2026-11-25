# Experimental N-color forward model

## Operator-supplied FOGRA55 reference dataset

Inspected the supplied Ref_FOGRA55.zip, reading FOGRA55.txt directly. ISO28178, PC7_1..PC7_7, CMYKOGV order explicitly named, declared D50 / 2° / M1. This is reference characterization data, not a new ceramic-print measurement or an independent validation chart. Files are not redistributed.

- 4,884 rows; 4,272 unique ink recipes, 612 repeat rows (identical supplied Lab to numerical precision).
- Ink recipes quantized at 0.0001 percentage point and sorted lexicographically. Average Lab within each group, split by FNV-1a hash of little-endian u32 recipe components modulo 5. Fold 0 yields 3,405 training groups and 867 holdout groups. Repeats cannot cross the split.
- Maximum recipe TAC in the dataset: 400%, not the 300% in the accompanying ICC filename. Do not use this as a production TAC instruction.
- Independent Python evaluation with integer squared distance, stable lexicographic tie order, sklearn/skimage-compatible CIEDE2000 (skimage), and eight-neighbor inverse squared distance (IDW8).

| Model | Mean ΔE00 | P95 ΔE00 | Max ΔE00 |
|---|---:|---:|---:|
| Nearest neighbor | 7.4200 | 15.8770 | 30.6614 |
| IDW8 | 3.3204 | 7.7928 | 14.2082 |

Nearest training recipe normalized Euclidean distance: median 0.30, P95 0.40, max 0.60. The reference data provides only one unique recipe with all seven inks nonzero; many recipes have three or four active inks. No coverage guarantee for arbitrary 7D recipes. Equal-weight distance ignores ink interactions and does not guarantee smoothness or spectral validity. A single grouped random-like fold is insufficient for generalization; structured holdouts and independent measured charts are needed. These numbers are independent Python diagnostics, NOT Rust test execution.

## Rust and UI

New ncolor_model::evaluate accepts declared homogeneous D50/2° Lab with M0/M1/M2/M3, ignores ICC predictions and derived spectral estimates. 2–15 inks, max 10,000 unique groups / 100,000 rows; fail closed on unsupported dimensions or data. Runs in background from Patch explorer, displays grouped counts, relative error bars and distance diagnostics. Does not generate, fit or export ICC LUTs. The current Argyll CMYK path remains unchanged.

Run the opt-in real reference regression locally:
```powershell
$env:ICC_FOGRA55_DATA = 'C:\private\icc-corpus\FOGRA55.txt'
cargo test --lib --no-default-features --release fogra55_ncolor_baseline -- --ignored --nocapture
```
Synthetic tests cover grouped counts, exclusion of predicted values, mixed-condition rejection and deterministic tie order. Windows CI and real Rust run pending at PR creation.

## beta.36: five-fold pooled evaluation and high-active-ink stress test
Reopened the operator-supplied FOGRA55.txt from Ref_FOGRA55.zip. Independent Python/NumPy reference, integer squared recipe distances with lexicographic tie ordering, skimage CIEDE2000, grouped FNV folds as before. Fixed local affine ridge: 32 neighbors, normalized device fraction, weights 1/(squared distance + 0.01) normalized to unit sum, slope penalty 0.002, unpenalized intercept, clamp predicted Lab to PCS range. Hyperparameters fixed for this experiment; this is not independent hyperparameter validation.

| Fold | Holdout recipes | NN mean | IDW8 mean | Local ridge mean |
|---|---:|---:|---:|---:|
| 0 | 867 | 7.4200 | 3.3204 | 1.2514 |
| 1 | 850 | 7.5984 | 3.2779 | 1.2389 |
| 2 | 808 | 7.3025 | 3.1405 | 1.1805 |
| 3 | 909 | 7.1351 | 3.1696 | 1.1301 |
| 4 | 838 | 7.4435 | 3.2936 | 1.2458 |

| Pooled 4,272 holdouts | Mean DE00 | P95 | Max |
|---|---:|---:|---:|
| NN | 7.3773 | 15.8435 | 33.0765 |
| IDW8 | 3.2406 | 7.6626 | 18.8744 |
| Local ridge | 1.2086 | 3.2173 | 11.6811 |

Active ink count 0..7: [1,119,308,1406,2025,339,73,1]. Stress split trains on 3,859 recipes with <5 active inks, tests 413 with >=5. NN mean/P95/max 6.9297/13.0597/22.9100; IDW8 6.1718/11.0556/15.1818; local ridge 3.4681/6.8795/19.7700. Local ridge has worse worst-case stress error than IDW8 despite better mean. No prediction continuity guarantee when neighborhood changes, no spectral or independent print validation. All figures are actual independent Python calculations, not Rust execution. Opt-in Rust test now asserts pooled and stress mean within 0.01 of reference. No ICC export.

## beta.37: constrained inverse, forward continuity and reverse ramp
The inverse uses the original hard-neighborhood ridge, 3 deterministic starts, measured/projected seed recipes, integer 0.0001 percentage-point coordinates, floored per-channel caps and TAC, coordinate/transfer search, max 1,200 uncached search evaluations, plus local sensitivity probes. Distance ceiling is a heuristic; tolerance means model prediction, not print approval. No original ICC pipeline or ICC generation. A missed tolerance does not prove the target is impossible.

Reopened operator-supplied FOGRA55.txt (4,884 rows / 4,272 grouped recipes). Independent Python/NumPy + skimage CIEDE2000 mirror, 128 evenly spaced lexicographic anchors, all feasible single-axis moves within 0–100%; 1,237 probes per step. These are full-data anchor perturbations, not holdout errors or proof of global continuity. Tapered candidate uses 32 neighbors, 33rd neighbor squared distance R², weights proportional to (1-d²/R²)^2/(d²+0.01), ridge 0.002 on slopes; exact-anchor blend with alpha=(max(0,1-d/min(0.02,d_second)))². Reject degenerate kernel. Candidate is NOT substituted into inverse solver.

| Step (percentage points) | Hard mean | Hard P95 | Hard max | Tapered mean | Tapered P95 | Tapered max |
|---|---:|---:|---:|---:|---:|---:|
| 0.01 | 0.446686 | 0.887893 | 1.359250 | 0.002321 | 0.006775 | 0.010810 |
| 0.10 | 0.449340 | 0.895853 | 1.393576 | 0.022913 | 0.065532 | 0.110940 |
| 0.50 | 0.471706 | 0.937082 | 1.585634 | 0.112060 | 0.277419 | 0.608007 |

| Held-out evaluation | Count | Hard mean/P95/max | Tapered mean/P95/max |
|---|---:|---:|---:|
| Grouped five-fold | 4272 | 1.208607 / 3.217346 / 11.681106 | 1.084994 / 3.046417 / 9.478160 |
| >=5 active inks stress | 413 | 3.468146 / 6.879548 / 19.770024 | 3.485905 / 9.386173 / 13.781778 |

The tapered model reduces anchor jumps and pooled errors, but stress P95 is worse. Do not pick a production model using the same test corpus. No claim of continuous mapping globally, spectral accuracy or physical print validation.

Independent inverse ramp mirror, hard ridge, center Lab(50,0,0), K cap 0%, other caps 100%, TAC <=300%, distance <=0.4, tolerance 2:

| L* | C | M | Y | K | O | G | V | TAC | Predicted DE00 | Max adjacent ink change (pp) |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 48 | 61 | 50.5 | 54 | 0 | 0 | 0 | 0 | 165.5 | 0.177032 | — |
| 49 | 60 | 49.19 | 52.69 | 0 | 0 | 5 | 11.5 | 178.38 | 0.118670 | 11.5 |
| 50 | 58.5 | 48 | 51.5 | 0 | 0 | 0 | 9 | 167 | 0.214549 | 5 |
| 51 | 57 | 46.24 | 50.24 | 0 | 0 | 5 | 11 | 169.48 | 0.216062 | 5 |
| 52 | 56 | 45 | 49.5 | 0 | 0 | 0 | 4.5 | 155 | 0.098557 | 6.5 |

All five satisfy predicted tolerance but adjacent single-ink jumps reach 11.5 pp. This is the inverse ambiguity that must be regularized before reverse LUT construction. Subsequent work should use neighbor-aware separation with a bounded color error tradeoff, and test both forward and inverse lattice continuity. Numbers above are independent Python diagnostics, NOT compiled Rust execution. New opt-in real-data assertions accompany existing FOGRA55 test. No source data redistributed, no ICC generated.

## beta.38 — continuity-regularized inverse ramp
The existing independent five-point hard-ridge inverse remains available. New regularized refinement minimizes sum of squared model ΔE00 plus lambda times summed squared adjacent ink differences in percentage points. Four alternating sweeps over five nodes, steps 5/2/1/0.5 pp, candidate neighbor mean, halfway blend and projected coordinate changes. Every trial respects floored integer channel caps and TAC, supported recipe distance, and a pointwise color guard: a previously matching point remains within tolerance; a previously missed target cannot worsen (numeric tolerance 1e-12). At lambda=0 the exact independent baseline is returned. No claim of global optimality or maximum-jump monotonicity. Up to 80 candidate batches, at most 32 trials/batch for 15 channels; cached by point and integer recipe. No profile generation.

Independent NumPy/skimage mirror on reopened operator-supplied Ref_FOGRA55.zip / FOGRA55.txt, 4,272 grouped recipes, K cap 0%, other caps 100%, TAC <=300%, max distance 0.4, tolerance 2. No data redistributed. Center L50, a=b=0, lambda=.02:

| L* | C | M | Y | K | O | G | V | TAC | ΔE00 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
|48|60.5000|50.1900|53.6900|0|0|3.0000|10.0000|177.3800|0.325897|
|49|59.7658|48.9475|52.6523|0|0|3.1655|9.7828|174.3139|0.095449|
|50|58.4931|47.6547|51.4818|0|0|2.9902|9.6395|170.2593|0.104306|
|51|57.2465|46.3273|50.4909|0|0|2.7451|9.0697|165.8795|0.158101|
|52|56.0000|45.0000|49.5000|0|0|2.5000|8.5000|161.5000|0.133079|

| Ramp | Lambda | Baseline max jump pp | Refined max jump pp | Baseline max ΔE00 | Refined max ΔE00 | Objective before → after | Refinement evals |
|---|---:|---:|---:|---:|---:|---|---:|
| L48–52 | .02 | 11.5 | 1.3274 | .216062 | .325897 | 6.233547 → .515131 | 883 |
| L18–22 | .02 | 70 | 18.7902 | 1.025488 | 1.664711 | 190.282273 → 31.780664 | 1016 |
| L18–22 | .10 | 70 | 15.8632 | 1.025488 | 1.823913 | 945.262273 → 116.306157 | 1009 |

All 15 refined model predictions satisfy tolerance and caps, but dark separation remains discontinuous. Lambda=.10 trades more color error for lower jump; objectives at different weights are not directly comparable. All figures are independent Python diagnostics on the full reference dataset, not compiled Rust or independent print validation. No ICC generated. Next: wider neighborhood/coupled continuation and 3D lattice budget/coverage feasibility, physical validation; do not infer global continuity from five-point ramps.

## beta.39 — bounded local 3D Lab lattice
27 nodes, 54 axis-aligned edges (18 per axis), 8 cubic cells. Configurable center and per-axis half-span (0.5–25 Lab units); reject out-of-PCS requests. Independent hard-ridge inverse seeds, explicit unsupported nodes (never substitute zero), four alternating sweeps of joint six-neighbor refinement with existing error/coverage/cap/TAC guards. Fixed 3x3x3 size and conservative 3 billion channel-distance operations ceiling; estimate is a bound on distance work, not wall-clock duration. Seed evaluation count covers successful searches only. Report node errors and edge jumps separately by axis. At each complete cell center independently re-evaluate the integer-floored equal-weight mixture of its eight recipes using the forward model; report unsupported/missed centers separately. This samples one location per cell, does not establish whole-cell accuracy. No ICC serialization.

Independent Python/NumPy/skimage mirror rerun on supplied Ref_FOGRA55.zip / FOGRA55.txt (4,884 rows, 4,272 unique), actual local cube centered Lab(50,0,0), half-spans (1,1,1), K cap 0%, other caps 100%, TAC <=300%, max recipe distance .4, tolerance 2, lambda .02, four sweeps. All 27 nodes and all eight centers within predicted tolerance before and after.

| Metric | Independent | Regularized |
|---|---:|---:|
| Node mean DE00 | 0.172808 | 0.275284 |
| Node max DE00 | 0.344624 | 0.457056 |
| L-axis max ink jump pp | 18 | 1.5554 |
| a-axis max ink jump pp | 18 | 1.3935 |
| b-axis max ink jump pp | 18 | 1.6076 |
| Center mean DE00 | 0.581480 | 0.207189 |
| Center max DE00 | 1.085051 | 0.283328 |
| Max node TAC (%) | 190.38 | 172.8591 |
| Joint objective | 142.734046 | 5.204566 |

4,860 distinct refinement trials in independent mirror; no closed-channel or TAC violations by construction. This narrow cube is deliberately chosen to establish local behavior and cannot represent dark or saturated areas or entire PCS. Same reference dataset fits and evaluates model; no independent physical measurement. Python diagnostics, not compiled Rust real-file execution. Real-data regression is opt-in; synthetic topology/limits tests are ordinary CI. Next: broaden coverage sampling, evaluate forward/ reverse full-lattice cost and continuity, explicit gamut mapping for out-of-gamut targets before ICC serialization.

## beta.40 — bounded anisotropic forward CLUT diagnostic
A configurable per-ink grid (2–33 samples per axis, 2–15 channel input), checked product, max 8,192 nodes and 500 million estimated channel-distance operations. This budget rejects even 2^14 or 2^15; it is a diagnostic limit, not an ICC standard limit. Raw payload estimate is 6 bytes/node for three 16-bit Lab components, excludes header, curves, mask and other tags. Uniformity flag describes grid dimensions only, not legacy mft2 serialization eligibility. Hard ridge sampled in the full 0–100% device domain, not inverse caps or TAC. Nodes beyond nearest-recipe distance threshold are absent. The interpolation reference uses positive-weight corners only, last axis fastest, Lab16 full-range normalized encoding, and no unsupported-corner substitution. It is NOT native CMM interpolation, ICC media-relative PCS conversion, or a serialized tag.

Independent Python/NumPy/skimage calculation using operator-supplied Ref_FOGRA55.zip / FOGRA55.txt, grouped 4,272 recipes, fold 0: 3,405 training, 867 holdout. Fit ONLY training rows; max normalized recipe distance .4. No files redistributed. Raw payload and error reported on supported subsets; coverage is essential to interpreting error.

| Grid (CMYKOGV order) | Nodes | Supported nodes | Raw bytes | Interpolated holdout count | Mean / P95 / Max DE00 | Direct model mean on same holdouts |
|---|---:|---:|---:|---:|---|---:|
| 3×3×3×3×3×3×3 | 2187 | 1237 | 13122 | 742 | 1.295872 / 2.572715 / 6.318467 | 1.053203 |
| 5×5×5×5×2×2×2 | 5000 | 3300 | 30000 | 743 | 1.704474 / 4.353256 / 7.597168 | 1.100251 |
| 3×3×3×3×2×2×2 | 648 | 415 | 3888 | 706 | 1.843227 / 4.631433 / 7.633624 | 1.103331 |

Direct model on 861/867 distance-supported holdouts: mean 1.226860, P95 3.284877, max 7.503415. Uniform 3-grid has lower error than the 5/2 anisotropic grid on their respective supported subsets; these are not identical subsets and do not prove universal superiority. None has complete full-domain node coverage. No absent nodes filled; no ICC generated. Python reference only, not compiled Rust results. Opt-in FOGRA55 asserts coverage and mean errors, ordinary tests check sizes, order, missing corners and held-out synthetic recipes. Next: explicit gamut/target mapping and model support policy, scalable full-domain forward model, native CMM validation before generating experimental ICC.
