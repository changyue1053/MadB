# ArkTS native gateway

This is the single ordinary Node-API gateway for ADR 0011 and ADR 0013. It uses
`sdk/arkts/native-runtime` and the selected real C/C++/Rust provider. It contains
no test methods. The ordinary facade remains in `../facade`.

`adapters.toolchains.arkts.native.source_paths` supplies the explicit freeze
allowlist. `compose_native(repository, output, route=..., variant=...)` creates a
fresh source workspace and returns its original/derived byte identities, compile
units, include directories, private declaration and facade. It does not run a
compiler, SDK launcher, device or download. C04's worker owns compilation.

The identity generator supplies `identity.generated.hpp`, including the provider
ABI and constants `GatewayAbi`, `GatewayProfile`, `GatewayBuild`,
`GatewayIdentity`, `GatewayRouteIdentity`, `GatewayModuleName`, and the selected
`GatewayProviderCreate` function pointer. Build identity must cover these new
source bytes; earlier development binaries are not inputs to this build.
`frame.generated.hpp` derives from the facade's versioned frame layout. The
export initializer derives from the actual `Index.d.ts` reader output.

C/C++ use the single marked constructor registrar. The genuine Rust frontend
replaces only the marked callback declaration and registrar blocks, preserving
the actual handler bodies. Rust's registrar and provider remain in the Rust
source boundary. The source composer lists its Rust unit explicitly as Rust;
a worker must never feed it to a C/C++ compiler.

The initial source is the reviewed ordinary development gateway (native SO
`6fa0a7031e2f3abc5704b9e70203545692997a9add933d194db7fc96f260bf24`).
Repository integration changes only formatting, factory/module constants and
explicit Rust derivation markers. Historical target results remain scoped to
that old identity; this source needs a fresh C04 build and target verification.
