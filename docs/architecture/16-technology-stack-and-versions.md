# MadB 技术栈与版本选型

选型日期：2026-10-24；C07 鸿蒙候选更新：2026-10-25。设计基线：0.9。状态：设计采用，所有实现与认证仍按计划推进；具体版本是验证候选，正式支持以通过 MadB-Lab 验收的配置、锁和制品摘要为准。

MadB 采用四条独立版本线：官方 Kotlin、鸿蒙 CPF、原生实现与绑定、MadB 工具自身。共享契约与业务源码，分别构建，再验证组合应用。鸿蒙 Kotlin 默认采用 **CPF-KMP-CMP 工具链 + akinterop**；CMP 按需启用，KNOI 保留为独立扩展。

本文集中管理选型依据与候选版本，[ADR 0007](../adr/0007-technology-stack-and-cpf.md)记录版本线与 CPF 决策，[ADR 0008](../adr/0008-route-selection-and-distribution.md)记录路线选择与分发决策，[ADR 0014](../adr/0014-swift-production-baseline.md)明确 Swift 生产能力与公开交付基线，[路线目录](../catalog/routes.json)记录路线身份、阶段和认证引用。本文中的候选、参考工程结果和官方发布事实均不自动形成 MadB 认证。

## 四条版本线

| 版本线 | 范围 | 管理边界 |
|---|---|---|
| 官方 Kotlin | Android、iOS、JVM 的 Kotlin/KMP 和可选 CMP | 官方 KGP、KSP、Apple 增强与平台依赖在同一配置中验证 |
| 鸿蒙 CPF | CPF Kotlin、akinterop、可选 CPF CMP | 独立 Gradle 根、插件环境、Konan 缓存与目标依赖 |
| 原生实现与绑定 | Rust、C/C++、UniFFI、Gobley、CXX、平台 ABI | 领域实现与 FFI 导出分离，生成器与导出二进制配套 |
| MadB 工具自身 | CLI、模型、规则、调度、适配器和模板 | 可以独立升级，不被某个后端的旧依赖整体牵制 |

平台 SDK 是各条路线的目标约束。宿主能运行 MadB CLI，不表示该宿主能构建所有目标。消费预编译 SDK 的开发者只安装其消费端需要的工具与依赖。

## Kotlin 与 CPF 配套

| 组件 | Android / iOS / JVM 候选 | 鸿蒙 CPF 候选 | 约束 |
|---|---|---|---|
| Kotlin / KGP | 2.4.20 | 2.2.21-1.1.0 | 官方与 CPF 插件环境分离 |
| 共同编译单元的 languageVersion / apiVersion | 2.2 | 2.2 | 平台独占模块可以使用所属工具链支持的能力 |
| Compose Multiplatform | 1.12.2，可选 | 1.9.2-1.1.0，可选 | 仅共享逻辑时不强制依赖共享 UI |
| Compose 编译插件 | 2.4.20 | 2.2.21-1.1.0 | 跟随各自 Kotlin 编译器 |
| kotlinx-coroutines | 1.11.0 | 1.10.2-1.0.0 | 共享代码只用双方支持的 API |
| kotlinx-serialization 运行库 | 1.11.0 | 1.9.1-1.0.0 | 序列化编译插件另随 Kotlin 配套 |
| atomicfu | 0.31.0 为原候选；C06 Native 配套采用 0.32.1 | 0.31.0-1.0.0，按需 | 精确区分官方制品与 fork 制品，按实际依赖闭包锁定 |
| KSP | 2.3.12，KSP2 | 2.2.21-2.0.4，当前 akinterop 配置使用 KSP1 | 前端适配器分别编译与运行 |
| Gradle | 9.7.1，优先验证 | 8.14.6 | 不将 akinterop 插件直接搬入 Gradle 9 环境 |
| 构建 JDK | 21 LTS | 21 LTS，参考 MadDemo 记录 | 锁定发行商、补丁与构建号；CPF 官方入门说明使用 JDK 17 |
| Swift 增强 | SKIE 0.10.15；NativeCoroutines 1.0.6 按需 | 不加载 | Apple 编译插件不进入 CPF 构建 |
| ArkTS 互操作 | 不加载 | akinterop 1.1.0 配套组件 | annotation、runtime、KSP 与 Gradle 插件一起锁定 |

