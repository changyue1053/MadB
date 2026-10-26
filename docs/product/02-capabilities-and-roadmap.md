# MadB 能力范围与认证模型

[路线目录](../catalog/routes.json)是主线路线与实施阶段的权威源，包含九组语言边界、十六条路线。当前所有条目均为 planned，认证列表为空。

## 能力单元

一个可认证单元由以下信息共同确定：调用路线、方向、源与目标运行时、目标 OS/ABI/架构、编译器及 SDK 版本、绑定后端及补丁、公开类型集合、同步或异步特征、运行时策略、构建系统、打包/分发形式，以及生成器补丁身份。宿主工具、IDE 与目标 SDK 分开锁定。

采用稀疏登记：只列实际设计和测试过的组合。通过某个 macOS 样例不隐含支持 Linux、Windows、iOS 真机或鸿蒙；宿主机与目标平台是不同维度。向回调接口发起反向调用，不隐含可以导入任意对方语言模块。

## 主线和扩展线

| 主线组 | 主要入口 | 关键分支 |
|---|---|---|
| Kotlin/Native 与 Swift | Objective-C 导出、SKIE 适配 | 双向宿主接口；模型直接使用、Facade、快照 |
| Kotlin/Native 与 ArkTS | CPF-KMP-CMP + akinterop 主线 | 类型化值、受检消息载荷、字节缓冲；CMP 按需 |
| Kotlin 与 Rust | UniFFI、Gobley | JVM 和 Native 分开认证 |
| Swift 与 Rust | UniFFI Swift | 原生 async/流包装与 Swift 并发隔离 |
| ArkTS 与 Rust | ohos-rs / napi-ohos | Node-API 环境、引用与停止协议 |
| ArkTS 与 C/C++ | Node-API 适配 | C 过程接口、C++ 对象封装 |
| Swift 与 C/C++ | Clang importer；受支持源码配置使用 Swift C++ interop | direct-source 与公开 SDK 的 C/Objective-C++ 包装分开；按构建系统和方向认证 |
| Kotlin 与 C/C++ | JVM JNI；Native cinterop | JNI 契约/描述符生成；C++ 经明确 C 门面供 Native 消费 |
| Rust 与 C/C++ | bindgen、cbindgen、CXX | 按方向和 ABI 稳定性选择 |

扩展线保留官方 Swift export、KMP-NativeCoroutines 的 Async/Combine/RxSwift 组合、KNOI、ANI、JDK FFM/JNA、Crubit、UniFFI ArkTS beta、更多平台及专用 IDE 向导/深度插件。它们使用同一套契约与认证机制，不能用主线证据替代独立验证。UniFFI Kotlin/JVM 主线自身的 JNA 运行依赖仍随主线交付，与直接 JNA 调用 C 的扩展用途区分。

[工程一键初始化与原生 IDE 接入](04-project-initialization.md)随 S1 至 S4 对应平台主线交付，S5 完成推荐模板矩阵。模板认证还包含模块组合、宿主环境与 IDE 版本；语言路线通过不自动证明组合应用可导入、构建或运行。

新增语言同样属于可独立接入的扩展线，按[扩展规范](../architecture/11-language-extension.md)声明语言、运行时、路线和具体目标。当前 Dart Native→C ABI 仅作为扩展性探针和格式示例，不增加主线计数或已认证组合；正式纳入产品范围时另行登记路线与实施阶段。

鸿蒙 Kotlin 默认采用 CPF-KMP-CMP + akinterop；官方 Kotlin 与 CPF 分开构建，CMP 是共享 UI 的可选组合。具体候选、公开制品与平台 SDK 验证条件见[技术栈选型](../architecture/16-technology-stack-and-versions.md)和 [ADR-0007](../adr/0007-technology-stack-and-cpf.md)。KNOI 保留为独立扩展。选型失败必须留下探针结果并更新 ADR，不静默更换后端。首版 SDK 的官方/CPF 构建采用独立 Maven GAV 及根 metadata，模板显式选择；独立编译本身不能证明同一坐标可以同时发布。

