# Swift ↔ Rust 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

主线为 UniFFI Swift 直连 Rust；经 Kotlin/Native 中转的业务链单独装配和验证。swift-bridge 与显式 C ABI 属于生态研究选项，不自动替代主线。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 模型权威 | Rust 导出声明生成 Swift 原生 Record/Enum 与资源代理；嵌套 Object 仍共享资源，struct 不自动等于深度独立快照。 |
| 版本与取消 | 现行 UniFFI 候选以技术栈规范为准。旧研究对 0.32.1 模板的正反取消观察只属于该版本，升级后重验模板与实际效果。 |
| 单一接线责任 | 复用 UniFFI 脚手架、引用管理与匹配检查；MadB 补充声明的取消、关闭、执行器与交付适配，不再制造重复完成或重复释放机制。 |
| Swift 并发 | Swift 6 严格并发、MainActor、回调线程与 Rust 同步联合验证；unchecked Sendable 不能代替锁或正确所有权。 |
| 物理共享 | 直连 Swift SDK 与 KMP SDK 共存时，在组合根选择同一内核或明确隔离；包装对象看起来相同不证明链接到同一实例。 |
| 发布形态 | Rust 库、生成 Swift、C 头、modulemap、平台变体配套；源码包装与预编译 Swift 模块采用各自消费矩阵。 |

路线身份：`SWIFT-RUST-UNIFFI`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| 旧研究观察 UniFFI 0.32.1 的正向 Task 取消未自动接到 Rust Future，反向释放会请求取消 Swift Task | 作为固定版本观察保留；新候选重验两个方向，底层停止和副作用另有业务契约。 | 9、20 |
| MadDemo 请求表先登记再发送以接住同步响应 | Swift continuation 同样先登记、一次认领，并清理无人接收的资源。 | 10、11 |
| Rust 被合入唯一 MadKit | 新增直连包装时用 CRIT-04 检查实际内核和资源 owner。 | 23 |

完整问题、手段和验收见[工程清单](Swift_Rust_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## Swift Task 取消与 Rust 资源结果

以下是协议说明，不是已经实现或冻结的 MadB API：

1. 固定生成器和二进制，确认两个调用方向的取消接线，再声明此操作只结束等待还是请求停止工作。
2. 调用前建立操作 owner 和终态认领；回调或 Future 完成时执行已声明的错误/资源转换。
3. Task 已取消且没有消费者接收结果时，按契约释放返回资源；不要释放仍被轮询或回调访问的句柄。
4. Rust 持有 Swift listener、listener 持有 Rust 对象时，显式解除注册或断环；普通 deinit 与业务 close 分工。
5. 在独立 Swift 消费者中控制取消、完成、释放和关闭交错，记录实际工具、平台、候选及符号身份。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

Swift 直连不复用 Gobley/Native 运行结论。保留 UniFFI checksum 并检查实际装载身份；模板、运行依赖、Release 和资源清理一起验证。

关联工作包：[C06 Kotlin / Swift / Rust 四路线](../../../visuals/implementation-progress.html#task-C06) · [L04 Swift 惯用 API](../../../visuals/implementation-progress.html#task-L04) · [I04 Android 与 Apple 模板](../../../visuals/implementation-progress.html#task-I04) · [I05 ArkTS 与多平台组合模板](../../../visuals/implementation-progress.html#task-I05) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
