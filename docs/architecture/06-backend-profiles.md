# MadB 后端接入方案

设计基线：0.9。后端复用互操作机制，MadB 补充契约、工程封装与真实消费验证。以下均为设计路线，未因此取得认证；版本与能力必须由探针和真实目标结果共同写入配置锁。

候选版本集中在[技术栈与版本选型](16-technology-stack-and-versions.md)。根据 [ADR-0007](../adr/0007-technology-stack-and-cpf.md)，鸿蒙 Kotlin 默认采用 CPF-KMP-CMP + akinterop，KNOI 为独立扩展；CMP 只在共享 UI 配置中启用。

## 主线路线

下表 ID 是稳定路线别名，具体 boundary profile 还必须固定目标、工具和行为策略，并遵循 [ADR 0008](../adr/0008-route-selection-and-distribution.md) 的选择与分发规则。协议 ID 对照在 [routes.json](../catalog/routes.json)；不得直接拿路线别名冒充已经解析的配置。

| 路线 ID | 实现机制 | 反向范围 |
|---|---|---|
| KN-SWIFT-OBJC | Objective-C export + SKIE | Objective-C 可见协议或回调 |
| KN-ARKTS-NAPI | CPF-KMP-CMP + akinterop；KNOI 为独立扩展 | 后端公开的代理或宿主回调 |
| KJVM-RUST-UNIFFI | UniFFI Kotlin | callback interface |
| KN-RUST-GOBLEY | 修复并验证后的 Gobley/UniFFI 配套 | 配置明确支持的 callback interface |
| SWIFT-RUST-UNIFFI | UniFFI Swift | trait/callback interface |
| ARKTS-RUST-NAPI | napi-ohos 直连；UniFFI-ArkTS 为独立实验后端 | environment-bound callback；以具体线程协议为准 |
| ARKTS-C-NAPI | Node-API C/C++ adapter | function + context callback |
| ARKTS-CPP-NAPI | Node-API C++ adapter | environment-bound callback |
| SWIFT-C-CLANG | Clang importer + C export adapters | C callback with context |
| SWIFT-CPP-INTEROP | 源码直接互操作 / C ABI / ObjC++ 三类正式设计配置 | 受支持 Swift export 或显式适配器，按构建系统验证 |
| KJVM-C-JNI | JNI + constrained generation | JNI registered callback |
| KJVM-CPP-JNI | JNI C++ adapter | JNI registered callback |
| KN-C-CINTEROP | cinterop + C export surface | C-compatible callback |
| KN-CPP-CABI | explicit C adapter + cinterop | C-compatible callback |
| RUST-C-ABI | bindgen incoming; cbindgen outgoing | 独立 C 提供方；另验证 function + context callback |
| RUST-CPP-CXX | CXX | CXX supported Rust/C++ surface |

## 工具职责

