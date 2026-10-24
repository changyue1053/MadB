# ADR 0008 路线选择与 SDK 分发配方

状态：设计采用，具体组合须通过对应探针与消费者验收。日期：2026-10-24。补充 ADR-0005 和 ADR-0007，不改变十六条主线路线的编号与范围。

## 背景

同一语言对可能对应不同运行时、调用方向和制品形态。仅按语言或最新版工具选择，容易将源码共同构建能力、应用内运行经验和公开 SDK 可分发能力混为一谈。MadB 需要让使用者选择任务，由工具解析可执行且可验证的完整配置。

## 决策

1. **按五个维度解析配置。** 目标运行时、调用方向、接口能力、交付形态和构建系统共同约束 profile。已有 C ABI 可以继续复用；Swift C++ direct、C/ObjC++ 包装和已有 UniFFI 接口的 ArkTS 生成分别按场景选择。未知组合或能力不足明确拒绝，不自动换桥或降低语义。
2. **官方 Kotlin 与 CPF 首版独立发布坐标。** 两个构建根独立维护根 publication、目标变体和依赖映射，由初始化模板选择正确坐标；不得各自覆盖同一 GAV 的根 metadata。后续统一坐标须先建立单一发布编排并验证全部目标解析。[KMP publication 结构](https://kotlinlang.org/docs/multiplatform/multiplatform-publish-lib-setup.html)
3. **Android 明确构建和包装两种配方。** shared 使用 Android-KMP library 插件，app 独立。已构建 native 库经受管任务和 jniLibs 包装；需要 externalNativeBuild 或多变体时使用独立 Android library。host/device tests 显式开启。Android Native target 不代替 JVM/JNI 消费路径。[Android KMP 插件](https://developer.android.com/kotlin/multiplatform/plugin)
4. **Apple 配置按消费方式区分。** 受控源码共同构建可选择 Swift C++ direct；希望隐藏 C++ 构建条件的公开 SDK 优先采用 C/ObjC++ 边界及普通 Swift API。SwiftPM 不默认承诺 C++ 消费 Swift API，反向导出选择经过验证的 Xcode/自定义构建。SKIE 二进制发布显式开启分发设置，并测试公开增强接口可调用。[Swift 构建约束](https://www.swift.org/documentation/cxx-interop/project-build-setup/)、[SKIE 分发](https://skie.touchlab.co/configuration/swift-compiler)
5. **版本修复覆盖生成器本身。** Gobley/UniFFI 的完整异步候选须覆盖 foreign future 双完成模板修复；仅升级 runtime 或修改一次生成文件不构成修复。补丁、源码提交、模板和运行依赖共同锁定，具体候选与冻结顺序由[技术栈规范](../architecture/16-technology-stack-and-versions.md)维护。
6. **可选后端保持独立资格。** UniFFI-ArkTS beta 作为已有 UniFFI 接口复用的实验候选，不自动替换 napi-ohos，不新增已认证路线。ANI、KNOI、Swift export、Crubit 等沿用各自目标与验证要求，不继承其他后端证据。

## 取舍

独立坐标会增加模板映射，但避免两个编译器共同修改同一根 metadata。按场景维护多种配方会增加适配与消费测试，换来可明确说明的部署条件；不要求每个用户手工理解或组合这些配方。

不为此修改核心语言、建立新的万能后端或重复资源运行时。Profile 负责边界能力与工具约束，ProjectPlan 负责模块和构建接线，发布适配器负责制品及 metadata；它们共享解析结果和摘要。

## 验收与重新评估

对应工作包从[实施计划](../delivery/01-implementation-plan.md)跟踪：版本修复后的生成代码真实执行，Android 独立 SDK 消费，官方/CPF 根及目标依赖解析，SKIE 异机分发，Swift C++ 构建系统与方向约束，以及实验后端准确阻断不支持能力。

上游稳定版本、类型能力或平台发布机制改变时，先更新候选和探针，再评估默认值；发布文档领先于实际制品、命令成功但设备未运行、或编译通过但 API unavailable 都不能提升认证状态。
