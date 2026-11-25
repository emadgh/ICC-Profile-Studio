# CIE spectral data attribution

Copyright International Commission on Illumination (CIE), Vienna, Austria. The five original CSV datasets are licensed under [Creative Commons Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/). The published CSV bytes are embedded unchanged in official builds. The computation code is separate. NaN in CIE 1964 z-bar above 559 nm is interpreted as zero per the dataset's metadata (zero extrapolation).

- CIE 2019, CIE 1931 2-degree observer. DOI: https://doi.org/10.25039/CIE.DS.xvudnb9b. Original file: https://files.cie.co.at/Publications-datasets/CIE_xyz_1931_2deg.csv. SHA-256: `fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1`.
- CIE 2019, CIE 1964 10-degree observer. DOI: https://doi.org/10.25039/CIE.DS.sqksu2n5. Original file: https://files.cie.co.at/Publications-datasets/CIE_xyz_1964_10deg.csv. SHA-256: `1b27fd4e8ca1167b47c3a6aee3aafe56abc57eae51fa20032cb83704224a27dc`.
- CIE 2022, standard illuminant D50. DOI: https://doi.org/10.25039/CIE.DS.etgmuqt5. Original file: https://files.cie.co.at/Publications-datasets/CIE_std_illum_D50.csv. SHA-256: `b23049c6f7b266c1c1fbe147aa271e8930ca02d6e569c5ae1804c036faea4193`.
- CIE 2019, standard illuminant D65. DOI: https://doi.org/10.25039/CIE.DS.hjfjmt59. Original file: https://files.cie.co.at/Publications-datasets/CIE_std_illum_D65.csv. SHA-256: `e76f210bffff3d552ef7113025da5f325d5dfec200dd4b878b1a2f3a507032cb`.
- CIE 2018, standard illuminant A. DOI: https://doi.org/10.25039/CIE.DS.8jsxjrsn. Original file: https://files.cie.co.at/Publications-datasets/CIE_std_illum_A_1nm.csv. SHA-256: `61ef23fe146b8b665c74706717ab28cec7db6c9022993490bdc71991f43cb59b`.

Run `pwsh -File scripts/fetch-cie.ps1` from the repository before building locally. The CI workflow requires all five verified tables. Local builds without the tables retain the explicitly labelled legacy D50/2° estimator; other viewing conditions are unavailable. No network access is needed by the installed executable.

The integration uses 1 nm summation and piecewise-linear interpolation of measured reflectance, with constant endpoint extension outside the measured range. It is an estimate, not ASTM E308 bandpass correction or ISO 13655 conformance. The original measurement mode is retained; fluorescence and M0/M1/M2/M3 cannot be reconstructed from ordinary reflectance alone.
