# Multichannel validation and N-color fitting research

## Real profile corpus (not executed)
Provide licensed output ICC/ICM files in a private directory, not the repository. Confirm the header declares prtr and 3CLR, 5CLR, 6CLR, 7CLR, 8CLR, FCLR respectively; physical printer ink count alone does not establish ICC device space. Record source, license, SHA-256, ICC version, PCS, channel order and LUT types. Run:
```powershell
$env:ICC_REAL_PROFILE_DIR = 'C:\private\icc-corpus'
cargo test --lib --no-default-features --release real_profile_corpus -- --ignored --nocapture
```
The harness inventories the declared capabilities, tests a bounded rebuild with the last channel closed when supported, and reports missing counts. It does not claim all profiles are editable. Manually exercise every workspace, compare four intents, source hash, maximum channel values, TAC, Mean/P95/Max Delta E, channel mapping, runtime and RIP print acceptance. Preserve source files.

- 7CLR candidate: FOGRA55 CMYKOGV exchange space, with characterization data and reference ICC archive: https://fogra.org/en/research/prepress-technology/multiprimary-printing-13003
- Additional registry candidate: APTEC CMYKOGV https://registry.color.org/profile-registry/
- 3/5/6/8/15CLR: obtain permission-cleared output profiles from printer/RIP operators; no verified file corpus acquired. Synthetic fixtures are not real measurements.

## N-color ICC fitting
Argyll colprof creates RGB/CMY/CMYK profiles, not arbitrary nCLR (https://www.argyllcms.com/doc/ArgyllDoc.html). ICC iccDEV offers ICC manipulation, construction and validation tooling, not a proven general measured N-color fitting solution (https://www.color.org/iccdev/). ICC research topic 10 explicitly includes characterization for n-colour printers (https://www.color.org/resources/research/icc_research_portal/). A candidate implementation requires:
1. Measured recipes, explicit physical channel order, safe limits, measurement conditions and independent holdout samples.
2. Fit a forward device-to-Lab/spectral model with regularization and held-out validation. Dense N-D CLUT growth is exponential: enforce memory and error budgets; high-N profiles may require sparse sampling/model stages and compatible consumers.
3. Invert to bounded B2A tables using explicit ink priorities, gamut mapping and black selection; validate all intents independently.
4. Serialize supported v4 ICC structures, check with LittleCMS and independent ICC validator, and test actual RIP/prints.
Do not use synthetic targets as measurements, fabricate ink identity, or claim that an edited separation is a new characterization. N-color fitting remains a future research milestone.

## beta.32 — regression coverage
The synthetic 3/5/6/7/8/15-channel matrix performs a bounded rebuild, serializes and reopens the ICC, checks preserved forward transforms, exact zero inks, a 10% channel cap, TAC, four native rendering intents and no-clobber saving. A native complex mBA matrix/M-curve regression exercises bounded smoothing. These are synthetic, not proof of physical color accuracy.

Run a single available real/reference count without requiring the complete corpus:
```powershell
$env:ICC_REAL_PROFILE_DIR = 'C:\private\icc-corpus'
$env:ICC_REQUIRED_COUNTS = '7'
cargo test --lib --no-default-features --release real_profile_corpus -- --ignored --nocapture
```
The harness reports Mean/P95/Max predicted DE00 for four intents, sampled TAC, exact bounds, preserved A2B and unchanged source bytes. Unsupported profiles are explicitly logged as skipped; inventory coverage is not edit success.

FOGRA55 is a seven-channel exchange space, not a measured profile for the user's particular printer. Official download: https://fogra.org/fileadmin/files/1_forschen/vorstufe/13.003_multicolor/Daten/Ref_FOGRA55.zip . Download was not successful in the development environment, so no result is claimed. Acquire manually, inspect license and checksum, extract ICC only into private corpus. Do not commit third-party profiles without permission.
