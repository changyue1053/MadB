# Swift ↔ Rust 跨语言边界工程清单

对照基线：MadB 0.9；修订日期：2026-10-24。本文为研究阅读版，保留原问题编号和历史样本；当前实施以权威规范、任务和目标证据为准。表内项目事实沿用原研究快照，本次没有重新验证旧仓库的现状，也不把风险推演当作已发生事故。

主线为 UniFFI Swift 直连 Rust；经 Kotlin/Native 中转的业务链单独装配和验证。swift-bridge 与显式 C ABI 属于生态研究选项，不自动替代主线。

当前选型、关键更新与实施关联见[Swift ↔ Rust 研究整理](Swift_Rust_架构总结与新版_mad-boundary_设计输入.md)；精确版本只维护在[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)。原文件、本机路径、旧行号及旧来源映射保存在[更新前快照](../snapshots/2026-10-24-before-refresh.zip)，原始记录不被改写。

本页共 26 项，稳定来源身份为 `Swift_Rust:序号`。[source-coverage.json](../../../catalog/source-coverage.json)记录修订阅读版的路径、行号、摘要及公共问题映射；原始映射可从快照中的 source-coverage.json 复核。

## 一 公开契约与数据

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **1. 路线与后端能力** | 主线选择 UniFFI Swift，与 JVM/Native 后端分别固定版本及能力；KMP 中转、swift-bridge 或 C ABI 不是自动替代配置 | UniFFI Swift 后端、能力探针、Swift 编译、API 基线 | 原研究：MadDemo 的 Gobley fork 限制属于 KMP 后端；Swift 后端应单独判定。[Swift 生成工具](https://mozilla.github.io/uniffi-rs/latest/swift/uniffi-bindgen-swift.html) | 不把 Kotlin 能力表直接复制为 Swift 能力表；不支持的形状有明确诊断 |
| **2. 模型权威源与值语义** | Rust SDK 可以以 Rust 声明为源；优先使用生成的 Swift 类型，领域隔离需要时再做映射 | Record / Enum 生成、custom type、受限 Facade | 原研究：UniFFI 将 Record 映射为 struct、Enum 映射为 Swift enum、Object 映射为包装类与协议。[类型映射](https://mozilla.github.io/uniffi-rs/latest/swift/overview.html) | 值数据、嵌套资源代理和句柄明确区分；修改值数据不会被误认为写回 Rust |
| **3. 数值 可空值与稳定 ID** | 保留固定宽度、符号、单位与空值；业务 ID 与资源句柄分开 | UInt64 / Int64、受检转换、强类型 ID、边界值测试 | 原研究：MadDemo 用 WireRange 管理跨符号与收窄转换；Swift Facade 面临同类语义问题。 | UInt64 转 Int64 越界拒绝；nil 不转成 0；句柄不作为持久化业务身份 |
| **4. 字符串 字节与 ABI 表示** | 明确字符串与任意字节的区别，检查长度和输入大小；手写 C ABI 时声明布局、编码和释放协议 | String / Data、生成转换、cbindgen、Unicode 样例、布局探针 | 原研究：UniFFI 0.31.2 修复过 Swift 转换时移除字符串首位 U+FEFF 的问题。[发布记录](https://github.com/mozilla/uniffi-rs/blob/v0.32.1/CHANGELOG.md) | 文本不丢字符；字节数不等同字符数；Rust String / Vec 内部布局不作为公共 C ABI |
| **5. 业务错误 panic 与宿主失败** | 业务失败使用 Result / Error；声明回调错误和未知错误映射；明确 panic 后资源是否可继续使用 | UniFFI Error、错误目录、UnexpectedUniFFICallbackError 适配、panic 边界 | 原研究：UniFFI foreign trait 的未知错误需要适配；MadDemo 导出门另设 guarded 处理展开式 panic。[回调规则](https://mozilla.github.io/uniffi-rs/latest/foreign_traits.html#error-handling)、 | 错误不静默变成空结果；业务拒绝、取消、内部故障分开；不依赖错误文案识别类别 |

## 二 调用与异步协议

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **6. 阻塞性与执行位置** | 分开声明调用执行器和回调执行器；阻塞 I/O、推理和关闭交给适当工作队列 | 工作队列、执行器 Adapter、主线程检查、耗时基准 | 原研究：MadDemo 推理经同步 inferGenerate 调用，主线程守卫只检查，不负责切线程。 | 包一层 async 或 Task 不被当作完成调度设计；长阻塞不会占用 UI 或耗尽协作执行线程 |
| **7. Swift 6 隔离与 Sendable** | 按实际共享关系使用 Actor、不可变数据或锁；审核所有 unchecked 声明 | Swift 6 模式、严格并发检查、Actor、NSLock、Rust 同步设施 | 原研究：UniFFI 发布记录包含回调表严格并发警告的修复；Swift 的 unchecked Sendable 将同步证明交给实现者。[发布记录](https://github.com/mozilla/uniffi-rs/blob/v0.32.1/CHANGELOG.md)、[Sendable](https://docs.swift.org/latest/documentation/swift/sendable/) | Rust 工作线程不直接访问 MainActor 状态；生成代码可编译与运行协议安全分别验证 |
| **8. async 运行时与任务归属** | 确认桥接支持、Rust Future 的驱动方式、Tokio 等运行时需求；给派生任务明确 owner | 锁定版本的异步桥、运行时 Adapter、任务登记 | 原研究：UniFFI 的异步桥与 Rust 自身运行时是不同层；MadDemo 该快照中的以同步调用和回调组合原生能力。[异步机制](https://mozilla.github.io/uniffi-rs/latest/internals/async-overview.html)、 | 运行时要求满足；派生任务不会因为外层返回或取消而失去管理 |
| **9. 双向取消 超时与副作用** | 按锁定模板和方向分别验证 Task/Future 的取消接线；声明等待、工作、观察及副作用效果；缺接线时由明确适配补足 | withTaskCancellationHandler、线程安全取消信号、操作 ID、取消状态机 | 原研究：原研究对 UniFFI 0.32.1 模板记录：正向未自动连接 Task 取消，反向释放请求取消 Swift Task；这不是所有后续版本的结论。[固定模板](https://github.com/mozilla/uniffi-rs/blob/v0.32.1/uniffi_bindgen/src/bindings/swift/templates/Async.swift) | 当前候选重验正反方向；取消先于注册、完成相撞和迟到资源均可控；请求取消不等于底层停止 |
| **10. 一次完成与结果交接** | 先登记再触发；完成、失败、取消通过同一认领协议协调；无人接收的资源结果按契约释放 | CheckedContinuation、同步状态机、迟到结果清理、屏障测试 | 原研究：MadDemo 请求表先登记等待者再发送，因为响应可能早于发送返回。 | continuation 恰好恢复一次；等待者离开后原生回调仍可安全收尾；不直接释放在途句柄 |
| **11. 同步回调与重入** | 声明首回调、执行线程、同步返回值和重入规则；不持业务锁等待外部回调 | 回调 Adapter、入队协议、锁边界审查、交错测试 | 原研究：mad 普通事件在事件泵线程交付，关户最后一批在关闭调用线程同步交付。 | 不假设所有回调都异步；同步要求返回的结果不能简单投递 MainActor 后假报成功 |
| **12. 事件 背压 顺序与恢复** | 声明容量、顺序、丢弃与恢复；状态合并和可靠事件分开；检查入队结果 | AsyncStream / AsyncThrowingStream、有界队列、序号、水位、恢复接口 | 原研究：AsyncStream 的 continuation yield 立即返回，默认缓冲无界；MadDemo 对存储变化和网络响应采用不同交付策略。[Swift 设计](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0314-async-stream.md)、 | 有界缓冲不被当作生产者背压；并发回调不会依赖调度顺序保持业务顺序；ACK 不被当状态丢弃 |

## 三 所有权 内存与多线程

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **13. 唯一关闭责任与共享引用** | 明确谁创建和关闭，哪些调用方只持有引用；资源状态独立于引用数 | 绑定已有句柄管理、业务 close、状态机、资源作用域 | 原研究：UniFFI Swift Object 的 deinit 释放绑定句柄，Rust Object 使用 Arc 管理引用。[模板](https://github.com/mozilla/uniffi-rs/blob/v0.32.1/uniffi_bindgen/src/bindings/swift/templates/ObjectTemplate.swift)、[引用机制](https://mozilla.github.io/uniffi-rs/latest/internals/object_references.html) | 单一关闭责任不禁止共享引用；最后引用释放不被笼统等同于业务排水完成 |
| **14. ARC 与 Rust 引用环** | 回调注册与撤销配对，明确保活链；适当使用弱引用或显式断环 | 解除注册、弱引用、Memory Graph、长时间运行测试 | 原研究：UniFFI 明确说明 foreign trait 会形成跨语言引用环，工具不会自动消除。[官方说明](https://mozilla.github.io/uniffi-rs/latest/foreign_traits.html#avoid-cycles) | Rust 持有 Swift listener、listener 持有 Rust 对象时仍有释放路径；弱引用不会让必需回调过早消失 |
| **15. 缓冲寿命 别名与释放配对** | 明确复制、借用、转移和共享；配对分配释放；借用不得逃逸，独占可变访问不得被别名破坏 | Data.withUnsafeBytes、后端缓冲协议、专用释放函数、内存测试 | 原研究：UniFFI 支持调用期间借用 Data；swift-bridge 提醒 Swift 侧可能违反 Rust 借用寿命与可变别名约束。[字节协议](https://mozilla.github.io/uniffi-rs/latest/types/bytes.html)、[借用限制](https://chinedufn.github.io/swift-bridge/safety/index.html) | 指针不传入超出借用期的任务；所有错误路径配对释放；不默认用 Swift deallocator 释放 Rust 分配 |
| **16. 关闭 取消与在途操作** | 关闭由正确 owner 驱动；明确停止准入、在途排水、资源回收与超时状态 | 幂等 close、关闭任务登记、保活引用、关闭报告 | 原研究：MadDemo 用专门生命周期执行位置和不可取消段避免关库被跳过；同一原则需要 Swift 独立实现。 | 调用者取消不会丢失必需清理；关闭超时后仍有人负责完成；未排水前不释放被访问的内存 |
| **17. 回调保活与释放位置** | 原生持有宿主实现期间保持有效引用；终止与释放分开；最后引用路径避免阻塞和线程违规 | 生成句柄表、回调生命周期契约、显式关闭、释放线程测试 | 原研究：UniFFI 为 foreign trait 管理跨语言引用；MadDemo 推理回调在推理线程读取取消信号。[引用说明](https://mozilla.github.io/uniffi-rs/latest/foreign_traits.html)、 | 不假定 deinit / Drop 在主线程执行；需要特定隔离或执行器的清理有明确入口；回调存活期覆盖在途调用 |

## 四 UI 集成与性能

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **18. 页面观察与共享订阅** | 页面取消自身观察；账户或 SDK owner 管理原生订阅；多观察者分发独立设计 | AsyncSequence Adapter、显式取消入口、onTermination、SwiftUI / UIKit 作用域接入 | 原研究：MadDemo 存储每库只允许一次常驻订阅；其 UIKit 样例在 viewDidLoad 订阅、deinit 取消。 | 不为每个页面重复原生注册；离页与销毁分别处理；提前结束观察有清理路径；AsyncStream 不被当作自动广播器 |
| **19. 跨界频率 复制与峰值内存** | 优先批量、增量和受限快照；测量分配、复制与队列增长；有瓶颈后再引入借用或共享缓冲 | Instruments、Time Profiler、Allocations、性能夹具、容量预算 | 原研究：UniFFI 对复杂类型进行内部表示转换；借用 bytes 是带寿命约束的独立路径。[表示转换](https://mozilla.github.io/uniffi-rs/latest/internals/lifting_and_lowering.html)、[字节借用](https://mozilla.github.io/uniffi-rs/latest/types/bytes.html) | 使用真实载荷和慢消费者测延迟、吞吐、复制量与内存；类型化不等于零拷贝 |

## 五 工程交付

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **20. 工具链与后端版本组合** | 按现行技术栈规范选择 Swift 的独立 UniFFI 候选；固定模板、scaffolding、运行依赖、Rust/Swift/SDK/目标与补丁 | 版本锁、能力探针、可重建生成物、回归矩阵 | 原研究：UniFFI 0.31.1 修复过 Swift 异步内存泄漏与启用 ASan 时的 iOS 崩溃。[发布记录](https://github.com/mozilla/uniffi-rs/blob/v0.32.1/CHANGELOG.md) | 固定版本历史修复不覆盖新配置；生成器变化也执行消费、取消与释放回归，不能只替换 runtime |
| **21. API ABI 与行为兼容** | 分别比较 Swift API、UniFFI 元数据与业务语义；保留适用的初始化 checksum | API 基线、元数据 diff、绑定校验、旧消费者测试 | 原研究：UniFFI Swift 默认执行库 checksum 检查，配置可关闭；关闭会削弱混用制品的检测。[配置](https://mozilla.github.io/uniffi-rs/latest/swift/configuration.html) | 业务契约号、UniFFI 协议版本和文件摘要不混用；签名未变不代表取消或关闭语义兼容 |
| **22. Apple 打包与跨工具链消费** | 按平台变体构建 Rust 库、头和 modulemap；明确 Swift 源码包装与预编译模块的交付方式 | XCFramework、SwiftPM、xcodebuild、模块接口与制品清单 | 原研究：Apple 支持静态库和头的 XCFramework；MadDemo 该快照中的合并 K/N 与 Rust，并使用内部发行选项。[官方打包](https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle)、 | 真机与模拟器变体不混合；干净环境、最低系统和声明支持的 Xcode 可消费；内部发行成功不等于跨 Xcode 兼容 |
| **23. KMP 共存与重复内核** | 组合根选择共享或隔离内核；核对最终链接、实例身份和初始化/关闭责任，不能仅靠逻辑单例 | 依赖图、链接映射、符号门、初始化和关闭测试 | 原研究：MadDemo 将 Rust 合入唯一 MadKit，壳不再另链一份内核。 | CRIT-04 验证两入口的实际实例与资源；隔离实例不得混用句柄，新增 SDK 不复制预期共享状态 |

## 六 验证与诊断

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **24. 双向消费与竞态验证** | Rust 单测、Swift 包装测试和真实 FFI 分层；控制完成、取消、关闭与回调交错 | Rust tests、Swift Testing / XCTest、设备与模拟器消费者、屏障、故障注入、适用的 Sanitizer | 原研究：上游 Swift ASan 与异步释放修复说明绑定和运行工具组合也需要验证。[发布记录](https://github.com/mozilla/uniffi-rs/blob/v0.32.1/CHANGELOG.md) | 覆盖同步首回调、重复完成、取消竞态、引用环、非法输入和 Release；测试覆盖范围如实记录 |
| **25. 跨语言故障定位** | 关联操作 ID、资源与构建身份；保留原生错误类别，按最终二进制归档符号 | 结构化日志、Rust panic hook、LLDB、dSYM、崩溃符号化 | 原研究：MadDemo 提供 Rust 到宿主的日志出口，panic hook 避免输出用户载荷。 | 符号对应实际发布二进制；能定位跨语言调用与释放路径；日志脱敏且不会递归触发边界失败 |
| **26. 生成与验证结论可信度** | 确定性生成、手写漂移检查；区分契约声明、静态观察和真实运行证据 | 生成物摘要、正反夹具、缺口报告、制品关联 | 原研究：旧 mad-boundary 曾有证据与已知漏报测试，可复用其分层思路扩展 Swift 消费证据。 | Kotlin 运行通过不算 Swift 直连通过；未支持、未运行、启发式零命中不算完整验证 |

## 本路线补充

- AsyncStream 有界缓冲不自动让生产者减速，也不自动广播；事件恢复和可靠性需要显式协议。

## 使用方式

按选定 profile 和本次任务读取相关条目；验收采用[公共问题目录](../../../validation/04-problem-catalog.md)、[关键交错场景](../../../validation/08-critical-scenarios.md)与任务 acceptance。本研究不维护独立进度、支持矩阵或第二套模块架构。共同约束与资料优先级见[归档入口](../README.md)。
