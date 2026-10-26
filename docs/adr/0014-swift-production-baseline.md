# ADR-0014 Swift 生产能力与公开交付基线

状态：设计采用；实现、目标验证和发布资格分别验收。日期：2026-10-25。适用基线：0.9。关联 L01 设计子范围、[ADR-0008](0008-route-selection-and-distribution.md)、[ADR-0009](0009-implementation-contracts-and-gates.md)与[ADR-0012](0012-extension-composition-and-delivery.md)。

## 需求与现有边界

MadB 面向可公开发布、由第三方独立使用的工具与 SDK。Swift 的完成目标是：作者在明确支持的声明子集内定义业务，生成可维护的普通 Swift API，并通过公开包在干净工程中消费。绑定生成、SDK 运行、应用构建和公开发行是四种责任；工具发布与用户业务 SDK 发布仍是两个流程。

现有 [C06 证据](../reviews/2026-10-24-c06-kotlin-swift-rust.md)覆盖固定 SampleEngine 的选定 macOS Arm64 Release 配置，不证明任意业务声明、iOS、其他 Apple 目标、UI 增强或正式发布渠道可用。框架实际编译模式与消费者使用的 Swift 6 严格并发模式分别记录，不能由一方推导另一方的保证。Swift 生产基线以 iOS 真机与模拟器作为首发验证重点，具体目标、最低系统、工具链和交付承诺由选定 profile 固定；本文不增加最低 OS 或认证结论。

## 后端与交付选择

| 提供方或交付面 | 推荐方案与理由 | 其他方案的边界 |
|---|---|---|
| Kotlin/Native → Swift | Objective-C export + SKIE；复用已有类型、suspend 与 Flow 增强，MadB 补必要业务接线和明确协议 | 官方 Swift export 保留 X02 独立 Alpha 配置；其当前能力与集成方式另验，不沿用早期限制或 SKIE 证据 |
| Rust → Swift | UniFFI；复用既有导出元数据、绑定、ABI/checksum 与对象机制，配套锁定生成器、scaffolding、runtime 和补丁 | 既有 C ABI 可由 Swift/C 配方消费，但不把两段桥接自动记为 Swift/Rust 直连支持 |
| C → Swift | Clang importer + 明确的 C 公开面；由类型、所有权、错误及 callback/context 适配建立普通 API | 导入头文件不等于已证明指针有效、释放正确或任意回调可跨线程 |
| C++ → Swift | 面向独立 SDK 优先选 C ABI 或 ObjC++ 包装，分别适合收敛 ABI 与 Apple 对象/协议集成 | direct-source 保留正式独立 profile；公开 C++ 类型、构建系统、调用方向和下游 interop 条件须显式声明 |
| 公开 Swift 包 | 优先采用 SwiftPM 包装选定 XCFramework 与必要 Swift 源码的混合配方；原生二进制、后端已生成模块和外层适配保持一致身份 | 纯源码或其他渠道另列分发 profile；SwiftPM 不自动生成正确切片、完成链接或验证运行能力 |

