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
