# L05 value declaration projection

This slice separates pure-value structure reading from the unchanged full C07
profile. `readValueDeclarations` accepts required number/string record fields
and synchronous free functions using scalar/record values. Resources, exported
classes, Promise/callback types, imports and unsupported syntax are rejected.
It does not select numeric ABI, ownership or a native backend.

The generator retains the real authored `Business.ets` implementation and emits
its ordinary export barrel, declaration view and source locations/digest.
It has no private business-name list. Original, added-function, added-field,
renamed and deleted-function sources are independently asserted and regenerated.
The default C07 reader still rejects these value-only inputs and retains its
ordinary/rename and three diagnostic regressions.

Run from the repository with pinned Python/Node and the existing formal SDK:

```sh
.venv/bin/python -B tests/integration/l05_declaration_generation/check.py \
  --cli-root /absolute/formal/command-line-tools \
  --cli-inventory /absolute/frozen/cli-inventory.json \
  --java-home /absolute/jbr/Contents/Home \
  --typescript /absolute/locked/typescript.js \
  --typescript-sha256 REVIEWED_MODULE_SHA256 \
  --output /absolute/new/output-outside-repository
```

The generated declaration view is independently consumed by the locked
TypeScript compiler and compared with the formal SDK's actual HAR declarations;
export names, record fields and positional parameter/result types must match.
The SDK may rename parameter identifiers during obfuscation, so those local
names are not treated as public callable identity.

The check builds five actual Release HARs from generated exports and real
business implementations, then five isolated ordinary HAP consumers with the
selected SDK. A sixth consumer importing the deleted method must fail with its
specific source diagnostic. No packages are downloaded, no application is
installed and no native stubs are used. All generated files, compiler logs and
temporary build caches remain outside source. The current home is retained.

This is a reusable declaration projection frontend, not completed second-business
cross-language generation: provider discovery, explicit numeric/ownership
semantics, checked ContractIR integration, C04 Source publication and independent
candidate/runtime validation remain work for L02/L04/L05 and the existing route
packages. It does not change a C07 profile or certify general Facade generation.