选择依据是复用已承担相应职责的生态机制，并将 MadB 的新增责任限制在契约、必要适配与交付验证。不能因为外层统一为 Swift API，就把四种后端的模型、资源协议或 ABI 检查改成同一实现。[SKIE 特性](https://skie.touchlab.co/features/)、[UniFFI 生成](https://mozilla.github.io/uniffi-rs/latest/tutorial/foreign_language_bindings.html)、[Swift/C++ 支持范围](https://www.swift.org/documentation/cxx-interop/status/)、[Swift export](https://kotlinlang.org/docs/native-swift-export.html)。

具体工具版本仍由[技术栈与版本选型](../architecture/16-technology-stack-and-versions.md)及实际锁维护；以上选择不将上游最新功能追认到当前锁定版本。分发与 C++ 配方的完整条件由[后端规范](../architecture/06-backend-profiles.md)和[构建打包规范](../architecture/09-build-packaging-release.md)维护。

## 十二项必要能力及责任映射

下列序号仅是本次讨论索引，不是新门禁 ID、任务或进度字段。“必要”针对所选 profile 承诺的接口和目标，不要求所有可选框架或任意工具组合同时实现。验收继续引用现有 CRIT、INIT、DX 与路线案例。

| 序号与能力 | 必须形成的交付结果 | 权威规范 | 既有工作包 |
|---|---|---|---|
| 1 明确支持范围 | 后端/版本、方向、目标、声明子集、构建和分发方式可查询；不支持项准确拒绝 | [后端配置](../architecture/06-backend-profiles.md)、[技术栈](../architecture/16-technology-stack-and-versions.md) | C06、C08、C10 |
| 2 真实业务声明生成 | 复用真实声明与显式策略，新增业务可重生成；固定样例映射不冒充通用能力 | [契约权威源](../architecture/03-contract-and-ir.md)、[生成](../architecture/07-code-generation.md)、[普通 API](../architecture/15-developer-ergonomics.md) | L01–L04、C06、C08 |
| 3 普通 Swift API | 业务方法、类型、错误和必要接线完整；无须靠 raw/native 或手工摘要补齐普通调用 | [数据与错误](../architecture/04-data-and-errors.md)、[语言体验](../architecture/15-developer-ergonomics.md) | L01、L04、L06 |
| 4 并发与线程安全 | 每入口的 actor/执行器、阻塞性和跨线程值保证有依据，不靠注解制造安全 | [生命周期与并发](../architecture/05-lifecycle-and-concurrency.md)、[SDK 运行时](../architecture/08-runtime-and-idiomatic-api.md) | C06、C08、L04 |
| 5 资源所有权与关闭 | owner、别名、保活、停止准入、排水及释放后条件明确 | [生命周期与并发](../architecture/05-lifecycle-and-concurrency.md) | C06、C08、L04、L06 |
| 6 异步与取消接线 | 区分取消等待、停止工作与业务撤销，迟到结果和完成通知有唯一责任 | [生命周期与并发](../architecture/05-lifecycle-and-concurrency.md)、[SDK 运行时](../architecture/08-runtime-and-idiomatic-api.md) | C06、C08、L04 |
| 7 状态与事件观察 | 状态/事件、冷/热源、共享、容量、停止完成和消费者作用域可解释 | [SDK 运行时](../architecture/08-runtime-and-idiomatic-api.md)、[生命周期与并发](../architecture/05-lifecycle-and-concurrency.md) | C06、C08、L04 |
| 8 宿主回调与反向调用 | 声明支持的反向形状、同步返回、执行上下文、重入与保活；不冒充任意反向导入 | [后端配置](../architecture/06-backend-profiles.md)、[生命周期与并发](../architecture/05-lifecycle-and-concurrency.md) | C06、C08 |
| 9 原生依赖与身份装配 | 依赖闭合、唯一物理装配、包/ABI/profile 检查及期望身份由公开装配提供 | [构建打包](../architecture/09-build-packaging-release.md)、[扩展组合](../architecture/17-extension-composition.md) | C06、C08、C10 |
| 10 正常工程构建与接入 | 普通 IDE/CLI 构建完成必要生成；新增、重命名、删除与已有工程更新可恢复 | [工程初始化](../architecture/14-project-initialization.md)、[生成](../architecture/07-code-generation.md) | I01、I04、I06 |
| 11 正式包交付 | 交付选定切片、模块、适配源码、依赖、许可和符号策略；从公开包安装可用 | [构建打包](../architecture/09-build-packaging-release.md) | C10 |
| 12 独立消费、兼容与发布验收 | 第二业务模块、干净消费、API/ABI/行为兼容、原候选发布和下载验收形成证据 | [Lab](../validation/02-lab-architecture.md)、[INIT](../validation/06-project-initialization.md)、[DX](../validation/07-developer-ergonomics.md)、[CRIT](../validation/08-critical-scenarios.md) | C09、C10、I06、L07、C11 |

资源、取消、观察和装配继续采用 DEC-07/08/09/10 及已有协议，不另造 Swift 专属资源内核、全局 owner 或平行门禁。E01–E06 承担这些配置所需的公开扩展与组合机制；Swift 能力表不替代扩展作者的独立接入和禁用验证。

## 普通 API、按配置增强与启动决定

普通 API 必须独立可用。通用运行原语可以手写维护；随业务声明变化的方法、模型投影、错误转换及必要资源接线按声明生成。优先保留后端已经正确表达的值与对象，只有类型安全、独立快照或协议确有需要时增加 Facade；不把所有对象复制成 struct。详细规则集中在[语言体验规范](../architecture/15-developer-ergonomics.md)。

Observation、SwiftUI/UIKit 适配、薄宏与属性封装保留 L04/I04 的按配置归属；没有 UI 的 SDK 不引入相应依赖。Combine/Rx 是消费方式，不预先绑定 NativeCoroutines；采用 SKIE + NativeCoroutines 时按 X01 逐接口选择唯一协程桥。Swift export 由 X02 承接，深度插桩/编译器插件由 L08 承接。SwiftSyntax、SwiftPM 插件、KMMBridge 等只在解决明确职责时选用，工具名称出现不等于具名功能已开工。

本决定原先仅作设计采用；后续 [ADR-0015](0015-swift-extension-delivery.md)记录九项扩展选择及项目所有者授权的实施推进，由受托实施负责人安排已选 Swift 子范围顺序，无需再次逐项申请开工。已有任务的 scope、phase 和验收依赖保留，必要交付细化到相应工作包，不追溯扩大已验收子范围。C07 按其既有范围独立推进，后续 ArkTS 必要能力与扩展已由 [ADR-0016](0016-arkts-production-baseline.md)、[ADR-0017](0017-arkts-extension-delivery.md)另行裁定并授权实施，不改变本决定的 Swift 范围。

## 实施与验证的交接

L01 先明确普通入口、支持子集和生成/手写分工；其余任务沿[现行计划](../catalog/implementation-plan.json)承接各自责任。正式实现前，将具体 API、生成形状、依赖、正反消费用例和目标限制放入对应工作包，不能把本文的能力表当作全部接口已定稿。

从固定样例走向产品的关键验收是：第二个独立业务模块在声明子集内增删改后，只通过公开输入与构建入口重生成并消费候选；不修改 MadB 私有映射、固定样例 Facade 或生成文件。普通配置、已承诺增强及允许组合分别验证；缺少目标、失败或未执行结果保持缺失，不借基础配置通过授予组合认证。

本决定按文档一致性和语义复核交付。目标构建、Swift 并发、资源交错、独立安装及发布检查由对应任务执行并绑定实际候选；本次文档修改不新增运行证据。
