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