ArkTS 普通 SDK 以 [ADR-0016](../adr/0016-arkts-production-baseline.md)规定的 HAR 配方交付；必要能力与扩展的选型、替代方案及归属集中在该 ADR 和 [ADR-0017](../adr/0017-arkts-extension-delivery.md)，不在本页维护另一份清单。消费者未选 UI、装饰器或其他便利入口时，基础 API 仍可用；L05 对其产品承诺仍需提供相应实现、等价普通用法及验收。替代后端沿既有 S6 工作包独立准入，不继承 Node-API 主线证据。

## 场景与准入约束

- Android 共享模块采用 AGP 新 KMP 插件，app 独立；已有 native 库按 `jniLibs` 配方打包，需要 `externalNativeBuild` 或多 variant 时使用独立 Android library。发布 SDK 的消费端不应临时复制库。
- SKIE 接口必须在 Swift 中实际可调用；跨机器分发 framework 另启用分发配置。custom cinterop 与 lambda 泛型参数限制由能力检查和显式适配处理。
- Gobley 的旧 foreign future 模板必须先处理双完成风险并回归，再准入其异步配置；只升级 Rust runtime 不代表生成模板已修复。
- Swift C++ profile 同时指定构建系统与方向。SwiftPM 当前的 C++ 互操作不能当作 C++ 直接消费 Swift 的证据；源码 direct 与 C/ObjC++ SDK 包装分别验收。
- UniFFI ArkTS beta 由 X07/P13 独立探针；使用自己的 UniFFI 版本配置。它既不自动成为第十七条主线，也不继承 napi-ohos 的运行证据。

这些是具体配方和验收约束，不要求消费者学习底层实现。版本、官方来源与已知限制集中在[技术栈选型](../architecture/16-technology-stack-and-versions.md)。

## 三种状态分别记录

| 维度 | 值与含义 |
|---|---|
| 范围 | mainline / extension：产品承诺的实施位置 |
| 实现 | planned / partial / implemented：代码完成程度 |
| 验证 | missing / failed / passed / stale；附 environment 和 evidence_level |

以上是能力汇总状态；逐用例执行结果使用[统一结果分类](../validation/01-testing-strategy.md)，不能把工作包状态、执行结果和证据新鲜度混用。

结果证据细分静态形状、宿主编译、目标编译、模拟执行、目标运行和设备运行。`skipped`、测试基础设施错误和能力不支持都是独立结果，不能计为 passed。静态检查零命中仅代表该检查未发现问题。

## 认证记录的约束

每条记录包含配置摘要、工具与运行时版本、后端摘要、SDK/编译器标识、产物摘要、Lab revision、用例集合、执行结果和日志位置。输入变更后证据根据依赖失效；没有依赖分析时按保守范围失效。

设备相关问题要求设备证据，例如真实装载、页面生命周期和平台线程行为。跨 Xcode、最低 OS、Release 裁剪等采用显式测试通道；所有最低版本在实验后写入版本锁，未实测前不声明最低版本支持。

## 扩展范围的实施归属

语言特性有明确的[范围与启用方式](../architecture/15-developer-ergonomics.md)：普通 API 和必要协议随主线交付；主线中的自定义宏/属性封装等由 profile 按需启用；S6 深度工具独立验收；Dart 只承担 E04/E05 的扩展机制探针。能力类别已有架构设计，不等于具体展开和 API 已冻结。详细设计按[阶段准入](../delivery/04-design-readiness-and-first-route.md)收敛。

路线目录的 `extension_work_packages` 将扩展方向映射到稳定工作包。KNOI 为 X08，远程制品为 X09，IDE 向导为 I07，深度插桩为 L08。扩展机制本身的 S0/S1 工作包与具体后端的 S6 验收分别统计；一项主线通过不自动完成这些扩展。

C11 按上述 ADR 和任务目录核对 ArkTS 产品承诺及验收前置；可先实施不依赖完整目标链的 L05 子范围，但不能以局部状态/页面订阅适配代替 C07、HAP/ArkUI、推荐模板或全部 L05 验收。实施先后与正式完成前置分开，不增加主线与扩展之间的完成依赖环。
