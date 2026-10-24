# Kotlin/Native ↔ Swift 跨语言边界工程清单

对照基线：MadB 0.9；修订日期：2026-10-24。本文为研究阅读版，保留原问题编号和历史样本；当前实施以权威规范、任务和目标证据为准。表内项目事实沿用原研究快照，本次没有重新验证旧仓库的现状，也不把风险推演当作已发生事故。

主线为 Objective-C export + SKIE。SKIE 与 KMP-NativeCoroutines 组合按 X01 验收；官方 Swift export 为 X02 独立 Alpha 配置。

当前选型、关键更新与实施关联见[Kotlin/Native ↔ Swift 研究整理](Kotlin_Native_Swift_架构总结与新版_mad-boundary_设计输入.md)；精确版本只维护在[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)。原文件、本机路径、旧行号及旧来源映射保存在[更新前快照](../snapshots/2026-10-24-before-refresh.zip)，原始记录不被改写。

本页共 24 项，稳定来源身份为 `Kotlin_Swift:序号`。[source-coverage.json](../../../catalog/source-coverage.json)记录修订阅读版的路径、行号、摘要及公共问题映射；原始映射可从快照中的 source-coverage.json 复核。

## 一 公开契约与数据

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **1. 导出范围与 API 形状** | 主线固定 Objective-C export + SKIE 的公开面；反向限受支持的 ObjC 协议/回调；Swift export 属于 X02 独立配置 | `internal`、Gradle `export`、`@ObjCName`、`@ShouldRefineInSwift`、导出 API 检查 | 原研究：MadDemo 的 `MadKit` 从多个模块组合导出，需要审查最终暴露面。 | 真实消费公开 API，拒绝不可调用的增强占位；不把官方 Swift export 的能力或限制套到 ObjC 路线 |
| **2. 类型映射与信息丢失** | 逐项处理可空值、值类、数值范围、泛型擦除、enum / sealed；以实际导出结果为准 | Kotlin/Native、SKIE、受限 Adapter、类型编译探针 | 原研究：MadDemo 的 `MessageSeq?` 经 ObjC 导出为不透明对象，通过 Kotlin `seenAction(row)` 辅助构造动作。 | `nil` 不变成 `0`；数值不截断；变体完整处理；不依赖强制转换 |
| **3. ID、相等性与业务含义** | 区分业务 ID、运行时句柄、对象身份；规定稳定表示、比较规则及数值单位 | 强类型 ID、明确的 raw value、`Equatable` / `Hashable` 适配、契约检查 | 原研究：MadDemo 原生页用字符串插值构造消息与已读去重键，这类依赖应收敛为明确的 ID 接口。 | 比较不依赖 `description/toString`；业务 ID 可按领域规则持久化 |
| **4. 引用对象、Facade 与独立快照** | 默认复用适合公开的 Kotlin 类型；需要独立值语义时再生成 Swift 模型；快照须在源对象的隔离规则下读取 | Kotlin 数据投影、Swift Facade、受限 DTO 生成器 | 原研究：MadDemo 的 `MessageRow` 已经是面向 UI 的 Kotlin 投影，不必再机械增加一套 Swift DTO。 | 明确是否共享对象、是否深度独立、修改是否回写；`struct` 不自动等于深拷贝 |
| **5. 边界输入校验** | 在入口校验长度、范围、格式、命名空间与载荷大小；保留领域层约束 | 受检构造器、生成校验器、长度上限、畸形输入测试 | 原研究：mad 的安全存储接口面对 `Any` 参数，显式校验账户 ID 和命名空间；配置存储限制键格式与记录大小。 | 非法输入得到稳定错误，不被静默修正为另一个有效请求 |

