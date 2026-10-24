# 跨语言边界研究归档

本目录收纳九组、18 份研究阅读版，对照 MadB 0.9 于 2026-10-24 修订。资料用于理解问题和追溯设计输入；当前开发先读任务关联的 architecture、ADR、engineering 与 validation，不将本目录当作另一套实施规范。

## 如何查阅

- 按语言组查[研究索引](../history.md)，先读研究整理，再按问题查看工程清单。
- 236 个来源问题身份、标题及公共问题映射保持不变；可从[来源映射](../../catalog/source-coverage.json)定位修订阅读版行号。
- [更新前完整快照](snapshots/2026-10-24-before-refresh.zip)封存原始 18 份文件、旧 source-coverage.json 和 manifest.json；目录层次及文件字节保持原样。旧名称、旧候选版本、本机绝对路径与行号只在该快照中用于追溯，不作为运行指令或公开安装入口。
- 研究记录的项目事实、上游机制与推演场景各按原有性质理解；本次整理没有重跑旧项目，也不证明 MadB 已实现或已认证。

## 本次统一的口径

| 主题 | 当前依据 |
|---|---|
| 路线、目标、方向及分发方式 | [后端规范](../../architecture/06-backend-profiles.md)、[选型 ADR](../../adr/0008-route-selection-and-distribution.md) |
| 精确版本与探针阻断范围 | [技术栈规范](../../architecture/16-technology-stack-and-versions.md)；研究页不另维护版本表 |
| 所有权、取消、源端/队列/结果收尾 | [生命周期](../../architecture/05-lifecycle-and-concurrency.md)、[运行机制](../../architecture/08-runtime-and-idiomatic-api.md) |
| 模块、生成与手写分工 | [工具骨架](../../architecture/02-modules-and-ports.md)、[Lab 骨架](../../validation/02-lab-architecture.md) |
| 生成事务、缓存和实际装载身份 | [生成规范](../../architecture/07-code-generation.md)、[构建与分发](../../architecture/09-build-packaging-release.md) |
| 普通 API、宏、属性封装与诊断 | [语言体验](../../architecture/15-developer-ergonomics.md)、[诊断与性能](../../architecture/10-diagnostics-and-performance.md) |
| 工作包与 Agent 开发方法 | [计划](../../delivery/01-implementation-plan.md)、[Agent 规范](../../engineering/07-agent-development.md) |

原研究中的模块建议和 P0–P3 顺序已改为上述权威入口及现有工作包关联；不以整理资料为由新增路线、任务或进度。选定后端不支持的形状明确拒绝，不静默换桥、改为 JSON 或减弱取消和线程保证。

## 维护与追溯

阅读版可在明确范围内更新。修改后同步 source-coverage 的路径、行号和文件摘要，保留 source_id、原问题含义和映射；重大归并另作显式决定。修订不能覆盖封存快照或改写旧审查报告。

校验入口为 `python3 tools/check_docs.py`（在 docs 目录执行），同时核对阅读版定位、快照内容和原映射。公开发布仍按现有贡献/许可规则核对历史材料；可独立检出的文档不要求作者机器上存在旧仓库。