- bindgen 从 C/C++ 声明生成 Rust 底层绑定；复杂 C++ 语义不能仅靠布局导入获得安全包装。[官方说明](https://rust-lang.github.io/rust-bindgen/)
- cbindgen 从 Rust 的 C 可见接口生成头文件；导出实现、符号和所有权仍要正确存在。[项目文档](https://github.com/mozilla/cbindgen)
- CXX 用明确桥接面连接 Rust/C++，生成接线与一致性检查；原生对象的线程与别名前提由接口作者承担。[项目文档](https://cxx.rs/)
- UniFFI 提供 Rust 接口与外语绑定；Gobley 提供面向 KMP 的集成。二者按具体后端和目标分别认证。[UniFFI](https://mozilla.github.io/uniffi-rs/latest/)、[Gobley](https://gobley.dev/)
- Kotlin/Native 的 Objective-C 互操作和 cinterop 分别处理对应导出/导入面；纯 Swift 模块不因此自动成为可导入的 Kotlin 模块。[Swift/ObjC 互操作](https://kotlinlang.org/docs/native-objc-interop.html)、[C 互操作](https://kotlinlang.org/docs/native-c-interop.html)
- Swift C++ interop 按受支持的语言特征和工具链使用；C ABI 和 Objective-C++ 是有明确用途的补充路线。[Swift 文档](https://www.swift.org/documentation/cxx-interop/)

## 平台组合与配置选择

### Kotlin/Native ↔ Swift

默认验证 Objective-C export + SKIE。需要 Combine/RxSwift 的接口可以选择 KMP-NativeCoroutines；每个接口的协程桥只有一个生成责任方，不重叠的类型增强可以保留。分别检查公开名字、订阅启动、取消、共享和 Swift 6 并发约束。SKIE 已有 Combine 预览适配，不能以“没有 Combine”作为引入另一套生成器的理由。[SKIE Flow](https://skie.touchlab.co/features/flows)、[SKIE Combine](https://skie.touchlab.co/features/combine)、[NativeCoroutines](https://github.com/rickclephas/KMP-NativeCoroutines)

分发配置与源码共同构建配置分开：对外分发 Framework/XCFramework 时显式配置 `produceDistributableFramework()`，由独立消费者验证 Swift 模块、链接与调试信息。自定义 cinterop 类型配置 `ClassInterop.CInteropFrameworkName`；缺少配置可能产生不可调用的增强占位声明。门禁必须编译并调用实际公开 API，也覆盖泛型 lambda 等已知不可调用形态。[SKIE 分发](https://skie.touchlab.co/configuration/swift-compiler)、[cinterop](https://skie.touchlab.co/known-issues/cinterop)、[泛型 lambda](https://skie.touchlab.co/known-issues/lambda-types-in-type-argument)

官方 Swift export 仍作为独立 Alpha 配置，按当前文档核对 suspend、Flow、enum/sealed 等支持，不能沿用旧版限制，也不能替代 Objective-C 路线的既有认证条件。[Swift export](https://kotlinlang.org/docs/native-swift-export.html)

### Kotlin/Native ↔ ArkTS

默认使用 CPF-KMP-CMP + akinterop；CMP 仅在共享 UI 配置启用。先验证公开依赖、正式 SDK、设备运行与 Release/HAR 消费。官方 Kotlin 与 CPF 独立构建根、插件环境和生成目录，首版使用独立发布坐标与根元数据，并用同一契约进行双线验证。共享源码不证明 KLIB 或发布变体可互换。[CPF 文档](https://gitcode.com/CPF-KMP-CMP/docs)、[KMP 发布元数据](https://kotlinlang.org/docs/multiplatform/multiplatform-publish-lib-setup.html)

KNOI 单独配置并验证，不串联为 akinterop 的隐式下一层。Node-API 是运行机制，JSON 是载荷格式，Flow 是上层异步接口，三者相互独立；类型化 API、回调或二进制载荷仍按契约选择。[KNOI](https://github.com/Tencent-TDS/KuiklyBase-components/tree/master/knoi)

### Rust ↔ Kotlin/Swift

UniFFI 负责 Rust 导出和目标绑定；Kotlin/Native 使用 Gobley 适配，但不冻结“一套版本覆盖全部后端”的假设。已发布 Gobley 0.3.7 / UniFFI 0.29.4 保留作复现基线，撤回其完整能力默认候选：异步模板中成功回调后异常可能进入错误回调，必须核对并回补单次完成逻辑，再验证对象、异步、取消与释放。**仅升级 Rust runtime 不能修复生成模板。**[UniFFI 修复](https://github.com/mozilla/uniffi-rs/pull/2733)、[Gobley 0.3.7 模板](https://github.com/gobley/gobley/blob/v0.3.7/crates/gobley-uniffi-bindgen/src/templates/ffi/Async.kt#L54)

Kotlin/JVM 与 Swift 的 UniFFI 0.32.2 独立候选、修复后的 Gobley 配套及任何修改版后端，各自具有生成器、scaffolding、二进制、补丁与能力身份。Gobley 官网出现的 0.3.8 不能作为已发布稳定版使用；固定 commit 的组合必须明确其来源和验证范围。[Gobley 发布](https://github.com/gobley/gobley/releases)、[版本对应说明](https://gobley.dev/docs/bindgen/)

Gobley CLI 可以单独调用，但上游 0.3.7 Gradle/Cargo 插件的 target 映射没有 OHOS。MadB 负责的 CPF 构建适配要单独验证；生成 `nativeMain` 不能替代真实 CPF 编译、cinterop、链接和运行。MadDemo 修改版的 async/Object 限制不能转为 MadB 的完整能力保证。[上游 target 映射](https://github.com/gobley/gobley/blob/v0.3.7/build-logic/gobley-gradle/src/main/kotlin/rust/targets/RustTarget.kt#L55)

UniFFI Kotlin/JVM 自身依赖 JNA，装载、jnidispatch、Android AAR/JVM JAR 与混淆属于主线验收；这与直接通过 JNA 调用 C 的扩展用途分开。需要 JNI 专用对象或调用语义时单独评估显式 JNI 配置，不能无证据断言另一机制总是更快。

### ArkTS ↔ Rust

| 后端配置 | 目标用途 | 必需约束 |
|---|---|---|
| napi-ohos 直连 | 直接为 ArkTS 暴露 Rust API，复用原生类型、async 与回调接线 | napi、derive、build、sys 各 1.2.0，ohrs CLI 1.5.0 单独锁；只承诺目标 SDK 验证过的能力 |
| UniFFI-ArkTS 实验后端 | 为已有 UniFFI Rust 接口生成 ArkTS API 和 N-API Rust adapter | `uniffi-bindgen-arkts 0.0.1-beta.2`；独立锁 0.31.x（仓库 lock 为 0.31.2），不混用其他 UniFFI 版本 |

UniFFI-ArkTS 由 X07 扩展工作包独立推进，不改变十六条稳定路线与阶段。两个后端不自动互换。新生成器复用 napi-ohos 并链接既有 UniFFI 动态库；要额外验证序列化、库装载、对象、取消与多 env 清理。当前生成代码会拒绝异线程同步回调；custom types 等未覆盖声明应在生成前诊断，不隐式改变调用语义。[ohos-rs](https://github.com/ohos-rs/ohos-rs)、[ohrs 发布版本](https://crates.io/api/v1/crates/ohrs)、[ArkTS 生成器](https://github.com/ohos-rs/uniffi-bindgen-arkts)、[同步回调实现](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/tests/fixtures/generated/callbacks/native/src/lib.rs#L457)

普通 Node.js 测试、fixture 存在或 HAP 构建成功不能代表 ArkTS 运行通过。实验后端的烟测可以跳过环境或只完成构建，MadB-Lab 必须要求真实设备执行、用例完成与报告关联。ANI 是另一种运行时配置，不继承 Node-API 认证。[烟测脚本](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/scripts/ci/run_ohos_smoke.sh)

### Swift ↔ C/C++

Swift ↔ C 使用 Clang importer 和明确的 C 可见导出面。Swift ↔ C++ 先按源码/二进制、调用方向、构建系统和公开类型选择配置：

| 配置 | 选择条件 | 验收重点 |
|---|---|---|
| 源码直接互操作 | 同源构建、工具链一致、类型受支持，消费方接受 C++ interop | Clang 模块、标准库、部署要求及下游配置传播 |
| C ABI 分发 | 面向独立消费者的 SDK，希望收敛公开 ABI | 不透明句柄、错误/所有权协议、平台布局与版本兼容 |
| Objective-C++ 适配 | Apple 对象模型、Cocoa 协议或既有 Objective-C 接口 | Objective-C 可见 API、ARC/RAII 生命周期和异常截断 |

三类都是正式设计配置，均需认证；不把 C/ObjC++ 仅当直接互操作失败后的临时兜底。SwiftPM 对 Swift 调用 C++ 的支持不自动包含 C++ 调用 Swift；反向选择支持生成 Swift 导出头的 Xcode 或经验证的编译流程。开启 C++ interop 对下游配置及公开 API 的影响进入兼容检查。[Swift 构建系统说明](https://www.swift.org/documentation/cxx-interop/project-build-setup/)、[支持范围](https://www.swift.org/documentation/cxx-interop/status/)

### Kotlin 与 ArkTS ↔ C/C++

Kotlin/JVM 使用 JNI，明确签名、引用、R8、ClassLoader 与线程附着。Native 使用对应官方/CPF 工具链的 cinterop 和 C 可见边界。FFM 属于桌面 JVM 扩展，不能作为 Android 通用替代。

AGP 9 候选中，KMP 库使用 `com.android.kotlin.multiplatform.library`，Android App 独立。MadB 预编译 `.so` 通过受支持的 `jniLibs` 装配；需要 `externalNativeBuild` 的 CMake/ndk-build 模块独立使用 `com.android.library` 并由 `androidMain` 消费，不能把两类结构混写进模板。所需单元与设备测试显式启用。[Android-KMP 插件](https://developer.android.com/kotlin/multiplatform/plugin)、[Sources API](https://developer.android.com/reference/tools/gradle-api/9.3/com/android/build/api/variant/Sources)

ArkTS 使用平台 Node-API，分别验证 env、线程投递和销毁；桌面 Node API 文档中的行为不能当作目标 SDK 保证。[OpenHarmony 稳定性说明](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md)

## 平台特例不能被通用抽象抹掉

CPF + akinterop 的主线探针覆盖当前版本的 Gradle/KSP 限制、桥注册、依赖装载、参数校验、线程与 env 生命周期、Release 混淆以及可选 CMP 渲染。KNOI 扩展另行覆盖其依赖预载、sharedLib 注册、dlopen 后模块不可调用和 initBridge 查找问题。后端专属入口不能照搬到另一个后端；共同问题归入 BUILD-002、BUILD-003 和 CAP-004。[KNOI 使用与 FAQ](https://github.com/Tencent-TDS/KuiklyBase-components/blob/master/knoi/README.md)

OpenHarmony 的 napi_env、napi_value 和线程具有明确关联。持久引用不等于能跨线程使用；释放环境前的清理、异步回投和 handle scope 均在相应 profile 下验证，不能直接套用桌面 Node.js 的认证结果。[平台稳定性说明](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md)

## 后端必须交付的包

每个接入包包含：能力声明、契约读取方式、类型映射、策略限制、代码生成入口、工具链要求、运行时依赖、诊断映射、符合性套件和安装说明。支持清单区分工程生成、IDE 导入、宿主构建与目标运行；Xcode/macOS、Android Studio/AGP、DevEco/SDK 的配套随配置锁定，不以 CLI 可运行推导全部平台可构建。生成器失败输出可定位的源符号，不只返回通用非零退出码。

包可以引入新语言、给已有语言增加运行时或路线，也可以只提供复用组件。清单、组件引用、能力和版本的正式写法见[扩展规范](11-language-extension.md)与[扩展格式](../schemas/README.md)。提供方、消费方、反向范围和目标组合分别声明，不自动推导其他路线。

二进制发布与随源码共同编译采用不同 ABI 承诺。直接 C++ 桥通常要求共同工具链，不能宣称任意编译器间稳定 ABI；面向独立发布的 C 接口也需要目标布局、调用约定与版本协议。

## 降级规则

后端不支持的能力首先产生 CapabilityUnsupported。作者可以显式选择 Facade、C 适配器、受限类型或不同载荷，并得到新的契约摘要。运行时不得静默退到 JSON、吞掉未知字段或减弱取消/线程保证。

新增与升级后端均执行[接入验收](../validation/05-language-onboarding.md)。扩展自报能力、构建成功或两段桥接各自可用，都不能替代该组合的真实消费与生命周期证据。

## Swift export 独立配置的边界

当前上游文档列出 suspend→async、Flow→AsyncSequence、enum/sealed 等映射；不能沿用早期“不支持协程”的结论。同时它仍为 Alpha，当前工程集成限定 Xcode direct integration；泛型上界擦除、类与集合继承限制另做探针。X02 先验证这个具体配方，不默认套用 SKIE、NC 或 XCFramework/SwiftPM 的分发配方。[Swift export 文档](https://kotlinlang.org/docs/native-swift-export.html)

反向宿主能力未确认的形状在生成前拒绝。专题图只能画已声明机制，不能靠“回调条件待验证”的脚注为任意 Kotlin→Swift 调用画一条成功路径。KNOI 独立后端由 X08/P14 承接，C07 继续负责 CPF+akinterop 主线。

## Rust/C 的 S0 探针配置

C03 的两个 stateless provider 使用独立 C/Rust 消费者；Arm64 运行与 x86_64 编译分别保存结果。完整 engine ABI 只验证实际生成形状，不用桩函数伪装运行实现。具体配方、生成差异与运行层级见 [C03 记录](../reviews/2026-10-24-c03-native-probes.md)；该探针不增加路线、不降低 C05 的资源/关闭要求。

bindgen 的宏处理和默认 signedness 是配置的一部分：首链常量采用普通整数宏，固定 signed 生成策略，编译断言检查类型/值。Rust archive 的 bitcode 检查使用同 Rust 配套的 llvm-tools；Clang/libclang 与 C 链接器继续按 SDK 锁定。
