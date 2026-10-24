# Kotlin/Native ↔ Swift 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

主线为 Objective-C export + SKIE。SKIE 与 KMP-NativeCoroutines 组合按 X01 验收；官方 Swift export 为 X02 独立 Alpha 配置。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 协程桥责任 | 同一符号的同一协程语义阶段只有一个责任方；NativeCoroutines 提供所选 async/Combine/RxSwift 入口时，禁止重复桥接，允许不重叠的 SKIE 类型增强。 |
| 模型策略 | 优先直接消费适合公开的导入类型；Facade 可复用原模型；独立 Swift 快照只在独立值语义、领域隔离等明确需要时生成，不强制三套模型。 |
| Swift export | 不能沿用早期不支持 suspend/Flow/enum/sealed 的笼统结论；按现行规范及探针验证映射。Xcode direct integration 配方不继承 ObjC/SKIE 分发资格。 |
| 并发与资源 | Swift Actor/Sendable、Kotlin 同步、ARC/GC 和原生关闭各有责任；页面只停止自身观察，共享引擎由其 owner 决定关闭。 |
| 便利语法 | 宏、属性封装器与 Observation 服务局部状态和接线；普通 API 始终可用，属性读取不隐式耗时调用。具体名称在对应模块集成前确定。 |
| 分发 | 源码共同构建与可分发 Framework 分开；SKIE 分发设置、cinterop 模块名、增强 API 实际调用和真机/模拟器各自验证。 |

路线身份：`KN-SWIFT-OBJC`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| MessageSeq? 的导出形状在真实 Swift 消费中暴露问题 | 按具体签名做受限投影或辅助方法，不把所有模型统一复制。 | 2、4、22 |
| 会话页用 draftRevision 和 markedTextRange 保护输入 | 工具处理接线；受理后清空、草稿冲突和输入法组合态仍有业务规则。 | 15 |
| SKIE 类加载器冲突、缺切片及 SwiftPM 首次解析无制品 | 官方/CPF 插件隔离、制品准备顺序和独立消费者进入验收。 | 17、20、21 |

完整问题、手段和验收见[工程清单](Kotlin_Swift_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## 页面观察状态与关闭共享引擎

以下是协议说明，不是已经实现或冻结的 MadB API：

1. Kotlin 暴露明确语义的状态流；profile 指定由 SKIE 或 NativeCoroutines 负责所选协程入口。
2. Swift 页面通过普通异步/订阅 API 获取状态；必要投影在声明的隔离域读取，再更新 UI 本地状态。
3. 页面隐藏与销毁分别处理，停止收集不自动关闭共享引擎，也不证明上游工作已经停止。
4. 账户 owner 关闭时拒绝新操作，按既定协议停止并排水；异步等待可取消，后台清理责任不能丢失。
5. 验证页面重建、宏组合、注册即回放、取消与完成竞争、ARC/GC 引用环；诊断关闭不改变保护行为。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

主线 C06/L04 与 X01/X02 组合分别验收；SKIE 已有 Combine 预览能力，不能以“没有 Combine”为引入另一生成器的唯一理由。

关联工作包：[C06 Kotlin / Swift / Rust 四路线](../../../visuals/implementation-progress.html#task-C06) · [L04 Swift 惯用 API](../../../visuals/implementation-progress.html#task-L04) · [I04 Android 与 Apple 模板](../../../visuals/implementation-progress.html#task-I04) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10) · [X01 NativeCoroutines 与 SKIE 组合](../../../visuals/implementation-progress.html#task-X01) · [X02 官方 Swift export](../../../visuals/implementation-progress.html#task-X02)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