## 二 调用与异步协议

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **6. 空值、业务失败、取消与异常** | 分开表达“没有数据”“业务拒绝”“取消”“内部故障”；明确哪些异常允许过界 | 类型化结果、`@Throws`、`NSError` Adapter、Flow 错误策略 | 原研究：mad 的持久化接口明确区分“返回 nil 且无 NSError＝键不存在”和“有 NSError＝读取失败”，并用 ObjC 适配特殊导入形状。 | 不把失败当空结果；不吞取消；未知异常不会悄悄变成成功 |
| **7. 执行位置与任务作用域** | 声明调用执行器、回调执行器、任务归属及上下文传递；阻塞操作显式调度 | `CoroutineScope`、Dispatcher、`@NativeCoroutineScope`、Swift Task、显式操作上下文 | 原研究：mad 的配置存储注明是同步 I/O，可能由 UI 协程调用；有锁也不会自动切到后台。 | 主线程不执行重 I/O；页面或账户结束后，所属任务按契约终止 |
| **8. 取消、超时与副作用** | 区分停止观察、停止等待、请求停止工作、撤销副作用；对结果未知提供查询或幂等策略 | 取消桥接、底层 cancel、操作 ID、终态机、必要时 join / drain | 原研究：mad 的 HTTP 适配把协程取消传到 `URLSessionTask.cancel()`，同时处理取消后的迟到回调。 | 调用方知道取消究竟保证什么；超时后不会盲目重试已生效操作 |
| **9. 回调重入与完成竞态** | 允许契约声明同步回调；用原子终态协调完成和取消；处理注册期间回调、重复完成、迟到完成及资源交接 | `suspendCancellableCoroutine`、取消处理器、一次完成状态机、资源清理回调 | 原研究：mad 的安全存储校验失败会同步回调，正常路径进入队列，不能统一假设“回调总是稍后发生”。 | 各种回调时序都安全；取消导致结果无人接收时，结果持有的资源仍被释放 |
| **10. 状态、事件与交付保证** | 状态允许合并；事件明确回放、顺序、容量与溢出策略；需要确认时使用事件 ID、队列与 ack；跨进程可靠性另需持久化 | `StateFlow`、`SharedFlow`、Channel、订阅协议、持久化事件队列 | 原研究：MadDemo 将 notice 保存在状态列表，收到 `NoticeShown` 再确认；已读水位区分本地显示与服务端确认。 | 慢消费者行为可预测；状态允许跳过中间值；可靠事件不会靠“恰好收到回调”保证 |

