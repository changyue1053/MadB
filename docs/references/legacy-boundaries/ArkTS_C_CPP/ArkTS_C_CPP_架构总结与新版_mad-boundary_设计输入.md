# ArkTS ↔ C/C++ 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

主线为目标 HarmonyOS/OpenHarmony Node-API 的 C/C++ 适配；C 与 C++ 分别验证。ANI 属于 X03 独立扩展。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 后端选择 | Node-API 负责运行时接线；AKI、napi_generator、Taihe 属于可评估的生态工具，不能仅凭本研究列出就计为 MadB 已接入后端。 |
| 声明与原生实现 | 真实头文件及编译参数是签名依据；C++ 包装处理异常、RAII、线程与对象寿命。IDL 方案需明确唯一权威，不与头文件各自维护一套接口。 |
| 执行环境 | napi_env、局部值、持久引用和 TSFN 分别管理；持有引用不授予跨线程或跨 Worker 访问权。 |
| 关闭与队列 | 关闭准入、停止生产者、源端回调、排队载荷和消费结果分别收尾；容量同时约束项目数与字节，不等待已经退出的 ArkTS 环境。 |
| SDK 交付 | 声明、适配 so、底层库和应用 C++ runtime 配套；最终 HAR/HAP 的装载、反向回调及 Release 由真实消费者验证。 |

路线身份：`ARKTS-C-NAPI`、`ARKTS-CPP-NAPI`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| 旧 mad 台账桥通过 TSFN 投递后等待落盘结果 | 入队成功与业务完成不同；双向同步等待必须检查死锁。 | 8、11、18 |
| MadDemo 装载壳在后续仍有回调时保留共享库 | 回调、虚表和析构函数的代码寿命同样属于资源协议。 | 25、26 |
| 旧 mad-boundary 的 N-API 扫描使用模式识别 | 保留启发式诊断的范围说明，新增元数据、真实调用和拒绝用例。 | 30 |

完整问题、手段和验收见[工程清单](ArkTS_C_CPP_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## 耗时图片解码与环境退出

以下是协议说明，不是已经实现或冻结的 MadB API：

1. 先声明 decode 是否保留输入、失败输出是否为空、Image 释放器和释放执行器；签名不能推断这些语义。
2. 离开 ArkTS 调用环境前准备由操作持有的原生输入，工作线程执行解码，不携带局部 napi_value。
3. 结果准备回投时检查环境和操作状态；入队不等于消费方已经取得 Image。
4. 若取消只结束等待，原生 owner 继续持有输入和 context；env 退出或结果无人接收时，在允许的原生路径清理。
5. 关闭图像前等待登记的渲染/读取使用结束；用队列满、环境退出及迟到结果验证 CRIT-02/03/11。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

绑定生成 → C/C++ 与 ArkTS 编译 → HAR/HAP 装载 → 正向与宿主回调 → 环境退出/关闭交错。普通 Node 或 TypeScript 测试不替代 ArkTS 设备证据。

关联工作包：[C07 ArkTS 四路线](../../../visuals/implementation-progress.html#task-C07) · [L05 ArkTS 惯用 API](../../../visuals/implementation-progress.html#task-L05) · [L06 C / C++ 资源与属性封装](../../../visuals/implementation-progress.html#task-L06) · [I05 ArkTS 与多平台组合模板](../../../visuals/implementation-progress.html#task-I05) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10) · [X03 ANI 后端](../../../visuals/implementation-progress.html#task-X03)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
