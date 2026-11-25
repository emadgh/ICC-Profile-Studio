# ICC profile characterization and validation

1. Identify the device condition, substrate, ink channel mapping and measurement geometry. Establish safe ink caps/TAC.
2. Generate direct-device charts in Profiling Lab, print without additional color conversion, then measure using an external measurement workflow.
3. Compare matching measured patches from CSV files. This quantifies differences but does not automatically correct an ICC.
4. Build a new profile using real measured CMYK TI3 data and Argyll colprof; do not invent measurements.
5. Compare the new ICC to the reference using gamut, Delta E, gradients and ink distribution. Use DeviceLink generation if applicable. Apply the profile externally; Studio does not convert images.
6. Save each edit to a new ICC and test the actual output. Unknown profiles or unsupported LUT structures must be rejected rather than approximated silently.

Direct measurement-device integration is outside scope. All in-app color predictions are model-based unless explicitly based on imported measurements. Validate supported real profiles, Windows UI behavior, dependency licenses, update and Argyll package integrity before a stable release.

For N-color, Studio prepares characterization targets and measured comparisons; it does not fit a new N-color ICC. Retain original measurements and the exact layout manifest. See RELEASE_QA.md before treating a preview as production-ready.
