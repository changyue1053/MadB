# L05 API26 compiler probe

This development check compiles the actual observation sources with the formal
API26 SDK. It follows the C07 Release/Hvigor recipe and reuses the existing bounded
HAR reader. It does not produce a C04 candidate or certify C07/L05.

Use repository-pinned Python and an existing, reviewed mac-arm64 Command Line
Tools 26.0.0.851 installation, its frozen acquisition inventory, and Java 21:

```sh
python3 -B scripts/check_arkts_observation.py \
  --cli-root "$MADB_CLI_ROOT" \
  --cli-inventory "$MADB_CLI_INVENTORY" \
  --java-home "$MADB_JAVA_HOME" \
  --output "$MADB_L05_NEW_OUTPUT"
```

All four arguments are required. The output must be a new directory outside the
repository and tool installations. Protected roots must exist; existing output
ancestors are compared by filesystem identity, including symlink and case aliases.
Each build gets its own HOME, Hvigor, npm and
temporary directories underneath that output. The check does not install tools
or dependencies, invoke hdc, sign an application, contact a registry, or reuse a
previous build cache. It constructs only local declaration/HAR dependency maps;
this is not proof of ohpm installation or publication.

The acquisition inventory is verified against the complete installed CLI tree
before and after compilation. The existing C07 profile rejects other CLI/SDK
versions; this recipe is specifically SDK 26.0.0.105 Release, Hvigor 6.26.8 and
bundled Node 24.14.1. Java 21's actual version/image and the Python image are recorded.
This does not close the entire Java/OS runtime or authenticate the vendor archive.

The check builds two separate obfuscated Release HARs. The ordinary package
contains StateObservation and the C07 facade, with no ArkUIStateModel source or
export. The UI package adds ArkUIStateModel. Two fresh HAP projects consume only
the generated HAR and its declarations: both compile attach/requestStop/await
close; the UI project also compiles a ComponentV2/Local page reading the ObservedV2
model's snapshot. Both use native declarations only; no native DSO is shipped.

`success.json` is written only after all four builds and unchanged-input checks
pass. `failure.json` records failures, attempted commands and their project inputs,
including malformed inventory failures; a failure exits
nonzero. Existing output is rejected without altering its evidence. Receipts
include source/support/tool inventory hashes, commands/environments, every project
input, artifacts, HAR declarations, and complete stdout/stderr log hashes. The
SDK can warn about generated `@ts-nocheck` declaration comments, unsigned output
and HAR product applicability; warnings remain in the logs and are not suppressed.

Compilation proves ArkTS acceptance, declaration export and independent package
consumption at this selected SDK. It does not execute the consumer methods, prove
UI refresh, native loading/drain, page/env lifecycle, signing, or a published
candidate's installation. Host protocol tests and target execution remain separate.
