# Native CMY support (pending CI)

The ICC device signatures CMY (three standard colorants) and 3CLR (generic three-channel output) are distinct. Use LittleCMS CMY_16 for the first, PT_MCH3 for the second; retain the original signature, channel count and A2B data. This is not a CMYK-to-CMY channel-removal feature or new printer characterization.

- Native CMY is admitted at the shared print-space and installed-profile picker gates, with standard Cyan/Magenta/Yellow swatches and dynamic channel limits.
- Custom rebuild supports three caps and TAC up to 300% through the shared engine. It never inserts or assumes a K channel. All three source A2B tables remain unchanged.
- Native CMY does not permit arbitrary physical ink permutations under the fixed signature. Generic 3CLR remains a separate format. Existing profiles are not silently reinterpreted.
- CGATS input recognizes CMY_C, CMY_M, CMY_Y only when the loaded ICC signature is native CMY; order is mapped explicitly, mixed column families rejected. Existing numbered CH1..CH3 import/export remains available. ISO28178 import retains its explicit identity checks.
- Mixed CMY/CMYK DeviceLink tests check source and destination header signatures and existing sampled deviation gates. Display metadata uses shared predicted/standard color provenance.
- Tests use synthetic fixtures, not measured native CMY profiles. Four-intent Roundtrip and bounded rebuild, picker, CGATS mapping and mixed links are included. Run CI and test actual compatible native CMY profiles before production; no CMY Argyll fitting workflow, physical print, or RIP validation is claimed. v2 bounded correction is tracked in separate PR #16. No version bump in this independent PR to avoid conflicting with beta.34.
