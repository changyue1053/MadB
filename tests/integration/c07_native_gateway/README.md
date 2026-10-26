# C07 native composition checks

Run `python -m unittest discover -s tests/integration/c07_native_gateway -p 'test_*.py'`.
These checks derive the real source variants, verify their byte identities and
reject changed baselines, malformed patches, missing inputs and output reuse.
They do not invoke tools or a target.

`run_host.py` additionally accepts explicit `--native-sdk`, `--cc`, `--cxx`,
`--uv-include`, `--output` and optional `--route c|cpp`. It builds real Core,
provider and worker code in Release and ASan/UBSan. The twelve registry cases use
only an explicitly labelled Node-API return harness. All output stays in the
fresh requested directory. The supplied SDK headers and host tools are recorded
as observations, not product toolchain admission. No device is accessed and no
host success is reported as ArkTS target execution.

The companion v2 host suite runs eleven native owner/gate cases and twelve
registry/identity cases in both configurations. Registry tests disable observer
macros in their explicit return harness; they prove only registry behavior. A
separate observer binary runs the actual wrappers against 32 recording-stub calls
in each configuration. Seven scanner controls reject missing interception and
raw-call bypass. Final target builds additionally verify actual SDK preprocessed
translation units with `sdk/arkts/test-support/native/observer_inventory.py`.