官方 Kotlin 当前稳定版为 2.4.21。首轮采用 2.4.20，是因为 SKIE 0.10.15 明确增加该版本支持，NativeCoroutines 1.0.6 也以该版本构建；2.4.21 进入后续升级配置。[Kotlin 兼容矩阵](https://kotlinlang.org/docs/multiplatform/multiplatform-compatibility-guide.html)、[SKIE 0.10.15](https://skie.touchlab.co/changelog/0.10.15)、[NativeCoroutines 1.0.6 配置](https://raw.githubusercontent.com/rickclephas/KMP-NativeCoroutines/v1.0.6/gradle/libs.versions.toml)

官方线其余候选依据：[KSP 发布记录](https://github.com/google/ksp/releases)、[Compose 兼容说明](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)、[协程](https://github.com/Kotlin/kotlinx.coroutines)、[序列化发布记录](https://github.com/Kotlin/kotlinx.serialization/releases)。CPF 官方文档列出 KMP 2.2.21-1.1.0 / CMP 1.9.2-1.1.0 发布线；细分依赖组合参考 MadDemo 配置，来源见文末。[CPF 文档](https://gitcode.com/CPF-KMP-CMP/docs)

### 共享源码与依赖交集

共享源码在两条构建线上分别编译。语言/API 级别设为 2.2，并不能解决第三方库 API、KLIB 元数据、编译插件及平台变体差异，仍需双线编译和运行验证。不能直接把官方新编译器生成的 KLIB 交给旧 CPF 编译器消费。

官方与 CPF 依赖按明确映射管理。需要时同时映射 group、artifact、version 和目标变体，不只替换版本字符串。对缺少根元数据模块、仅发布 OHOS 平台模块的依赖，使用明确例外；禁止全局强制版本掩盖错误制品。

首版官方线与 CPF 线采用**独立发布坐标和独立根元数据**，共享源码不等于共享二进制身份。发布计划分别记录 Maven group/artifact、版本、Gradle module metadata、KLIB 编译器身份和目标变体，并在全新消费者中验证依赖解析。不得让两条流水线向同一坐标覆盖上传不兼容元数据；合并发布坐标属于以后需重新验证的独立决策。[KMP 发布与根元数据](https://kotlinlang.org/docs/multiplatform/multiplatform-publish-lib-setup.html)

CPF 1.1.0 发布线并不要求每个组件都带 -1.1.0 后缀。协程、序列化、atomicfu、Koin 等使用发布线实际配套的制品，不按后缀推断兼容性。

### 构建隔离与生成器接入

官方与 CPF 使用独立 Gradle 根和插件 classpath；同一组业务源码可以由两个根引用。生成目录、build 目录、配置缓存及 Konan 缓存按配置身份隔离，避免两个编译器覆盖同一产物。

MadDemo 记录的 akinterop 插件会调用 Gradle 9 移除的 ProjectDependency.getDependencyProject()，其 OHOS 构建使用 Gradle 8.14.6 和 KSP1。该限制归属于已核对版本，后续升级需要重新验证。[Gradle 8.14.6 发布说明](https://docs.gradle.org/8.14.6/release-notes.html)、[KSP 现行支持说明](https://github.com/google/ksp)

MadB 注解包保持轻量，不直接绑定 SKIE、akinterop 或 KSP 实现。共享契约提取逻辑，KSP1/KSP2 前端适配分别编译；CPF 流程不强制加载 MadB 官方线的 KSP2 插件。最终业务代码保持其目标 Kotlin 级别，KSP 任务的兼容参数不得误设为整个工程的语言基线。

## 平台工具链

| 平台 | 首轮候选 | 发布约束 |
|---|---|---|
| Android | AGP 9.3.3 + Gradle 9.7.1 + JDK 21 | KMP 库采用新 Android-KMP 插件；App 与需要 externalNativeBuild 的模块独立 |
| Android Native | NDK r30，30.0.16248370 | 固定完整版本；全部原生库通过目标 ABI 与 16 KB 页检查 |
| Android SDK | compileSdk 37 | minSdk、targetSdk 及 Java 字节码目标分别决策 |
| Apple | Xcode 26.4.1 + Swift 6.3 编译器 | Swift 语言模式、最低系统、设备与模拟器分别登记；C06 选定 macOS 配置采用下文的独立实测锁 |
| 独立 C/C++ 工程 | CMake 4.4.4 + Ninja 1.13.2 | 平台 SDK 有独立要求时使用该路线配置，不替换 SDK 自带工具 |

Kotlin 官方表当前列到 Gradle 9.7.0、AGP 9.3.1。采用 9.7.1 与 9.3.3 补丁候选是为了验证其修复后的组合，不能标记为官方已经认证。AGP 9.3 要求的 Gradle 9.5.0 是最低版本，不是唯一可选版本。[Gradle 9.7.1](https://docs.gradle.org/9.7.1/release-notes.html)、[AGP 9.3](https://developer.android.com/build/releases/agp-9-3-0-release-notes)

其他依据：[NDK r30](https://developer.android.com/ndk/downloads)、[Android 页大小](https://developer.android.com/guide/practices/page-sizes)、[Xcode 配套](https://developer.apple.com/xcode/system-requirements)、[CMake](https://cmake.org/download/)、[Ninja 1.13.2](https://github.com/ninja-build/ninja/releases/tag/v1.13.2)。

最低系统按实际启用的能力确定。Swift 编译器版本、语言模式、SwiftPM tools-version 与最低系统分别记录；不能因为可选 UI 宏需要新系统，就隐式抬高全部 SDK 的要求。Swift C++ 的特定能力也有独立部署限制。[Swift C++ 支持范围](https://www.swift.org/documentation/cxx-interop/status/)

### Android 工程结构

AGP 9 候选的 KMP 库采用 `org.jetbrains.kotlin.multiplatform` + `com.android.kotlin.multiplatform.library`；Android App 使用独立 `com.android.application` 模块。新模板不以旧 KMP + `com.android.library` 的兼容开关作为长期默认。[官方插件说明](https://developer.android.com/kotlin/multiplatform/plugin)

原生制品按生产方式选择装配：

| 场景 | 模块结构 | 必需验证 |
|---|---|---|
| Rust/C/C++ 已由 MadB 构建并生成 `.so` | 通过 Android-KMP 插件支持的 `jniLibs` 输入接入预编译产物 | 任务依赖、ABI 目录、AAR 内实际条目、传递装载与 16 KB 页要求 |
| 需要 AGP 的 `externalNativeBuild` 驱动 CMake/ndk-build | 独立 `com.android.library` 原生模块，由 KMP `androidMain` 消费 | 原生构建参数、构建变体、Release/AAR 消费与避免重复打包 |

新 Android-KMP 插件不提供 `externalNativeBuild`，这不等于不能包装预编译原生库。Android/JVM 的 JNI/JNA `.so` 不能以 `androidNative*` Kotlin/Native 目标替代；两者运行时与消费方式分别管理。初始化显式配置所需单元与设备测试，不依赖新插件默认关闭的测试配置。[Sources API](https://developer.android.com/reference/tools/gradle-api/9.3/com/android/build/api/variant/Sources)

### Apple 分发与调用面

Kotlin/Native + SKIE 的源码共同构建与预编译 Framework/XCFramework 是不同消费配置。对外分发时在 SKIE 的 `build` 配置中显式调用 `produceDistributableFramework()`；本机编译成功不能替代可分发配置。该配置同时处理 Swift library evolution、静态 Framework 的 Clang 模块调试路径与相对源码路径；仍需在独立 SwiftPM/Xcode 消费端验证生产/消费工具链、符号和调试信息。[SKIE 分发配置](https://skie.touchlab.co/configuration/swift-compiler)

公开 API 包含自定义 cinterop 类型时，生成增强包装需要配置 `ClassInterop.CInteropFrameworkName` 并配套分发、链接对应 Framework。缺少配置时 SKIE 可能生成不可调用的占位声明，原始 Kotlin 声明仍可能可用；因此门禁必须编译并调用具体 Swift API，不能只检查 Framework 已生成。泛型参数中包含 lambda 等已知限制同样进入能力诊断与显式适配。[SKIE cinterop 限制](https://skie.touchlab.co/known-issues/cinterop)、[泛型 lambda 限制](https://skie.touchlab.co/known-issues/lambda-types-in-type-argument)

反向调用另外验证 Objective-C 可见协议或回调；SKIE 增强 Swift 体验不会使任意纯 Swift 声明自动可被 Kotlin/Native 导入。[Kotlin Objective-C 互操作](https://kotlinlang.org/docs/native-objc-interop.html)

### 鸿蒙参考环境与发布候选

| 组件 | MadDemo 本机参考环境 | MadB 发布候选 |
|---|---|---|
| DevEco / Command Line Tools | DevEco 6.1.1.268 | Command Line Tools 26.0.0.851 Release（Mac ARM64） |
| Native / ETS SDK | 6.1.1.115，API 24，Beta1 | 26.0.0.105，API 26，Release |
| Hvigor / OHOS 插件 | 6.24.1 | 实际套装两组件均为 6.26.8 |
| ohpm | 6.1.2.268 | 26.0.0.630 |
| Node.js | 18.20.1 | 24.14.1 |
| 用途 | 复现 MadDemo 历史案例 | 验证 CPF 1.1.0 + akinterop 后再形成正式支持配置 |

参考环境来自 2026-10-24 对安装包元数据的读取；SDK 的 Beta1 标记不能被 DevEco 的产品版本号掩盖。最初的 `.821 / Hvigor 6.26.4` 来自[华为官方配套表](https://developer.huawei.com/consumer/en/doc/harmonyos-releases/deveco-studio-new-features-2600)。2026-10-25 用户提供并独立安装的实际套装为 `.851 / Hvigor 6.26.8`，因此更新本次 C07 验证候选，不能保留旧锁冒充同一组合。[安装与摘要记录](../reviews/c07-formal-toolchain-observation.json)包含本地 ZIP SHA-256、组件元数据和验证边界；尚未取得该 ZIP 的厂商公布校验值。平台工具自身配套，不证明 CPF、akinterop、目标运行或发布包已经兼容。

CPF 自带 LLVM、Konan 依赖及 sysroot 同时进入锁。MadDemo 的 macOS Arm64 参考组合包含 cpf-llvm-19-aarch64-macos-dev-21 和 ohos-aarch64-6.0.2.640-04；该宿主专用组合不能推广到其他主机架构，也不能等同于应用的 compileSdk。

HarmonyOS 与 OpenHarmony 分别登记目标配置。设备最低版本由依赖 API 和真实目标测试确定，不把框架介绍页的最低版本自动转为 MadB 支持承诺。

## Rust 与绑定版本

L02 自然 Rust 值配方按 [ADR-0022](../adr/0022-native-value-recipe.md)使用已锁 syn 2.0.119 解析有限声明，rustc 1.99.1 提供实际类型见证，cbindgen 0.29.4 与目标 Clang 核对生成物理面；不使用同锁中的 syn 3.0.9、不扩大共同 Rust 支持。解析器及其精确依赖只在工具适配层，普通候选 SDK 不包含这些工具依赖。

Rust 领域实现与 FFI 导出模块分离，MadB 工具自身使用的 Rust 依赖不决定目标 SDK 必须采用的绑定版本。**当前不冻结覆盖全部语言的统一 UniFFI 版本**；先确认各后端的修复、能力与目标运行结果，再由配置选择器给出完整配套。

| 组合 | 依据与限制 | 选型 |
|---|---|---|
| 上游 UniFFI 0.32.2 | 生成器、scaffolding、生成代码与目标二进制配套 | Swift/JVM 独立路线的首轮候选，不据此推导 Gobley 或 ArkTS 后端兼容 |
| 已发布 Gobley 0.3.7 + UniFFI 0.29.4 | 上游版本对应，但异步回调模板保留已知双完成风险 | 仅用于复现与比较；撤回完整能力的统一 SDK 默认候选 |
| 修复后的 Gobley 配套组合 | 必须核对模板补丁、UniFFI 协议与对象/异步能力；精确版本或 commit 由 P03 结果确定 | Kotlin/Native 主路线的待验证候选；通过前不宣称完整支持 |
| MadDemo 修改版 Gobley 0.3.7 + UniFFI 0.32.1 | 调整元数据、句柄和回调布局；async 与 Rust Object 导出明确受限 | 实现经验与负例，不直接作为完整通用后端 |
| uniffi-bindgen-arkts 0.0.1-beta.2 | manifest 使用 UniFFI 0.31.0 范围，当前仓库 lock 为 0.31.2；生成 napi-ohos 适配层 | 复用既有 UniFFI 接口的独立实验候选，不能混用 0.29.x/0.32.x 绑定 |

上表的上游基线与 MadB 受管构建分开。C06 Swift/Rust 已使用 UniFFI 0.32.2 的受管 Swift cancellation 补丁，并验证 cancel/free 同步；补丁身份及适用源码闭包见 [C06 记录](../reviews/2026-10-24-c06-kotlin-swift-rust.md)。不能用未修改的上游模板推断该候选缺少取消，也不能将此固定配置推广为任意业务停止或排水保证。

[上游 Gobley 0.3.7 依赖](https://raw.githubusercontent.com/gobley/gobley/v0.3.7/Cargo.toml)、[UniFFI 0.32.2](https://docs.rs/uniffi/0.32.2/uniffi/)、[ArkTS 生成器 manifest](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/main/Cargo.toml)、[对应 lock](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/main/Cargo.lock)。

UniFFI 0.29.5 已修复 foreign future 可能重复完成的问题；Gobley 0.3.7 自有模板仍把 `handleSuccess(makeCall())` 放在会调用 `handleError` 的 catch 范围内。因此候选必须核对并回补**生成模板**，只升级 Rust runtime 或依赖数字不算修复。回归覆盖成功通知后异常、错误通知、取消竞争、重复完成与释放。该问题以源码与上游修复为依据，不能把本次源码审查当作 MadB 目标测试通过。[上游修复 #2733](https://github.com/mozilla/uniffi-rs/pull/2733)、[0.29.5 变更](https://raw.githubusercontent.com/mozilla/uniffi-rs/v0.29.5/CHANGELOG.md)、[Gobley 模板](https://github.com/gobley/gobley/blob/v0.3.7/crates/gobley-uniffi-bindgen/src/templates/ffi/Async.kt#L54)。

Gobley 官网列出 0.3.8 / UniFFI 0.29.5 对应关系，但核验时 GitHub 与 crates.io 最新实际发布仍是 0.3.7。未发布的主线只能按固定 commit、补丁摘要与独立能力证据接入，不能登记成稳定发布版，也不能假设升级依赖已同步修复自有模板。[官网对应表](https://gobley.dev/docs/bindgen/)、[实际发布记录](https://github.com/gobley/gobley/releases)、[注册表](https://crates.io/api/v1/crates/gobley-uniffi-bindgen)

同一组导出符号对应的 scaffolding、生成器、生成代码和二进制必须配套。选择器同时检查完整能力与已知修复；不为了统一版本而丢失能力，也不让业务开发者手工拼装不匹配的版本。多条导出层复用内核时另验符号、链接与唯一 owner。

MadB 优先调用 Gobley CLI，再显式组织目标构建。Gobley 0.3.7 上游 Gradle/Cargo 插件的 Konan target 映射不包含 OHOS，未知目标会报错；CLI 能生成 `nativeMain` 不能证明 CPF/OHOS 支持。P03 必须分别验证官方 Kotlin 与 CPF 的编译、cinterop、链接、对象、异步和回调，并记录 MadB 自己承担的 OHOS 构建适配。[CLI 说明](https://gobley.dev/docs/bindgen/)、[上游 target 映射](https://github.com/gobley/gobley/blob/v0.3.7/build-logic/gobley-gradle/src/main/kotlin/rust/targets/RustTarget.kt#L55)

UniFFI Kotlin/JVM 实际依赖 JNA。首轮选 JNA 5.19.1，Android AAR、JVM JAR、jnidispatch、类加载和 R8 一起验证。直接通过 JNA 调用 C 是另一种扩展用途。[UniFFI 模板](https://raw.githubusercontent.com/mozilla/uniffi-rs/v0.32.2/uniffi_bindgen/src/bindings/kotlin/templates/wrapper.kt)、[JNA 变更记录](https://raw.githubusercontent.com/java-native-access/jna/master/CHANGES.md)

## 十六条路线

以下十六个稳定路线 ID 与阶段沿用现有目录；新增技术候选属于相应路线的独立 backend profile，不自动增加稳定路线或取得认证。双向能力受具体接口、回调与导出范围约束，不表示任意语言模块都可直接互相导入。

| 路线 ID | 调用路线 | 后端与版本选择 | 关键验证 |
|---|---|---|---|
| KN-SWIFT-OBJC | Kotlin/Native ↔ Swift | Kotlin 2.4.20、Objective-C export、SKIE 0.10.15 | 类型、协程、线程和 Apple 制品 |
| KN-ARKTS-NAPI | Kotlin/Native ↔ ArkTS | CPF Kotlin 2.2.21-1.1.0、akinterop 1.1.0 | KSP1、Gradle 8、注册、回调 env、释放 |
| KJVM-RUST-UNIFFI | Kotlin/JVM ↔ Rust | UniFFI 0.32.2 独立候选；JNA 5.19.1 | 模板修复、回调单次完成、装载、混淆与 JVM/Android 差异 |
| KN-RUST-GOBLEY | Kotlin/Native ↔ Rust | 修复后的 Gobley/UniFFI 配套待 P03 冻结 | 模板补丁、完整能力；官方 Kotlin/CPF 分别编译、链接与运行 |
| SWIFT-RUST-UNIFFI | Swift ↔ Rust | 与二进制配套的 UniFFI 0.32.2；C06 使用受管补丁构建 | Swift 6、异步、对象释放；公开 XCFramework 配方另验，不推导其他后端兼容 |
| ARKTS-RUST-NAPI | ArkTS ↔ Rust | napi-ohos/derive/build/sys 1.2.0；ohrs CLI 1.5.0；UniFFI-ArkTS 为独立实验候选 | 每个组件独立锁定；目标 SDK、env、线程、取消与 Release 消费 |
| ARKTS-C-NAPI | ArkTS ↔ C | 目标 SDK Node-API，C17 编译基线 | C ABI、参数、缓冲区、回调所有权 |
| ARKTS-CPP-NAPI | ArkTS ↔ C++ | 目标 SDK Node-API + C++17 | RAII、异常、标准库与线程回投 |
| SWIFT-C-CLANG | Swift ↔ C | Xcode 配套 Clang importer | 头文件、module map、符号、内存契约 |
| SWIFT-CPP-INTEROP | Swift ↔ C++ | Swift 6.3 + Apple Clang，默认 C++17；源码直接互操作 / C ABI / ObjC++ 正式候选配置 | 按分发、方向与构建系统选择；类型、标准库、ABI 与部署限制 |
| KJVM-C-JNI | Kotlin/JVM ↔ C | JNI + JDK/NDK 对应工具链 | 签名、引用、线程附着、ClassLoader |
| KJVM-CPP-JNI | Kotlin/JVM ↔ C++ | JNI + C++17 适配层 | JNI 约束、异常截断与资源管理 |
| KN-C-CINTEROP | Kotlin/Native ↔ C | 对应 Kotlin 工具链的 cinterop | 官方/CPF 分开，头文件与 sysroot 固定 |
| KN-CPP-CABI | Kotlin/Native ↔ C++ | 明确 C ABI + cinterop | 不暴露 STL、C++ 异常与隐式所有权 |
| RUST-C-ABI | Rust ↔ C | bindgen 0.73.2 + cbindgen 0.29.4 | libclang、目标头文件、宏、布局与释放器 |
| RUST-CPP-CXX | Rust ↔ C++ | CXX / cxx-build 1.0.205 | 版本配套、桥接面与 C++ 编译环境 |

候选依据：[napi-ohos](https://docs.rs/crate/napi-ohos/1.2.0)、[ohrs 已发布版本](https://crates.io/api/v1/crates/ohrs)、[bindgen](https://docs.rs/bindgen/0.73.2/bindgen/)、[cbindgen](https://docs.rs/cbindgen/0.29.4/cbindgen/)、[CXX](https://docs.rs/cxx/1.0.205/cxx/)、[cxx-build](https://docs.rs/cxx-build/1.0.205/cxx_build/)。使用 cxxbridge-cmd 时与 CXX 配套；cbindgen 0.29.4 与 UniFFI 0.29.4 的数字相同不构成依赖关系。

原生工具链不强制统一 LLVM 版本。分别锁定 CPF LLVM、Rust 工具链、SDK Clang 和 bindgen 宿主 libclang；验证目标 ABI、标准库、链接与运行行为。Rust 的 OHOS target 仍需目标 SDK linker 和 sysroot。[Rust OHOS 说明](https://doc.rust-lang.org/rustc/platform-support/openharmony.html)

C++ 默认采用 C++17，C++20 作为单独能力配置。独立发布接口要明确 ABI 承诺；相同语言标准不能证明不同编译器、标准库或 CRT 的二进制兼容。

### Swift ↔ C++ 配置选择

`SWIFT-CPP-INTEROP` 下保留三类正式设计配置，均待认证；C ABI/ObjC++ 不是出现故障后才启用的临时降级。

| 配置 | 适用条件 | 构建与分发约束 |
|---|---|---|
| 源码直接互操作 | 同源构建、类型受支持，消费方接受 C++ interop 配置 | 锁 Swift/Clang、标准库、编译参数；SwiftPM 的 C++ → Swift 调用与 Xcode/CMake 单独判断 |
| C ABI 分发 | 独立二进制 SDK 优先候选；收敛公开 ABI 或避免公开 C++ 类型 | C++ 实现在库内，导出不透明句柄与 C 函数；显式所有权、错误与版本协议 |
| Objective-C++ 适配 | 独立 Apple SDK 需要对象模型、Objective-C 可见协议或现有 Cocoa 接口时优先 | Objective-C++ 隔离 C++，Swift 消费 Objective-C 可见面；管理 ARC 与原生所有权 |

SwiftPM 对 Swift 调用 C++ 的配置不能当作 C++ 调用 Swift 的构建支持证明；后者选择可生成并消费 Swift 导出头的 Xcode 或经验证的编译流程。公开 Swift 包启用 C++ interop 还可能要求下游同步启用，因此必须进入 API/制品兼容评估。具体支持以锁定工具链的双向消费者为准。[Swift 构建系统说明](https://www.swift.org/documentation/cxx-interop/project-build-setup/)、[类型与平台支持](https://www.swift.org/documentation/cxx-interop/status/)


## 语言体验与扩展

| 能力 | 选择 | 约束 |
|---|---|---|
| SKIE + NativeCoroutines | X01 按需组合 0.10.15 + 1.0.6 | 每个接口的协程桥只有一个生成责任方；允许不重叠的 SKIE 类型增强 |
| Combine / RxSwift | 系统 Combine；RxSwift 6.10.2 按需 | NativeCoroutines Kotlin/Swift 组件同版本；Rx 不进入全部 SDK |
| Swift 宏 | SwiftSyntax 随所选编译器锁定；603.0.1 只作为 Swift 6.3 的参考候选 | 当前受管 Swift 6.4 首片使用系统 Observation，不沿用 603 作为自定义宏锁；宏实施时另验宿主组合，普通 API 可独立使用 |
| Kotlin 注解生成 | 官方 KSP2 与 CPF KSP1 前端分开 | 不强迫旧处理器加载新 KSP；生成代码接受目标编译验证 |
| ArkTS 状态和装饰器 | 选定 ArkTS/ArkUI SDK 配套能力 | 不以 TypeScript 装饰器库代替目标编译器能力 |
| 官方 Swift export | 随 Kotlin 的独立实验配置 | 当前 Alpha，已有 suspend/Flow 映射且仅支持 direct integration；与 Objective-C export 分别验证 |
| KNOI | 独立扩展，具体 fork 与版本另行验证 | 不进入 CPF + akinterop 默认工程，不串联两个调用桥 |
| ANI | 固定 SDK、头文件、适配器版本或提交 | 不继承 Node-API 认证，也不自动新增 Kotlin/Native ANI 路线 |
| UniFFI-ArkTS | uniffi-bindgen-arkts 0.0.1-beta.2；独立 0.31.x 锁 | 生成 napi-ohos adapter；不继承其他 UniFFI 版本或 napi 直连配置认证 |
| JVM FFM | JDK 25 桌面配置 | 与 Gradle JDK 分开，不作为 Android JNI 替代 |
| Crubit | 固定 commit 及受支持 Rust 工具链 | 单独验证，不使用浮动版本 |
| Xcode 初始化 | XcodeGen 2.46.0 候选 | 初始化生成原生工程，后续按文件所有权更新 |

依据：[NativeCoroutines](https://github.com/rickclephas/KMP-NativeCoroutines)、[RxSwift 6.10.2](https://github.com/ReactiveX/RxSwift/releases/tag/6.10.2)、[SwiftSyntax 603.0.1](https://github.com/swiftlang/swift-syntax/releases/tag/603.0.1)、[Swift export](https://kotlinlang.org/docs/native-swift-export.html)、[KNOI](https://github.com/Tencent-TDS/KuiklyBase-components/tree/master/knoi)、[ANI](https://github.com/ohos-rs/ani-rs)、[FFM](https://docs.oracle.com/en/java/javase/25/core/foreign-function-and-memory-api.html)、[Crubit](https://github.com/google/crubit)、[XcodeGen](https://github.com/yonaskolb/XcodeGen/releases/tag/2.46.0)。

Swift 九项扩展的选择与替代方案集中在 [ADR-0015](../adr/0015-swift-extension-delivery.md)。L04 Observation 首片只使用 Apple Observation 与标准库，没有引入 SwiftSyntax、RxSwift 或 NativeCoroutines 包；系统 `@Observable` 的编译器实现不等于已交付自定义 MadB 宏。源码包的最低系统声明与当前已测试目标分别列在[包说明](../../sdk/swift/observation/README.md)，不能从清单中的部署版本推导所有目标已经通过。

ArkTS 的必要与扩展工具选择分别集中在 [ADR-0016](../adr/0016-arkts-production-baseline.md)、[ADR-0017](../adr/0017-arkts-extension-delivery.md)。本页维护版本、来源与锁定条件；上游源码能力、可获取发布制品和 MadB 已验收能力分别记录。KNOI、ANI、UniFFI-ArkTS 的固定提交或单项能力不替代独立目标与发布验收；L05 页面状态首片也不升级这些后端的支持状态。

SKIE 0.10.15 的 Flow 错误和 Combine Future 限制集中见[后端规范](06-backend-profiles.md)。不能以“没有 Combine”为理由选择另一工具，也不能把同名外观视为相同语义；组合按固定版本验证启动、订阅、错误、取消和共享，升级另验。

## CMP 可选组合

纯 Kotlin 逻辑与 ArkTS UI 工程不强制引入 Compose、Skiko、Navigation 或 Koin。需要共享 UI 时，以下 MadDemo 参考组合进入独立验证。

| CPF 组件 | 参考版本 |
|---|---|
| Compose 与 ArkTS 包 @cpf-kmp-cmp/compose | 1.9.2-1.1.0 |
| Skiko | 0.9.22.2-1.1.0 |
| Lifecycle | 2.9.4-1.1.0 |
| SavedState | 1.3.3-1.1.0 |
| Navigation3 | 1.9.2-1.1.0 |
| Koin，按模板需要 | 4.1.1-1.0.0 |

共享 UI 源码仍需在官方 CMP 与 CPF CMP 两条线上验证 API 交集。坐标组差异、仅有平台模块的依赖及渲染后端均进入配套清单。

MadDemo 的 fusion-renderer 配置含兼容符号桩，被调用时会 abort。它只能作为特定路径的过渡案例，不能无条件复制为 MadB 通用渲染实现。纯逻辑、共享 UI、混合 UI 分别验收；选择渲染后端时同步锁定 Compose、Skiko、ArkTS 包、桥接产物和补丁，并运行对应渲染用例。

## MadB 工程实现

| 模块 | 技术栈候选 | 职责 |
|---|---|---|
| CLI | Rust 1.99.1 / Edition 2024、clap 4.6.7 | 参数解析、命令与结果展示 |
| model / rules | serde 1.0.229、thiserror 2.0.21 | 纯模型、能力与兼容约束，不执行工具或读取全局环境 |
| plan / app | Rust；执行层按需 Tokio 1.54.1 | 构建任务图、调度、取消与日志 |
| 生成器适配器和 helpers | 独立进程与各语言工具 | 隔离 UniFFI、Gobley、KSP、akinterop 等生态依赖 |
| 模板 | MiniJinja 3.0.3 候选 | 工程骨架；语言代码生成采用结构化生成与编译验证 |
| SDK | 对应语言普通库与可选便利层 | 执行状态、异步、资源和诊断协议 |
| MadB-Lab | 平台原生测试与消费工程 | 验证发布制品、初始化工程和组合路线 |

依据：[Rust](https://blog.rust-lang.org/releases/)、[clap](https://docs.rs/clap/4.6.7/clap/)、[serde](https://docs.rs/serde/1.0.229/serde/)、[thiserror](https://docs.rs/thiserror/2.0.21/thiserror/)、[Tokio](https://docs.rs/tokio/1.54.1/tokio/)、[MiniJinja](https://docs.rs/minijinja/3.0.3/minijinja/)。

C01 将工具宿主 Rust 1.99.1 / Edition 2024 固定在项目 rust-toolchain.toml 中，不改变用户全局默认版本。文档与骨架检查使用 Python 3.14.4、Node.js 26.8.2，校验依赖固定在 requirements-dev.lock；这只定义开发检查环境，不构成下游 SDK 的运行要求。首期仅引入已使用的依赖，其余候选随所属工作包验证加入。

CLI 构建版本与生成 Rust SDK 的最低支持 Rust 版本分别管理。CLI/Lab 提交解析锁；可复用库采用明确兼容范围和最低版本测试，不要求所有下游消费者无条件固定每个传递依赖。

保持现有[模块与端口](02-modules-and-ports.md)：Adapter 隔离工具差异，Strategy 选择显式配置，依赖倒置保护核心模型。版本配置、任务图和诊断共享稳定模型，核心不引入平台编译器或业务框架。

## 版本锁与工程初始化

每个可发布配置至少锁定以下输入：

- 路线、方向、宿主、目标 OS/架构/ABI、最低系统和启用能力。
- 编译器、SDK、构建工具、发行商与完整构建号。
- 生成器、运行库、模板、IR/协议版本与生成参数。
- 依赖坐标、目标变体、下载来源、校验值与必要补丁。
- 原生头文件、宏、sysroot、标准库/CRT、链接方式及影响 ABI 的参数。
- 制品摘要、Lab 修订、执行环境、用例集合与验证报告。

保留默认配置、升级候选和明确有需求的兼容配置。仅认证实际验证过的组合，避免穷举所有版本排列。缓存与制品身份包含影响输出的输入，候选版本不能因配置名相同而复用旧认证。

初始化根据平台与能力选择完整配置。官方 Kotlin 与 CPF 各自拥有固定 Wrapper 和原生 IDE 入口；IDE 与 CLI 使用同一版本配置。生成器分别判断“可生成工程”“可在当前宿主导入”“可在当前宿主构建/运行”：初始化设计中的 Apple 候选 Xcode 26.4.1 要求 macOS 26.2–26.x；Android 候选 AGP 9.3 对应 Android Studio Quail 2（2026.1.2）或明确支持该 AGP 的后续版本，KMP 功能还按 IDE 插件与宿主单独验证；DevEco/HarmonyOS 使用自身宿主清单。缺目标工具链时可以生成明确标记的工程，不能报告构建就绪。对已有项目保留文件所有权与显式冲突处理，不通过修改用户全局工具设置解决依赖冲突。[Xcode 系统要求](https://developer.apple.com/xcode/system-requirements)、[Android Studio/AGP 配套](https://developer.android.com/build/releases/about-agp)

依赖获取支持来源核验、断点恢复与缓存校验；干净机器必须能够从公开依赖重建。参考工程的本地镜像可提供案例，但不能成为唯一下载来源。

## 组合路线与单一所有权

ArkTS 经 akinterop、Kotlin/Native、Gobley 调用 Rust，与 ArkTS 直接经 napi-ohos 调用 Rust，是不同接入链。Swift 直接调用 Rust 与 Swift 经 Kotlin 调用 Rust 同样需要分别装配。

如果两条链分别静态链接同一内核，可能产生两套状态、线程池或资源 owner。应用装配配置应明确内核实例归属、创建与关闭责任、库的链接与装载位置，以及句柄是否允许跨入口流转。不同后端的句柄不可仅因底层都为整数或指针而混用。

MadB-Lab 除单路线测试外，还必须验证共享内核的组合应用。仅两个局部桥接分别通过，不能推导完整链路通过。

## 验证优先级与配置冻结

风险验证尽早开展，阶段阻断按实际依赖确定。不存在“全部语言组合先通过，Rust/C 才能开工”的全局版本锁。

| 工作流 | 验证对象 | 冻结或阻断范围 |
|---|---|---|
| 首链必需 | P01 工具/协议、P02 Clang/bindgen/cbindgen、首链锁与直接双向调用 | C04/C05 的首链集成；S0 的扩展 schema/框架探针和模板契约门禁仍需完成 |
| 高风险提前探针 | P03 修复后的 Gobley/UniFFI 与 CPF、P05 CPF/akinterop/正式 SDK | 尽早识别可行性；分别阻断受影响的 S2 异步配置和 S3/OHOS 组合，不阻断独立 Rust/C 配置 |
| 平台配置 | P04/P07/P12 Apple、类型/协程与分发；P06 ArkTS env；P11 原生 IDE/Android 模块 | 对应 C06–C08、L03–L06 和 I04–I06 的具体配置集成与推荐 |
| 组合与公开交付 | P08 新环境消费；共享内核、双根发布、Release、最低系统 | I05 的组合验收及 C10/C11 发布；候选按实际证据分别冻结 |
| 独立扩展 | P13/P14/P15 等实验后端与远程制品 | 只阻断对应 X 工作包；不继承主线认证 |

DEC-11 采用“每条路线一个默认完整配置，按需追加兼容/升级候选”的稀疏矩阵。每个配置必须填明宿主、目标 triple/ABI、最低目标系统、运行时、工具与补丁、公开能力、方向、构建与分发形式、必需用例和证据级别。缺字段的展示配置不是可认证 profile，不能用全版本排列替代有需求的支持组合。

首链执行优先选择 macOS Arm64 宿主上的 `aarch64-apple-darwin`，分别运行 C 消费 Rust 和 Rust 消费 C；P02 另对 `x86_64-apple-darwin` 做布局/目标编译探针。后者未真实运行时只保留对应证据，不形成运行认证。该选择是实施起点，不是最低 macOS 支持承诺；准确 SDK、deployment target 和工具构建身份由 P01/P02 记录到首链锁后才能集成。

CPF 正式应用 SDK、编译 sysroot、官方 Kotlin 根与 CPF 根分别锁定。CMP 渲染只在选择共享 UI 的配置加入验证；NativeCoroutines 与 SKIE 的组合按 X01 独立验收。ArkTS-UniFFI 候选必须在设备执行其声明能力；无设备的构建 smoke 不能作为运行证据。[ArkTS 生成器烟测入口](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/scripts/ci/run_ohos_smoke.sh)

S5 仍要求十六条主线各有最低完整配置和全部适用必需项。某配置失败保留最小复现、缺口与阻断范围，通过 ADR 修正，不静默改后端、删异步能力或替换载荷。预览版本仅对明确列出的已验证子集负责。

## 参考工程与观察范围

以下是参考工作区的观察记录，读取日期为 2026-10-24，MadDemo 当时 HEAD 为 a8448d38b00410241cfd31faefecb9a2d6606467。工作区配置与本机安装信息分别列出，不能据此声称 MadB 已完成实现。

| 来源 | 可支持的事实 | 对 MadB 的作用 |
|---|---|---|
| MadDemo gradle/libs.versions.toml、docs/adr/ADR-004-ohos-release-line.md | CPF 配套、双构建根、依赖映射、Gradle 8 与 KSP1 经验 | 配置与负例输入 |
| MadDemo ohos/gradle.properties、tools/ohos-gradle.sh | KSP1、独立 Konan 与渲染配置 | 构建隔离与兼容探针 |
| MadDemo kernel/tools/bindgen/Cargo.toml、FORK-PROVENANCE.md | 修改版 Gobley 适配 0.32.1，async/Object 导出受限 | 补丁身份与能力测试 |
| MadDemo tools/fusion-compat/fusion_stub.c | 特定渲染路径的 abort-on-call 兼容桩 | 独立渲染验收，不能复制为通用保证 |
| DevEco 安装包 Info.plist、工具 package.json、SDK oh-uni-package.json | 参考工具版本与 SDK Beta1 标记 | 区分历史环境和正式发布候选 |

这些路径仅定位参考事实，不是新项目的构建依赖。公开发布时用 MadB-Lab 可独立复现的案例与报告承接证据。

## C03 首链实测锁

2026-10-24 的 P02 使用 Xcode 27.0 (27A266a)、macOS SDK 27.0 (26A425)、Apple Clang/libclang 21.0.0 (clang-2100.3.34.2)、Rust 1.99.1 及其 llvm-tools、bindgen 0.73.2、cbindgen 0.29.4；deployment target 为 14.0。此组合只冻结 Rust/C stateless 探针，不替换其他 Apple/Swift 路线的 Xcode 26.4.1 候选，也不构成最低系统支持或 SDK 认证。

[可复现配方](../../tests/integration/c03/README.md)固定生成器源归档、Cargo.lock 和安装 receipt；运行记录覆盖工具、SDK 标识、所用头文件与标准库摘要、精确命令和产物。Rust archive 使用匹配的 llvm-tools，因为 Apple nm 无法解析其中的较新 LLVM bitcode；不忽略检查失败。两目标、两方向的真实 project/profile/lock 采用既有 v1 schema；[证据](../reviews/c03-native-evidence.json)保留观察范围。生产环境的完整依赖发现和 frozen 执行由 C04/C10 接续。

## C06 四路线实测锁

C06 的 `kotlin-jvm-rust`、`kotlin-native-rust`、`swift-rust`、`swift-kotlin` 四个固定 SampleEngine 配置使用 Xcode 27.0（27A266a）、Swift 6.4、macOS SDK 27.0（26A425）、Kotlin 2.4.20、Gradle 9.7.1、Temurin JDK 21.0.10+7、Rust 1.99.1 和 Python 3.14.4。UniFFI 0.32.2、Gobley 0.3.7 / UniFFI 0.29.4、SKIE 0.10.15 按路线分别锁定源码及受管补丁，不能混用。精确安装、依赖和工具镜像身份以[公开复现指南](../examples/managed-routes/README.md)及实际候选 profile/lock 为准。

C06 的 coroutines 1.11.0 Native 运行依赖使用 atomicfu 0.32.1，见[受管运行依赖锁](../../adapters/toolchains/kotlin_rust/runtime-dependencies.json)及[上游固定 1.11.0 标签](https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/1.11.0/gradle.properties)。这不改写 CPF 或其他候选配置；SKIE 插件自身的不同依赖作用域继续以其实际锁为准。

本轮本机四路线正式构建、54 个必需案例和 92 项 Lab harness 检查已通过；54 项包括 51 项目标运行及 3 项预期编译/链接拒绝。实际结果与源身份见[C06 记录](../reviews/2026-10-24-c06-kotlin-swift-rust.md)；跨机器验证与正式发布继续按各自证据确认。

此实测锁只适用于本轮 macOS Arm64、未插桩 Release 的四个固定 SampleEngine 配置，不替换其他 Apple 路线、iOS、初始化或 Swift 宏任务的候选版本。deployment target 14.0 是编译/链接参数，不代表已在该最低系统上运行。独立业务声明生成、普通公开 Swift SDK 与正式包渠道仍按生产基线推进，选定 profile 的局部结果不等于整条路线或公开发布已经认证。
