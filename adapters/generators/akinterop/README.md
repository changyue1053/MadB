# Managed akinterop source for C07

This directory preserves the reviewed `managed-akinterop-mainenv-r2` source patch for the CPF/akinterop route. It is a mechanism-probe input, not a certified C07 profile, published SDK or complete installer.

[The source pin](managed-source.json) binds the upstream Git commit, Kotlin source archive, ten original and patched source files, patch bytes and upstream notices. Apply the patch only to a new extraction of that commit, verify every original file first and every resulting file afterward. Never modify a shared Maven/Gradle cache. The archive contains Kotlin source; the native helper is separately sourced from the same pinned Git revision.

The patch provides bounded explicit reference ownership, main-env/thread admission, owned nonblocking TSFN delivery, null-env native cleanup, registry cleanup and temporary argument release. r2 also initializes native call outputs and returns null on failed N-API calls, fixing an indeterminate pointer on callback exceptions. It preserves the upstream helper ABI; compatibility does not permit mixing differently identified helper/runtime binaries.

Build the native helper, cinterop, Kotlin runtime, KSP-generated exports and consumer as a matched set. The native helper uses the locked formal application SDK headers; Kotlin cinterop/runtime use the separately locked CPF platform headers/klib. The exact bridge ABI comparison and LLVM/sysroot/linker correction belong in the toolchain recipe, not this source pin.

The managed path permits only its registered main env and rejects foreign-thread N-API use. Legacy synchronous callback and uncontrolled Promise paths are outside the admitted subset. Capacity is 128 owned references and 32 managed pending notifications; these mechanism bounds do not establish the final SDK's complete payload/operation/waiter budget.

Independent source replay and matched Release ownership/notification experiments are recorded in [the C07 review](../../../docs/reviews/2026-10-25-c07-arkts.md). They do not prove main-env teardown, full state/event byte saturation, bootstrap-before-resource admission or complete candidate/Lab certification. The previous r1 patch and its receipts retain their historical identity; no old binary evidence is relabeled as r2.

No downloaded source tree, native binary, cache or signing material is distributed here. Source-patch attribution is in [ATTRIBUTION.md](ATTRIBUTION.md).
