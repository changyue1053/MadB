# ArkTS ↔ Rust 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

主线为 napi-ohos 直连。复用既有 UniFFI 接口的 ArkTS 生成器由 X07 实验配置承接，ANI 由 X03 承接；三者分别锁定和验证。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 直连与中转 | ArkTS → napi-ohos → Rust 与 ArkTS → akinterop → Kotlin/Native → Gobley → Rust 是不同链路，中转案例不证明直连已通过。 |
| 后端版本 | napi/derive/build/sys 与 ohrs CLI 分别锁定。UniFFI-ArkTS beta 使用自身 UniFFI 配套，不混用 Swift/JVM 或 Gobley 的版本。具体数字只在技术栈规范维护。 |
| UniFFI 实验链 | 额外验证生成的 N-API Rust adapter、UniFFI 序列化及既有动态库装载。未支持类型、异线程同步回调不能静默改成稍后通知。 |
| 环境和资源 | Rust Send/Sync、ArkTS env 权限与对象保活分开证明；取消 Promise 等待不能提前释放原生输入、context 或迟到结果。 |
| 组合装配 | 与 KMP 同时接入时，组合根、链接布局和实例身份共同证明共享内核，禁止仅靠单例声明推断物理唯一。 |

路线身份：`ARKTS-RUST-NAPI`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| MadDemo 网络超时仅停止等待，推理取消由原生轮询 | 每种操作分别声明取消效果；不能从统一 Promise 外壳推导停止工作。 | 8、9 |
| 旧 mad 台账回调需要真实执行结果 | 同步返回的 Rust 宿主协议不能随意改写成异步入队。 | 10、11 |
| MadDemo 的 Rust 静态库用于 KMP 组合装配 | 新增直连 so 后验证是否重复静态链接及复制状态。 | 26 |

完整问题、手段和验收见[工程清单](ArkTS_Rust_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## 迟到结果、队列满与 env 退出

以下是协议说明，不是已经实现或冻结的 MadB API：

1. 声明 Rust 操作返回值/错误、env 绑定、取消效果及结果释放器；阻塞库另定工作线程。
2. 注册时建立操作与环境代次；工作线程只持有允许的原生值，不跨线程使用普通 ArkTS 对象。
3. 生产事件使用条目和字节预算；满队列按声明拒绝、合并或背压，可靠事件的补偿由业务决定。
4. 取消或退出后停止新交付，在途任务仍有原生收尾 owner；不得为了返回 Promise 成功而吞掉投递失败。
5. 在独立设备消费者中验证两个后端各自的注册、同步回调拒绝和结果清理；无设备的构建 smoke 保持未运行。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

Rust 单测、ArkTS 编译、目标 SDK 装载、正反调用和环境退出分别记录。X07 的 fixture 或 HAP 构建成功不能替代设备执行完成。

关联工作包：[C07 ArkTS 四路线](../../../visuals/implementation-progress.html#task-C07) · [L05 ArkTS 惯用 API](../../../visuals/implementation-progress.html#task-L05) · [I05 ArkTS 与多平台组合模板](../../../visuals/implementation-progress.html#task-I05) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10) · [X07 UniFFI ArkTS 实验后端](../../../visuals/implementation-progress.html#task-X07) · [X03 ANI 后端](../../../visuals/implementation-progress.html#task-X03)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
