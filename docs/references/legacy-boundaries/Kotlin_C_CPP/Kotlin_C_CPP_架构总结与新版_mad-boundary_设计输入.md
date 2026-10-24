# Kotlin ↔ C/C++ 研究整理与 MadB 设计对照

对照基线：MadB 0.9；修订日期：2026-10-24。本文整理既有研究并解释当前方案的落点；不设新路线、实施阶段或认证结论。文件名保留旧名称，便于来源追溯。

JVM/Android 主线使用 JNI，Kotlin/Native 使用 cinterop 与显式 C 可见边界；四条路线分别验收。直接 C 调用的 JNA/FFM 属于 X04 扩展。

## 当前路线与决定

| 决策点 | 当前采用的规则 |
|---|---|
| 运行时分流 | JVM 的 native 签名、JNIEnv、ClassLoader 和 R8 与 Native 的 .def、C 布局、StableRef 分别实现；不能套一套引用协议。 |
| C++ 边界 | JNI 的 C++ 包装可直接调用 C++；K/N 使用窄 C adapter 或声明过的 Apple ObjC++ 接口，不让 cinterop 解析任意 C++ 模板。 |
| JNA 的位置 | UniFFI Kotlin/JVM 依赖 JNA 属于 Kotlin↔Rust 主线依赖；直接调用 C 的 JNA 扩展是另一用途。JavaCPP 是生态研究对象，不自动进入本计划。 |
| Android 构建 | shared KMP、Android app、需要 externalNativeBuild 的 Android library 分开；预编译 so 经 jniLibs 装配，不能混写两套构建配方。 |
| 输出和恢复 | 先检查状态，再检查原始输出长度/容量，最后转换与复制；resume 后调用方尚未恢复时发生取消，资源仍需按协议回收。 |
| 目标交付 | 官方与 CPF 的 cinterop 分开锁定；R8 后回调、传递 so、最终 APK 对齐及 16 KB 环境执行分别验证。 |

路线身份：`KJVM-C-JNI`、`KJVM-CPP-JNI`、`KN-C-CINTEROP`、`KN-CPP-CABI`。[后端规范](../../../architecture/06-backend-profiles.md)与[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)维护选型和精确候选；本文不复制另一份版本表。

## 保留的工程经验

以下项目事实是原研究的观察，未重新认定为当前代码缺陷或线上事故。目标是提炼可复现的通用 fixture。

| 原研究样本 | 在 MadB 中落实什么 | 清单条目 |
|---|---|---|
| MadDemo SecurityRandom 在 usePinned 中同步调用系统 C API | 同步借用范围可明确；原生返回后继续使用的异步指针不能照搬。 | 8、12 |
| 旧 mad HUKS 适配的输出长度检查顺序 | 先验状态与长度再复制；调用方检查不能补救 native 已发生的越界写。 | 11 |
| MadDemo 的 @CName 入口由 C++ dlsym 调用 | 导出名与函数指针描述分别校验，属于真实反向路径。 | 2、22 |

完整问题、手段和验收见[工程清单](Kotlin_C_CPP_跨语言边界工程清单.md)；原始详细案例与本机源码定位保存在[封存快照](../snapshots/2026-10-24-before-refresh.zip)。

## 回调转 suspend 时资源如何交接

以下是协议说明，不是已经实现或冻结的 MadB API：

1. 先明确同步首回调、输入保留期、输出释放器、注销确认及回调线程，再建立操作记录。
2. JNI 使用合法的线程环境和受管引用，K/N 使用有效的 StableRef/原生 context；两者通过自己的实现接入同一语义。
3. 启动 native 后允许立即完成，成功/失败/取消统一认领；原生结果到达不等于等待者取得所有权。
4. 等待者在 resume 后恢复前取消时，按资源交付失败规则回收；不能只做 isActive 检查。
5. 只有源端、启动路径和在途回调结束后才能释放 context；原生无法中断时保留后台收尾责任。

## 模块与生成边界

复用后端生成声明和基础接线；MadB 在已明确的契约上补类型检查、生命周期/执行器适配、构建和证据。算法、真实所有权、底层停止效果、业务受理/恢复与独立测试预期仍由作者负责。

[工具工程骨架](../../../architecture/02-modules-and-ports.md)固定 core/app/adapter/helper/SDK 分工；[运行机制责任](../../../architecture/08-runtime-and-idiomatic-api.md)避免重复生成或释放；[Lab 骨架](../../../validation/02-lab-architecture.md)要求公开候选消费和独立 expected。语言便利入口与普通 API 共用协议，具体设计见[开发体验规范](../../../architecture/15-developer-ergonomics.md)。

## 计划与验收入口

四条主线分别覆盖真实正反消费；JNI 检查实际 JVM 描述符、GlobalRef/LocalRef、attach/detach、Release/R8；Native 检查目标布局、cinterop 与借用期。

关联工作包：[C08 其余七条原生路线](../../../visuals/implementation-progress.html#task-C08) · [L03 Kotlin 惯用 API](../../../visuals/implementation-progress.html#task-L03) · [L06 C / C++ 资源与属性封装](../../../visuals/implementation-progress.html#task-L06) · [I04 Android 与 Apple 模板](../../../visuals/implementation-progress.html#task-I04) · [C09 完整 Lab、故障与性能收口](../../../visuals/implementation-progress.html#task-C09) · [C10 候选、分发与供应链收口](../../../visuals/implementation-progress.html#task-C10) · [X04 JVM FFM / JNA](../../../visuals/implementation-progress.html#task-X04)。

阶段、前置与真实状态仅由[实施目录](../../../catalog/implementation-plan.json)维护。原文 P0/P1/P2/P3 建议已收敛到既有 S0–S6 计划；不再要求迁移或修补旧项目。按照[Agent 开发规范](../../../engineering/07-agent-development.md)绑定任务与子范围，按[质量门禁](../../../engineering/05-review-and-quality-gates.md)报告真实验证。

内存、回调与关闭采用[生命周期规范](../../../architecture/05-lifecycle-and-concurrency.md)和[CRIT 场景](../../../validation/08-critical-scenarios.md)；构建、实际装载与唯一内核采用[分发规范](../../../architecture/09-build-packaging-release.md)。只覆盖本 profile 承诺的目标和机制，历史测试不能替代新候选证据。
