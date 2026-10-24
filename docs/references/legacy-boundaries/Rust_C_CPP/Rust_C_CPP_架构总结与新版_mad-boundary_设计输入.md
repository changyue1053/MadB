# Rust ↔ C/C++ 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

Rust↔C 的 bindgen/cbindgen 链为 S1 首链；Rust↔C++ 主线采用 CXX 并在 C08 验收。Crubit 属于 X05，现有明确 C ABI 可继续使用。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 生成方向 | bindgen 从实际 C/C++ 声明生成 Rust 底层绑定；cbindgen 从已设计的 Rust C 可见接口生成头。头文件生成不会凭空实现 extern 导出。 |
| CXX 边界 | CXX 生成明确桥接面的接线和一致性检查，不能替代外部对象寿命、Pin/别名、Send/Sync 的证明，也不自动承诺任意 C++ ABI 稳定。 |
| 首链句柄 | MadB 自有首链采用登记令牌、准入和回收协议；第三方原始指针 ABI 保持自身前提，不强制用登记令牌重写外部接口。 |
| 关闭分层 | 请求关闭、等待关闭和最终释放分开；源端可能已经取走 context 时，注销返回不一定允许释放。 |
| 目标范围 | 首链优先 macOS Arm64 的双向真实运行；x86_64 Darwin 的布局/编译探针不自动产生运行支持。更大矩阵随计划展开。 |
| 候选与恢复 | 生成源码批次、构建产物和候选各有身份与提交；IDE/CLI 并发、旧输入晚到及取消不可覆盖仍有效的批次。 |

路线身份：`RUST-C-ABI`、`RUST-CPP-CXX`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| IMLink 从 C 权威头生成 Rust sys 绑定并检查漂移 | 自动化声明镜像；目标宏、布局、工具和原生库仍须一致。 | 1、3、21 |
| IMDB observe 泵线程使用 user_data | 明确停止新回调、在途退出和 context 可释放时刻，数据库 close 不能代替观察线程确认。 | 11、13 |
| llama context 依赖 model，取消在词元间检查 | 父子寿命、线程能力和取消粒度分别声明，不承诺 decode 中即时中断。 | 10、12、14 |

完整问题、手段和验收见[工程清单](Rust_C_CPP_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## Future 消失之后谁保留原生输入

以下是协议说明，不是已经实现或冻结的 MadB API：

1. 启动前建立操作 owner，按已声明的保留期准备稳定输入和 context，允许同步首回调。
2. Future 只代表等待关系；被丢弃时请求已声明的取消，操作 owner 不随等待者立即销毁。
3. C/C++ 交付图像后，由终态认领决定交给消费者还是清理；重复通知同一指针不能每次构造独立 owning handle。
4. 只有原生保留期、启动路径、源端和在途回调都结束才释放输入/context；已交付结果按自身 owner 管理。
5. C 和 Rust 两个消费者分别运行，覆盖 CRIT-02/03/08；CXX 配置另验移动、析构、异常与线程约束。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

C05 交付 Rust→C 与 C→Rust 两个直接方向；回调不替代第二个消费者。C08 和 X05 不因首链通过自动取得资格，Miri 不替代任意原生 FFI 测试。

关联工作包：[C02 契约、配置、锁与证据格式](../../../visuals/implementation-progress.html#task-C02) · [C03 首链工具链与风险探针](../../../visuals/implementation-progress.html#task-C03) · [C05 Rust ↔ C 与独立 Lab 首链](../../../visuals/implementation-progress.html#task-C05) · [C08 其余七条原生路线](../../../visuals/implementation-progress.html#task-C08) · [L06 C / C++ 资源与属性封装](../../../visuals/implementation-progress.html#task-L06) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10) · [X05 Crubit 后端](../../../visuals/implementation-progress.html#task-X05)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
