# C07 ArkTS environment observations

Run the read-only preflight with explicit paths. `--sdk` is the product package
root containing `sdk-pkg.json` and `openharmony/`, not a CPF compiler sysroot.

```sh
.venv/bin/python scripts/check_arkts_environment.py \
  --deveco /path/to/DevEco-Studio.app/Contents \
  --sdk /path/to/application-sdk-package \
  --target 127.0.0.1:5557 \
  --target-abi arm64-v8a \
  --json-output /tmp/new-arkts-environment.json
```

The command reads SDK/tool metadata, runs version commands, and queries seven
nonsecret parameters from the selected target. Target requests use `hdc -p` so
they do not start its server. It does not install, launch, restart, sign, build,
discover devices, inspect credentials, or read another project's caches. Tool
probes have a per-command timeout (default 10 seconds, maximum 30). Timeout
terminates only that client and does not imply an absent target.

Output is an internal diagnostic JSON, not a candidate, frozen profile or Lab
acceptance report. The `environment_checks_passed` field and exit 0 mean only that the observed
environment conditions passed:
Release SDK component metadata is consistent, tools were found, and the selected
target supplied Release runtime parameters and the requested ABI (default
`arm64-v8a`). This ABI is an explicit preflight requirement, not a frozen profile.
It does not prove SDK provenance,
CPF/akinterop support, SDK/target compatibility, physical hardware, HAP/HAR
consumption, P05/P06 or C07 acceptance. The existing formal toolchain candidates
remain unchanged. A known emulator is identified as such; any other target is an
unverified device, not automatically a physical device.

Omitting `--target`, Beta metadata, malformed/missing components, failed tools,
target connection errors and timeouts produce exit 1 and retain observations.
Bad arguments and output errors produce exit 2. An optional JSON output path must
be new and have an existing parent; publication is atomic and never overwrites.

```sh
.venv/bin/python -m unittest discover -s tests/integration/c07_environment -v
```

These tests use synthetic executable/metadata fixtures to prove the preflight's
failure semantics. They are not ArkTS target execution or certification evidence.
