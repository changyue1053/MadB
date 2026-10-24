# Kotlin/Native ↔ ArkTS 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

主线采用 CPF-KMP-CMP 工具链 + akinterop，CMP 仅在共享 UI 配置启用；KNOI 为 X08 独立后端，不串接到 akinterop 后面。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 工具链与调用桥 | CPF 提供 Kotlin/Native 与可选 CMP 配套，akinterop 处理导出、代理和调用包装；Node-API 是平台机制，不是另一种业务载荷。 |
| 类型与载荷 | 支持的标量、对象与代理直接传递；复杂事件可显式选受检 JSON，二进制按后端能力单独选择。不能将 MadDemo 会话页 JSON 方案推广为所有接口必选。 |
| 构建隔离 | 官方 Kotlin 与 CPF 使用独立 Gradle 根、插件环境、缓存与输出，首版采用独立发布坐标和根元数据。共享源码不代表 KLIB 可交换。 |
| 生成器前端 | 现行 CPF/akinterop 配方的 KSP1 与官方线 KSP2 分别编译和运行；KSP 兼容参数不等于整个业务工程的语言版本。 |
| 运行协议 | 订阅、Job、napi_ref、TSFN 与页面生命周期分别管理；每个 env 有自己的执行器、退出和结果清理协议。 |
| UI 与交付 | ArkUI V1/V2 按目标生成状态适配，CMP 混排只在选用时验证；正式 SDK、设备、Release/HAR/HAP 与 Beta 参考环境分开。 |

路线身份：`KN-ARKTS-NAPI`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| 订阅 ID 经整数收窄曾存在误命中风险 | 不透明句柄与业务数值分开；在发生精度损失前校验。 | 3、5 |
| 错误监听器、空串降级曾掩盖入口转换失败 | 受检解码与 pending exception 处理分别验证，错误不能变为合法默认值。 | 4、6、7 |
| 会话页用状态字段参与 key 修复刷新 | 这是历史页面修复；通用适配优先稳定身份和正确观测，另验焦点与局部状态。 | 19、20 |

完整问题、手段和验收见[工程清单](Kotlin_ArkTS_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## 一条状态更新如何到达 ArkUI

以下是协议说明，不是已经实现或冻结的 MadB API：

1. Kotlin 在约定隔离域产生公开状态；选择类型化对象或声明过的载荷协议，不临时退成字符串。
2. 适配层登记观察身份和容量；按状态/事件协议决定合并、拒绝及恢复，不把 StateFlow 当可靠事件日志。
3. akinterop 在有效 env 的执行器上交付；字段检查、错误分类和 ArkUI 状态实例化各有明确责任。
4. 页面隐藏、后台或销毁按各自策略停止观察；共享资源仍由业务 owner 关闭。
5. 停止之后分别收尾源端、队列和迟到结果，验证重复进入、队列满、Worker 退出和输入法组合态。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

C07/P05 的 CPF 主线验证覆盖实际依赖、注册、目标 SDK 与设备；X08 单独验证 KNOI 的加载和注册。JSON 协议测试不能替代真实桥调用。

关联工作包：[C07 ArkTS 四路线](../../../visuals/implementation-progress.html#task-C07) · [L03 Kotlin 惯用 API](../../../visuals/implementation-progress.html#task-L03) · [L05 ArkTS 惯用 API](../../../visuals/implementation-progress.html#task-L05) · [I05 ArkTS 与多平台组合模板](../../../visuals/implementation-progress.html#task-I05) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10) · [X08 KNOI 独立 Kotlin/ArkTS 后端](../../../visuals/implementation-progress.html#task-X08)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
