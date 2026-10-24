# MadB 参考资料与案例来源

参考分为两类：既有工程研究提供问题样本，官方项目资料提供工具机制依据。它们都不能替代 MadB 自身的实现与平台认证。

历史问题与九组研究阅读版见[追溯索引](history.md)；18 份文档集中在[研究归档](legacy-boundaries/README.md)，原文另有不可变快照。当前规范不要求读者先阅读旧项目分析。

## 机制参考

| 主题 | 主要资料 | 用途 |
|---|---|---|
| Rust 多语言绑定 | [UniFFI](https://mozilla.github.io/uniffi-rs/latest/)、[Gobley](https://gobley.dev/) | 界定生成与 KMP 集成职责 |
| Rust C/C++ | [bindgen](https://rust-lang.github.io/rust-bindgen/)、[cbindgen](https://github.com/mozilla/cbindgen)、[CXX](https://cxx.rs/) | 方向、声明、对象与桥接能力 |
| Kotlin Native | [Objective-C 互操作](https://kotlinlang.org/docs/native-objc-interop.html)、[C 互操作](https://kotlinlang.org/docs/native-c-interop.html) | 导出与导入约束 |
| Kotlin Swift 包装 | [SKIE](https://skie.touchlab.co/features/flows)、[NativeCoroutines](https://github.com/rickclephas/KMP-NativeCoroutines) | 类型与异步桥的责任划分 |
| Swift C++ | [Swift C++ interop](https://www.swift.org/documentation/cxx-interop/) | 编译器支持及工程边界 |
| Kotlin ArkTS | [CPF-KMP-CMP](https://gitcode.com/CPF-KMP-CMP/docs)、[KNOI](https://github.com/Tencent-TDS/KuiklyBase-components/tree/master/knoi) | CPF + akinterop 主线；KNOI 独立扩展 |
| ArkTS 原生 | [ohos-rs](https://github.com/ohos-rs/ohos-rs)、[OpenHarmony](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md) | 平台运行时与生命周期 |
| 工程规范 | [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)、[C++ Core Guidelines](https://isocpp.github.io/CppCoreGuidelines/CppCoreGuidelines) | 可读 API、接口与资源管理 |
| 语言生成与惯用 API | [KSP](https://kotlinlang.org/docs/ksp-overview.html)、[Swift 宏](https://developer.apple.com/videos/play/wwdc2023/10167/)、[ArkTS 规则](https://github.com/openharmony/docs/blob/master/en/application-dev/quick-start/typescript-to-arkts-migration-guide.md)、[Dart 生成](https://dart.dev/tools/build_runner) | 声明、语法与生成限制；完整选择和实例见[专题设计](../architecture/15-developer-ergonomics.md) |
| 状态收集与异步清理 | [Compose 状态](https://developer.android.com/develop/ui/compose/state)、[stateIn](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines.flow/state-in.html)、[awaitClose](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines.channels/await-close.html) | 区分 UI 收集、共享上游与停止确认 |
| 工程初始化 | [KMP IDE](https://kotlinlang.org/docs/multiplatform/recommended-ides.html)、[Gradle Wrapper](https://docs.gradle.org/current/userguide/gradle_wrapper.html)、[XcodeGen](https://github.com/yonaskolb/XcodeGen)、[CMake Presets](https://cmake.org/cmake/help/latest/manual/cmake-presets.7.html) | 原生工程入口与构建复用；具体设计和其他来源见[初始化架构](../architecture/14-project-initialization.md) |
| 开源交付 | [GitHub 许可](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository)、[社区文件](https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/creating-a-default-community-health-file) | 许可、贡献和问题反馈机制 |

具体版本与行为以实施阶段的锁、探针和目标证据为准，本文不把网页中的最新版本自动设为兼容基线。

2026-10-24 的完整候选版本与参考观察集中在[技术栈与版本选型](../architecture/16-technology-stack-and-versions.md)。该文区分官方发布信息、MadDemo 工作区事实、本机 Beta SDK 与 MadB 发布候选，避免把参考工程补丁或局部验证推广为通用支持。

## 平台机制与已知限制

| 主题 | 上游依据 | 对应设计与限制 |
|---|---|---|
| foreign future 重复完成 | [UniFFI 修复](https://github.com/mozilla/uniffi-rs/pull/2733)、[Gobley 0.3.7 模板](https://github.com/gobley/gobley/blob/v0.3.7/crates/gobley-uniffi-bindgen/src/templates/ffi/Async.kt#L54) | 修复生成模板与配套运行依赖；不只替换 runtime |
| Android KMP 工程 | [官方插件](https://developer.android.com/kotlin/multiplatform/plugin)、[jniLibs API](https://developer.android.com/reference/tools/gradle-api/9.3/com/android/build/api/variant/Sources) | 区分 shared、app、native 构建及预编译库包装 |
| KMP SDK 发布 | [根与目标 publication](https://kotlinlang.org/docs/multiplatform/multiplatform-publish-lib-setup.html) | 官方/CPF 独立坐标是本产品首版决策，不是上游强制要求 |
| Swift C++ 构建边界 | [SwiftPM/Xcode 配置](https://www.swift.org/documentation/cxx-interop/project-build-setup/) | 方向、依赖目标的 interop 要求与制品形态分别选择 |
| SKIE 分发与可调用性 | [分发配置](https://skie.touchlab.co/configuration/swift-compiler)、[cinterop](https://skie.touchlab.co/known-issues/cinterop)、[lambda 类型参数](https://skie.touchlab.co/known-issues/lambda-types-in-type-argument) | 公开增强 API 必须被真实消费者调用；编译成功不足以证明可用 |
| ArkTS UniFFI 生成 | [生成器](https://github.com/ohos-rs/uniffi-bindgen-arkts)、[烟测脚本](https://github.com/ohos-rs/uniffi-bindgen-arkts/blob/56018734abafc1de7e9d124badb3a33d49243f59/scripts/ci/run_ohos_smoke.sh) | beta 候选；fixtures 或无设备构建成功不代表目标运行通过 |
| 运行时与可选工具 | [ArkTS 运行时](https://developer.huawei.com/consumer/cn/doc/HarmonyOS-Guides/arkts-runtime-overview)、[autocxx 维护状态](https://google.github.io/autocxx/)、[JNI 指南](https://developer.android.com/ndk/guides/jni-tips) | Node-API 保留主线；不将 autocxx 改为默认；JNI 自动生成仍需确定受限契约 |

[来源映射](../catalog/source-coverage.json)将这些专项事实与既有 236 条历史问题分开登记，避免改变旧来源的含义或把工程决策推断写成已发生的线上事故。

## 类型与生命周期机制

- [Swift export 上游文档](https://kotlinlang.org/docs/native-swift-export.html)：核对当前协程映射、Alpha 状态、类型限制和 Xcode direct integration；不沿用 ObjC/SKIE 的假设。
- [Android JNI 指南](https://developer.android.com/ndk/guides/jni-tips)：核对线程专属 JNIEnv 与 Modified UTF-8，生成示例按机制区分。
- [协程及时取消与资源](https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines/suspend-cancellable-coroutine.html)：解释 resume 后尚未消费的资源仍需取消回收。
- [ohos-rs 的 ArkTS 生成器](https://github.com/ohos-rs/uniffi-bindgen-arkts)：明确为该社区维护的后端，不能称为 UniFFI 官方内建后端。

这些资料支持机制选择；队列票据、阶段准入及责任分配属于 MadB 自己的设计决定，不能作为上游自动提供的保证。

## 格式与构建机制

- [RFC 8785 JCS](https://www.rfc-editor.org/rfc/rfc8785)：规范语义投影的确定编码；输入和制品继续按实际字节校验。
- [Cargo 构建脚本](https://doc.rust-lang.org/cargo/reference/build-scripts.html)、[Gradle 增量构建](https://docs.gradle.org/current/userguide/incremental_build.html)：原生工具的输入声明与变化探测，MadB 仍收集完整依赖和任务身份。
- [UniFFI 变更记录](https://github.com/mozilla/uniffi-rs/blob/main/CHANGELOG.md)：后端版本与 checksum 变化；适配器优先复用上游匹配检查。

DEC-01 至 DEC-12 的规则以对应规范为准；网页 latest 不直接更新项目工具锁。
