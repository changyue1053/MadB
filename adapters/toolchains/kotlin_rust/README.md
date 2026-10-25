# C06 managed tool installation

The committed recipe fixes the source URL and SHA-256 of Gradle, Temurin JDK, Kotlin/JVM, Kotlin/Native and its native compiler dependencies for macOS Arm64. Source archives are checked before extraction; no downloaded program executes during installation. The source cache is optional and every hit is rehashed.

```sh
.venv/bin/python scripts/install_managed_tools.py --root /tmp/madb-managed --cache /tmp/madb-source-cache
.venv/bin/python scripts/install_managed_tools.py --root /tmp/madb-managed --verify
```

The root must be new. A failed install leaves no successful receipt; use a new root after correcting the cause. `--components gradle jdk kotlin-jvm` installs an explicit subset for a probe; a subset does not certify all four C06 routes.

`verify(root, receipt_sha256)` checks every file, executable bit and contained symlink against the installation inventory, including jars and native libraries, before and after each build phase. The producer must bind `receipt.json` itself in the frozen input lock and pass that digest; verification without an externally pinned receipt is only a local consistency check. Returned `paths` map component names to their roots. Set `JAVA_HOME` to `paths["jdk"] + "/Contents/Home"`, use `paths["gradle"] + "/bin/gradle"`, `paths["kotlin-jvm"] + "/bin/kotlinc"` and `paths["kotlin-native"] + "/bin/konanc"` explicitly. Do not infer tools from PATH. Set `KONAN_DATA_DIR` to installation `konan` and configure the compiler cache outside these immutable distributions.

`install_dependencies(lock, root, cache)` / `--maven-lock LOCK` accepts a reviewed `madb.maven-files/1` object with `files` entries containing relative `path`, exact HTTPS `source`, `sha256` without a prefix, and `size`. It fetches that closure without dependency resolution. The lock includes POM/Gradle module metadata as well as selected JAR/KLIB artifacts. Kotlin stdlib follows the locked compiler; kotlinx-coroutines 1.11.0 Native metadata requires atomicfu 0.32.1.

Gradle plugins and their transitive dependencies must first be resolved in explicit preparation, then recorded in dependency locks and SHA-256 verification metadata. Frozen build inputs contain the selected closure, build scripts and locks; execution uses `--offline --dependency-verification=strict` with a private Gradle home. The developer's `~/.gradle`/`~/.konan` is never proof of a clean installation.

Xcode remains an externally installed Apple toolchain: compare exact Xcode/SDK/Swift observations and hash the actual compiler/SDK inputs in each route profile. These recipe observations do not claim that all routes, targets or Release consumers have passed.

The committed `runtime-dependencies.json` records the JVM/Native consumer runtime closure; `skie-dependencies.json` records all resolved SKIE compiler/plugin artifacts and the exact verification-metadata digest. Their paths are suitable for a flat runtime directory and a Maven repository respectively. The Kotlin plugin marker comes from Plugin Portal: Maven Central publishes different bytes for the same coordinate, so these sources must not be interchanged silently.

The installer creates and verifies Kotlin/Native's `.extracted` registry from the selected native dependency archives. Frozen compiler phases also set `airplaneMode=true`, an explicit Kotlin/Native home and Release builds without caches in the compiler distribution (or a separately locked cache location), so runtime-created compiler caches cannot mutate the installed distribution. `gradle.lockfile` and `verification-metadata.xml` remain route-owned build inputs.

The libffi and LLDB source archive hashes were measured from their fixed JetBrains HTTPS archive URLs; their observed `.sha256` sidecars did not match the archives and are not treated as validation evidence. The immutable recipe pins the actual archive bytes. JDK/Kotlin/Gradle/LLVM archives matched their published release digests. Source locks do not replace upstream security review.

Do not use `kotlin.native.cacheKind=none` with Kotlin 2.4.20: the old property was removed in 2.3.20. The cold Release build does not need it. Any profile that needs to disable supported native caches must use Kotlin's version-specific `disableNativeCache` DSL and validate the resulting behavior.

`tools.discover(repo, managed_root, gobley_root=None, runtime_root=None, maven_root=None, cargo_root=None)` records compact full-tree commitments for the Apple SDK, Apple compiler libraries, Rust libraries and selected dependency roots. `tools.verify(observation)` recomputes these and verifies compiler installation receipts before/after each phase. `register_image` adds a source-identified helper image; `register_tree` freezes a complete source/vendor tree. Runtime/Maven roots must exactly match their committed file locks, including refusal of extra files. Aggregate tree identities avoid putting tens of thousands of SDK paths into bounded C04 wire arrays while retaining mutation detection.

An observed executable keeps its callable `path` and pins its resolved `image_path`, invocation name and launcher link. This preserves Swift multi-call dispatch while rejecting link retargeting. Python alone uses its resolved path for the C04 host's no-symlink tool port.

Python helpers run with the recorded `-I -S -B -X pycache_prefix=/dev/null` arguments. The runtime inventory covers the actual framework/standard library and recursively observed non-system native dependencies. Only disabled `site-packages` and `__pycache__` locations are omitted; standalone importable bytecode remains inventoried. Cache redirection prevents existing unchecked bytecode from bypassing the frozen source tree. OS shared-cache libraries remain identified host dependencies, recorded with the macOS build rather than redistributed as SDK files.

Internal dangling links in a Python framework remain inventoried with their exact target and `target_missing: true`; an empty target directory appearing later also changes the commitment. Link loops, access errors and root escapes fail verification. Actual executable and native dependency images must still exist. This accommodates the Python.org Tcl header links without omitting them from the frozen runtime.
