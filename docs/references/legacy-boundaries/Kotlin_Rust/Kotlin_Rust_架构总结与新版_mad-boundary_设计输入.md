# Kotlin ↔ Rust 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

JVM/Android 使用 UniFFI Kotlin/JNA，Native 使用经修复和验证的 Gobley/UniFFI 配套；官方 Kotlin 和 CPF 的目标构建分别验证。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 接口与表示 | Rust 导出声明或既有 UDL 是结构权威；绑定生成参数转换、元数据和 ABI 脚手架，不要求额外维护一套业务 JSON。 |
| 后端配套 | JVM/Swift 的 UniFFI 候选不推导 Gobley 兼容；修复后的 Native 组合由 P03 固定模板、运行依赖、补丁和能力。 |
| 异步模板 | 成功通知后的异常不能再次进入错误通知；单次完成修复必须覆盖生成模板，单改 Rust runtime 或一次生成输出无效。 |
| CPF 构建 | Gobley CLI 生成 nativeMain 不代表上游插件支持 OHOS；MadB 单独组织 CPF 编译、cinterop、链接与设备消费。 |
| 引用与关闭 | 复用后端对象/回调保活；业务 close、停止确认和最终回收单独声明，不再生成第二套竞争 owner。 |
| JVM 交付 | JNA、jnidispatch、AAR/JAR、装载和 R8 属于本主线验证，不能误归为可省略的直接 C JNA 扩展。 |

路线身份：`KJVM-RUST-UNIFFI`、`KN-RUST-GOBLEY`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| MadDemo 修改版生成器明确拒绝 async/Object | 该限制是历史 fork 能力边界，不是通用后端完整能力或语言永久限制。 | 1、9、20 |
| 网络等待超时后仍保留在途名额 | 停止等待不等于请求已经停止；容量只能在对应事实发生后归还。 | 10 |
| 绑定集中在 capability/kernel，IM 与推理共享运行时 | 通用组合仍须检查最终链接和实例身份，代码集中不单独证明物理唯一。 | 22、23 |

完整问题、手段和验收见[工程清单](Kotlin_Rust_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## 成功通知抛错如何避免再次完成

以下是协议说明，不是已经实现或冻结的 MadB API：

1. 锁定本次实际使用的生成器模板与运行依赖，不以包版本接近作为兼容证据。
2. 将 native 计算失败与向外通知失败分开；一旦成功终态认领，不再调用错误完成通道。
3. 投递失败或等待者取消时，清理未交付的资源，而不是再次恢复同一个 continuation。
4. 在屏障处注入成功通知后的异常、取消竞争和迟到回调，分别检查完成次数与资源清理。
5. 在 JVM、官方 Native 和声明支持的 CPF 配置分别真实消费；不以一个后端结果代替其他目标。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

C06/P03 的修复后配套是对应 Native 能力的前置；不阻断独立 Rust/C 首链。异步、Object、反向回调与候选装载各有正反例。

关联工作包：[C06 Kotlin / Swift / Rust 四路线](../../../visuals/implementation-progress.html#task-C06) · [L03 Kotlin 惯用 API](../../../visuals/implementation-progress.html#task-L03) · [I04 Android 与 Apple 模板](../../../visuals/implementation-progress.html#task-I04) · [I05 ArkTS 与多平台组合模板](../../../visuals/implementation-progress.html#task-I05) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