## 三 所有权 内存与线程

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **11. 资源所有权与关闭协议** | 明确唯一关闭责任、允许的共享引用、借用期限和异步保活；关闭时协调新请求准入、在途操作与最终释放 | `close/dispose`、资源状态机、lease、原子准入；复用槽位时增加 generation | 原研究：mad-boundary 将身份、保活 lease、原子准入拆开，避免“对象内存仍在，但已不允许操作”被忽略。 | 无重复释放、关闭后操作、借用逃逸；关闭超时及在途任务处置明确 |
| **12. ARC＋GC 与释放时机** | 区分普通内存回收与确定性资源关闭；解除跨语言引用环；对释放执行器有要求的资源显式调度 | weak 引用、解除回调注册、显式取消、Memory Graph、Allocations | 原研究：Kotlin 官方说明：涉及 ObjC 对象的跨运行时引用环、延迟回收以及析构线程都需要单独处理。[官方说明](https://kotlinlang.org/docs/native-arc-integration.html) | 业务资源按时关闭；循环引用可解除；不依赖 GC 恰好及时执行 |
| **13. Swift 6 并发与跨边界原子性** | UI 隔离到 MainActor；共享对象两侧遵守一致的同步协议；`await` 后重验状态或版本，不能把整个异步方法当原子事务 | Actor、`Sendable` 审核、Mutex、串行队列、版本检查、支持范围内的 TSan | 原研究：mad 使用 `@unchecked Sendable` 配合锁；标注本身不证明安全。；Swift actor 在挂起期间允许其他任务进入。[官方机制](https://forums.swift.org/t/accepted-with-modification-se-0306-actors/47662) | Swift 与 Kotlin 并发访问均受保护；不持锁等待回调；没有“检查后挂起再使用”的竞态 |
| **14. 页面、账户与应用生命周期** | 分开定义可见、隐藏、销毁、后台、切号；订阅、任务和资源分别绑定正确作用域，并规定关闭顺序与超时 | 生命周期适配器、订阅 token、SwiftUI `.task`、账户 Scope、关闭状态机 | 原研究：MadDemo 关户先等待页面树卸载，再清 ViewModel，最后关闭账户域；卸载等待有 2 秒上限。 | 反复进出页面不重复订阅；切号无旧账户回调；超时分支同样安全 |

## 四 UI 与性能

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **15. UI 输入与权威状态同步** | 区分本地编辑、远端更新和命令受理；使用 revision；保护输入法组合态；按受理结果更新草稿 | 单向数据流、revision、编辑状态机、`markedTextRange`、明确的受理回执 | 原研究：MadDemo 用 `draftRevision` 和组合态判断避免覆盖正在输入的内容；发送后按受理结果清空草稿。 | 中文输入不中断；旧状态不覆盖新输入；发送失败或忙碌时保留草稿 |
| **16. 跨界频率、复制与内存峰值** | 衡量每次调用、每个字段读取和集合转换成本；优先批量接口、分页与增量更新；有证据后再考虑零拷贝 | Instruments、Time Profiler、Allocations、signpost、性能基准、受控批量复制 | 原研究：mad 的 `ByteArray ↔ Data` 使用逐字节循环，是明确可测的成本点，但是否构成瓶颈需测量。 | 针对真实数据量约定延迟、复制字节数、分配量和峰值内存预算 |

## 五 工程交付

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **17. 工具链与插件组合兼容** | 从技术栈规范选择官方 Kotlin/SKIE 配套，CPF 构建不加载 Apple 插件；NativeCoroutines 组合按 X01 选择单一协程桥 | 版本锁、兼容矩阵、插件加载规则、组合消费测试 | 原研究：MadDemo 记录过 SKIE 与构建逻辑中的 KGP 类加载器冲突，以及对 OHOS fork 版本识别不兼容。 | 同一符号/语义阶段不重复生成；Kotlin/Swift NC 组件配套；组合与独立后端均有真实消费证据 |
| **18. 多 Framework 与重复运行时/类型** | 共享依赖的 KMP 模块优先通过 umbrella framework 出口；多 Framework 场景检查重复依赖、类型互通与状态归属 | Gradle 依赖图、选择性 export、链接映射、多入口集成测试 | 原研究：Kotlin 官方说明，多份 KMP framework 包含相同依赖时，会出现重复代码及不兼容的类型/状态副本。[官方说明](https://kotlinlang.org/docs/multiplatform/multiplatform-project-configuration.html) | 跨模块对象可按预期使用；约定的全局服务和原生内核实例没有重复 |
| **19. API、ABI 与行为兼容** | 分开验证源码兼容、二进制兼容和行为兼容；明确联合发布还是独立升级 | 导出头与 Swift API diff、旧消费者重编译、按支持策略重链接/运行、行为契约测试 | 原研究：MadDemo 的 ABI 基线明确只是文本 diff；记录中既有 SKIE 编号变化，也有真实构造器变更。 | 不能因头文件或符号数量不变，就宣称行为与二进制兼容 |
| **20. 构建顺序、缓存与制品配对** | 把绑定、二进制、目标、构建参数和工具链版本作为一组输入管理；明确依赖顺序；失败停止消费旧产物 | Gradle 任务依赖、输入指纹、制品 manifest、摘要校验、干净环境构建 | 原研究：MadDemo 遇到 `env -i` 丢失内核档位，以及 SwiftPM 在预构建动作前解析依赖，导致本地 XCFramework 尚不存在。 | 同一输入得到可追溯产物；切换档位不误用缓存；库和绑定来源一致 |
| **21. SDK 打包与消费兼容** | 分开源码共同构建与公开 Framework/XCFramework；显式配置 SKIE 分发和自定义 cinterop 模块名，实际调用增强入口 | SKIE 分发配置、SwiftPM/XCFramework、独立 Release 消费者、真机/模拟器矩阵 | 原研究：MadDemo 曾出现 SwiftPM 尝试构建缺少 SKIE 模块的 x86_64 切片。 | 不能只以打包或 import 成功放行；检查 unavailable、泛型 lambda、最低系统及声明支持的 Xcode |

## 六 验证与诊断

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **22. 真实消费端与竞态测试** | 同时覆盖 Kotlin 实现、真实 Swift 调用、故障注入及生命周期竞态；将编译、运行、模拟器和真机证据分开 | XCTest、Swift Testing、Kotlin 测试、受控调度、取消/关闭压力测试、适用范围内的 Sanitizer | 原研究：MadDemo 的可空 `MessageSeq` 问题是在真实 iOS 编译时暴露，Kotlin 侧编译不能替代 Swift 消费验证。 | 同步回调、取消与完成相撞、重复关闭、迟到回调等场景可重复验证 |
| **23. 跨语言故障定位** | 关联操作 ID、生命周期和线程信息；保留 Kotlin 原始异常；配对归档二进制、符号及源码版本；诊断载荷脱敏 | CrashKiOS、崩溃平台、dSYM UUID 校验、`dwarfdump`、LLDB source-map、结构化日志 | 原研究：MadDemo 有 LLDB 源码路径映射；Kotlin 异常经过展开后，普通 iOS 崩溃栈可能丢失原始抛出位置。[CrashKiOS 说明](https://crashkios.touchlab.co/docs/misc/THE_PROBLEM/) | 能从故障定位到正确版本与原始异常；符号与二进制 UUID 匹配 |
| **24. 生成器与检查结论的可信度** | 只生成受支持、可机械推导的代码；核对实际导出结果；区分已发现问题、缺少证据和启发式未发现；证据绑定具体制品 | 能力矩阵、确定性生成、正反夹具、漏报探针、证据指纹、Gap 报告 | 原研究：mad-boundary 专门测试“启发式零命中不等于证明不存在”，并维护已知漏报探针。 | 不支持时明确拒绝或要求适配；缺证据不计通过；旧制品测试结果不能冒充新制品证据 |

## 本路线补充

- 反向调用限于配置支持的 ObjC 可见协议或回调；Kotlin 不能因消费端使用 Swift 就直接导入任意纯 Swift 模块。

## 使用方式

按选定 profile 和本次任务读取相关条目；验收采用[公共问题目录](../../../validation/04-problem-catalog.md)、[关键交错场景](../../../validation/08-critical-scenarios.md)与任务 acceptance。本研究不维护独立进度、支持矩阵或第二套模块架构。共同约束与资料优先级见[归档入口](../README.md)。
