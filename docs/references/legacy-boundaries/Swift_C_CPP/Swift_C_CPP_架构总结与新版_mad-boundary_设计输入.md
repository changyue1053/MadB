# Swift ↔ C/C++ 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

Swift↔C 使用 Clang importer 和明确 C 可见导出；C++ 按源码直接互操作、C ABI 分发、Objective-C++ 适配三类正式配置选择。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 按交付选配置 | 同源受控构建可用直接 C++；希望收敛独立 SDK ABI 时选 C 边界；Apple 对象模型集成选 ObjC++。三类均有正式用途，不只是失败兜底。 |
| 正反方向 | SwiftPM 中 Swift 调 C++ 的支持不自动包含 C++ 调 Swift；反向导出头采用受验证的 Xcode/编译流程。C 导出能力按实际编译器探针判断。 |
| 模型与借用 | 优先直接使用适合公开的导入类型；Facade 不要求复制模型；独立快照与借用视图明确区分，容器重分配可使视图失效。 |
| 安全属性 | 生命周期、边界属性和 API Notes 只表达已确认的契约，不靠添加注解消除未知所有权诊断。 |
| 并发与错误 | Actor 的串行隔离不保证固定 OS 线程；C++ 内处理异常，Swift 包装处理声明错误；异步关闭不能简单塞进析构。 |
| 分发检查 | Swift 模块接口、C++ 标准库/ABI、最低系统、interop 模式向下游传播和反向导出分别检查；XCFramework 只组织制品。 |

路线身份：`SWIFT-C-CLANG`、`SWIFT-CPP-INTEROP`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| MadDemo 原有 iOS 链经 Kotlin 访问 Rust/C++ | 它提供装配与生命周期经验，不能作为 Swift 直接 C++ 的调用证据。 | 1、24 |
| 首次 SwiftPM 解析时本地二进制尚未准备 | 构建依赖与包解析时点需要可复现，不依赖开发者手动先跑脚本。 | 21 |
| 内核只合入 MadKit，壳避免再次链接 | 共享内核须检查最终链接图、实例和清理顺序。 | 22 |

完整问题、手段和验收见[工程清单](Swift_C_CPP_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## 异步解码的临时指针与回调

以下是协议说明，不是已经实现或冻结的 MadB API：

1. 声明原生 decode 是否保留输入、返回视图依赖谁、错误结果与释放器；withUnsafeBytes 的临时地址不能默认保留到异步完成。
2. C callback 使用明确 context；受支持的 C++ 闭包桥按其生成机制保活，两种上下文不能混用。
3. 启动前登记完成认领，允许同步首回调；成功、失败、取消只恢复等待者一次。
4. 取消后仍在工作的原生任务由操作 owner 收尾；迟到图像按释放器和真实线程要求清理。
5. 分别在三种选定配置中调用公开接口，验证视图失效、固定线程、正反方向和独立 Release 消费。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

同一语言对的三个配置分别留下工具链与消费证据；Swift 类型检查、C++ 同步正确性与设备运行不能相互替代。

关联工作包：[C08 其余七条原生路线](../../../visuals/implementation-progress.html#task-C08) · [L04 Swift 惯用 API](../../../visuals/implementation-progress.html#task-L04) · [L06 C / C++ 资源与属性封装](../../../visuals/implementation-progress.html#task-L06) · [I04 Android 与 Apple 模板](../../../visuals/implementation-progress.html#task-I04) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
