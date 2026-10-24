# Gradle wrapper 9.7.1

The committed `sdk/kotlin/skie/gradle/wrapper/gradle-wrapper.jar` is unchanged from the `gradle-wrapper.jar` resource inside `lib/plugins/gradle-wrapper-main-9.7.1.jar` in the verified Gradle 9.7.1 binary distribution. Its SHA-256 is `7a9ce74cff467ca1bf60a4fcd9f05185acceda4d0f382434d393e17864262c5d`.

[LICENSE](LICENSE) is the exact `META-INF/LICENSE` text embedded in that wrapper JAR (Apache-2.0). [NOTICE](NOTICE) is retained unchanged from the same upstream Gradle distribution. That distribution notice names the Apache Software Foundation and bundled distribution components; it is preserved as upstream notice text, not a claim that all listed components are contained in the wrapper JAR.

The distribution URL and archive SHA-256 are pinned in [the managed toolchain recipe](../../adapters/toolchains/kotlin_rust/toolchain.json). This source PR carries the small wrapper JAR and wrapper scripts, not the full Gradle distribution. Original copyright and license headers in the wrapper scripts remain unchanged.
