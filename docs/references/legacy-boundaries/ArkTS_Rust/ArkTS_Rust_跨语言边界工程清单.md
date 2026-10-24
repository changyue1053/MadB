# ArkTS ↔ Rust 跨语言边界工程清单

对照基线：MadB 0.9；修订日期：2026-10-24。本文为研究阅读版，保留原问题编号和历史样本；当前实施以权威规范、任务和目标证据为准。表内项目事实沿用原研究快照，本次没有重新验证旧仓库的现状，也不把风险推演当作已发生事故。

主线为 napi-ohos 直连。复用既有 UniFFI 接口的 ArkTS 生成器由 X07 实验配置承接，ANI 由 X03 承接；三者分别锁定和验证。

当前选型、关键更新与实施关联见[ArkTS ↔ Rust 研究整理](ArkTS_Rust_架构总结与新版_mad-boundary_设计输入.md)；精确版本只维护在[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)。原文件、本机路径、旧行号及旧来源映射保存在[更新前快照](../snapshots/2026-10-24-before-refresh.zip)，原始记录不被改写。

本页共 30 项，稳定来源身份为 `ArkTS_Rust:序号`。[source-coverage.json](../../../catalog/source-coverage.json)记录修订阅读版的路径、行号、摘要及公共问题映射；原始映射可从快照中的 source-coverage.json 复核。

## 一 公开契约与数据

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **1. 接入路线与运行时能力** | 主线选择 napi-ohos；已有 UniFFI 接口的 ArkTS 生成按 X07 独立实验配置，ANI 按 X03；按目标/方向/类型选择，不自动互换 | napi-ohos 及配套宏/build/sys、ohrs；独立 UniFFI-ArkTS / ANI 探针 | 原研究：ohos-rs 提供 Node-API 风格 Rust 绑定；ani-rs 面向兼容 ArkTS 1.2 的 ANI 运行环境。[ohos-rs](https://github.com/ohos-rs/ohos-rs)、[ani-rs](https://github.com/ohos-rs/ani-rs) | 工具可生成、桌面测试、HAP 构建、设备执行分别记录；不继承其他后端认证 |
| **2. 模型权威源与公开 DTO** | 独立 Rust SDK 以明确的 Rust 导出面为源；生成目标 ArkTS 支持的类型；区分值快照和资源代理 | 后端导出宏、生成声明、受限 DTO、必要的 Facade | 原研究：napi-ohos 提供函数和类型接线；MadDemo 现有业务协调仍在 Kotlin，直连不自动继承其业务协议。[后端入口](https://github.com/ohos-rs/ohos-rs)、 | 不导出内部对象图、锁和裸指针；DTO 修改不被误认为写回内核；不重复手写多份权威模型 |
| **3. 整数范围 单位与稳定 ID** | Node-API 路线的任意 i64/u64 不直接映射为 number；采用受检十进制字符串或经验证的 BigInt；业务 ID 与临时句柄分开 | 数值范围转换、格式校验、不透明 ID、边界值夹具 | 原研究：MadDemo 用字符串订阅 ID 避免底层 Int 转换把非法值截断为其他 ID。 | 覆盖 2^53−1、负数、溢出、NaN 和非整数；禁止先损失精度再转字符串；单位不隐式改变 |
| **4. 空值 缺失与带数据枚举** | 明确 Option 的目标表达；三态修改显式建模；带数据 enum 转换为目标编译器支持的标签与载荷结构 | 后端类型能力表、类型 Adapter、ArkTS 编译夹具 | 原研究：ani-rs 的映射包含 i64 → long、可空整数装箱，说明不同运行时必须使用各自类型规则。[ANI 映射](https://github.com/ohos-rs/ani-rs#type-mappings) | 空值不变成 0；未修改与清空不同；标签载荷组合有效；未知标签策略明确 |
| **5. 字符串 字节与 JSON 选择** | 文本与任意字节分开；优先类型化 DTO 和 ArrayBuffer；JSON 用于明确的消息协议或兼容通道；检查编码与长度 | 生成转换、ArrayBuffer、受限序列化协议、Unicode 与空缓冲样例 | 原研究：ohos-rs 说明 N-API Buffer 与 ArkTS 的 buffer 模块不同，建议优先 ArrayBuffer；运行时差异需按版本验证。[Buffer](https://ohos.rs/docs/more/buffer)、[差异说明](https://ohos.rs/docs/more/diff) | 空缓冲、中文、嵌入零字符和大载荷正确；不把字节数当字符数；不默认 Base64 或 JSON |
| **6. 运行时校验 错误与 panic** | 声明文件之外校验范围、大小、句柄状态和调用条件；统一业务错误、取消、宿主异常与内部故障；明确 panic 后状态 | 生成校验器、稳定错误码、Result 映射、异常边界、故障注入 | 原研究：MadDemo 记录过非法监听器与挂起 N-API 异常；Rust 门通过 guarded 限制展开式 panic。 | 错误不静默变成空串或成功；不依赖文案分类；同步调用与异步回调的异常交付方式明确 |

## 二 调用与异步协议

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **7. 阻塞调用与异步运行时** | 区分短同步调用、阻塞工作、Future 驱动与回调投递；为后台任务指定 owner | napi-ohos 异步接线、明确配置的 Rust 运行时、工作队列、耗时检查 | 原研究：ohos-rs 提供 async 接入；MadDemo 推理入口调用同步原生函数。[后端说明](https://github.com/ohos-rs/ohos-rs)、 | 返回 Promise 不被当作同步重工作已离开 ArkTS 线程；派生任务和线程池容量有管理者 |
| **8. 取消 超时与副作用** | 分别声明停止观察、停止等待、请求停止工作和撤销副作用；以底层终态确认取消效果 | 操作句柄、原生原子信号、取消令牌、超时状态机 | 原研究：MadDemo 网络取消只放下等待；推理线程读取取消回调，属于不同取消协议。 | 不把 Promise、超时或取消接口返回成功当作底层停止；取消后留下的工作、缓冲和结果仍有归属 |
| **9. 一次完成与迟到结果** | 先登记操作再触发；成功、失败和取消使用统一完成认领；无人接收的资源结果按协议回收 | 操作状态机、完成门、迟到结果清理、可控交错测试 | 原研究：MadDemo 请求表先登记等待者，防止响应早于发送返回。 | 同步首回调不丢失；终态只认领一次；重复完成不造成重复释放；关闭后不更新失效页面 |
| **10. 反向回调与业务完成阶段** | 区分单向通知、异步请求和同步返回；明确返回值代表入队、接收、执行还是持久化 | 回调 Adapter、异步应答协议、请求关联、原生状态镜像 | 原研究：旧 mad 台账回调跨线程等待 ArkTS 实际执行，不能只入队就返回成功。 | 需要同步结果的回调不能改成异步通知；UniFFI-ArkTS 实验配置对异线程同步回调明确拒绝；取消查询优先使用已声明的原生状态 |
| **11. 同步回调 重入与死锁** | 不持业务锁调用外部代码；不让 ArkTS 线程同步等待必须回到该线程的工作；声明重入规则 | 锁边界审查、执行器约束、重入守卫、超时与屏障夹具 | 原研究：旧 mad 明确提示：工作线程等待台账回调期间，主线程再同步等待该线程就会互等。 | 回调可触发取消、退订或关闭时仍正确；关闭不会阻塞唯一能执行剩余回调的线程 |
| **12. 有界事件 顺序与恢复** | 显式指定队列容量、满队列策略与业务序号；状态可按约定合并，可靠事件不能静默丢弃 | TSFN、有界 Rust 队列、批处理、溢出事件、快照恢复 | 原研究：ohos-rs TSFN 的容量 0 表示无限队列；MadDemo 以 commit_seq 识别存储变化缺口。[TSFN](https://ohos.rs/docs/usage/tsfn)、 | 入队失败有人处理；慢消费者不会无限增长内存；跨生产者顺序不依赖偶然调度；可靠性由端到端协议保证 |

## 三 所有权 内存与多线程

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **13. env 与执行线程归属** | 所属环境的 ArkTS 对象在允许的线程操作；工作线程携带原生数据，通过环境安全的通道回送 | EnvScope、TSFN、线程断言、执行器 Adapter | 原研究：MadDemo 用 MainDispatcherBridge 先建立回 ArkTS 主线程的通道。 | Rust 线程安全不等于 ArkTS 对象可跨线程；不通过 unsafe Send / Sync 强行绕过环境约束 |
| **14. 多环境 Worker 与对象隔离** | 每个环境维护自己的回调、引用和关闭状态；共享 Rust 服务只交换原生数据 | 按环境登记、环境代次、Worker 消费夹具、过期引用检查 | 原研究：官方 FAQ 说明 env 与执行线程绑定，环境退出后继续使用可能失效。[稳定性 FAQ](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md) | 不默认全进程只有一个 main_env；一个 Worker 退出不误关其他环境或应用拥有的服务 |
| **15. 局部句柄 长期引用与引用环** | 区分 handle scope、napi_ref、Rust Arc 与业务 close；注册和解除成对；断开跨语言长期互持 | 后端引用包装、弱引用或显式解绑、引用计数诊断 | 原研究：Node-API 为局部值、长期引用和作用域提供不同机制。[生命周期文档](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/use-napi-life-cycle.md) | napi_ref 延长寿命不授予跨线程权限；引用减计数、删除引用和关闭原生资源不混用 |
| **16. 唯一关闭责任与在途操作** | 明确创建者、关闭者和引用持有者；关闭先停止准入，再按协议处理在途操作；保留完成清理的 owner | 幂等 close、资源状态机、在途保活、关闭结果 | 原研究：MadDemo Rust 句柄管理与 Kotlin 资源生命周期需要保持一致；直连 SDK 也需指定关闭责任。 | 多个包装不产生多个关闭 owner；关闭超时不等于内存可立即释放；逻辑关闭与最后引用释放分别检查 |
| **17. 环境退出与异步清理** | 环境关闭后禁止新投递；停止或脱离生产者；按后端拥有关系处理 TSFN、引用和未投递原生数据 | 受支持的 cleanup hook、关闭状态机、RAII 配合、环境销毁夹具 | 原研究：官方 FAQ 提醒缓存 env 需要处理环境退出；旧 mad-boundary 曾扫描 TSFN 收尾形态。[FAQ](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md)、 | 不在环境失效后创建 ArkTS 值；不等待无法再执行的 ArkTS 回调；不与后端析构重复释放 |
| **18. 缓冲寿命 转移与分配器配对** | 默认复制；零拷贝单独声明拥有者、可变性、有效期和线程；由分配协议指定释放方 | ArrayBuffer 转换、受限外部缓冲、专用释放入口、内存检查 | 原研究：ohos-rs 记录过空缓冲和 TypedArray 行为差异，不能照搬 Node.js 内存假设。[运行时差异](https://ohos.rs/docs/more/diff) | 不把短期借用指针交给异步任务；所有失败路径配对释放；空数组、偏移和长度均覆盖 |
| **19. Rust 共享状态与析构执行位置** | 通过不可变数据、锁或消息串行化控制 Rust 共享状态；明确 Drop、回调与重清理的位置 | Rust Send / Sync 约束、锁与队列、析构线程夹具、显式关闭 | 原研究：MadDemo 推理取消回调仅在推理线程读取原子状态。 | 不持锁调用 ArkTS；析构不假定发生在 UI 线程；后台重清理不会越过环境或资源寿命 |

## 四 UI 集成与性能

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **20. 页面生命周期与共享订阅** | 分开页面观察、SDK 原生订阅、账户资源与运行环境；页面只解除自身观察；多页面共享底层分发 | Subscription 包装、ArkUI 生命周期接入、共享分发器、页面代次 | 原研究：MadDemo 页面在 aboutToDisappear 解除观察，而存储门每库只允许一次常驻订阅。 | 重进页面不重复原生注册；观察停止不误关账户；账户切换后旧事件不写入新页面 |
| **21. 跨界频率 复制与内存峰值** | 优先批量、增量与受限快照；为队列和工作项设置预算；测量后再引入零拷贝 | 设备性能分析、Rust 指标、队列水位、真实载荷基准 | 原研究：推理进度和消息变化适合批量投递；已有存储接口对单批容量作约束。 | 测主线程耗时、吞吐、复制量和慢消费者内存；不以空载调用微基准代替实际场景 |

## 五 工程交付

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **22. 工具链 后端与 feature 组合** | 分别锁定 Rust、SDK、运行时、生成器、补丁和 feature；napi 组件与 ohrs 版本不混为一个版本；UniFFI-ArkTS 使用自身配套锁 | Cargo 锁、后端 profile、目标消费探针；版本数字引用技术栈规范 | 原研究：ani-rs 提醒未启用 async / tokio_rt 时某些异步绑定仍能编译，但 Promise 会立即拒绝。[ANI 异步要求](https://github.com/ohos-rs/ani-rs#async-functions-promise) | 生成器和运行依赖配套验证；异步能力必须真实调用，不能由编译成功推断 |
| **23. OHOS 目标与原生依赖** | 使用匹配 target、sysroot、链接器和最低 SDK；覆盖依赖的 C/C++ 库、运行库及所需符号 | Cargo、ohrs、OHOS SDK、适用的 CMake、ELF 依赖检查 | 原研究：Rust 官方列出 OHOS 目标与交叉编译要求；MadDemo C++ 壳链接 libace_napi.z.so。[Rust 平台说明](https://doc.rust-lang.org/rustc/platform-support/openharmony.html)、 | 不混用 Android 或普通 Linux 库；传递依赖在真实设备装载；各承诺架构分别验证 |
| **24. ArkTS API ABI 与行为兼容** | 分开比较导出 API、原生 ABI、数据表示及取消关闭行为；公共枚举与错误码有演进规则 | 声明基线、符号检查、契约差异、旧消费者测试 | 原研究：mad-boundary 的 ContractReader 与 Verifier 可承接边界差异和消费证据。 | 签名不变不代表行为兼容；不把 napi-ohos 宏版本等同于稳定公共 ABI |
| **25. 声明 库与包的一致性** | 将声明、包装、原生库、依赖清单与构建身份配套发布；使用真实消费工程检查包布局 | ohrs 构建、hvigor / ohpm 消费任务、制品清单、摘要与握手 | 原研究：MadDemo 模块名需与包声明及 libraryName 一致，装载时验证入口符号。 | 声明、adapter 与实际装载库一致；实验 UniFFI 后端另验动态依赖和 checksum；错误库按 CRIT-12 拒绝 |
| **26. KMP 共存与内核实例归属** | 组合根显式选择共享或隔离；审查静态链接、最终 so 与初始化入口；用实例身份和资源协议证明共享 | 链接图、ELF 符号与依赖检查、实例身份、初始化关闭测试 | 原研究：MadDemo 的 Rust staticlib 用于鸿蒙链接期注入；增加独立 Rust .so 后需要审查是否复制全局状态。 | 执行 CRIT-04：各桥看到同一预期实例，句柄不跨隔离实例混用；关闭责任与物理装配一致 |

## 六 验证与诊断

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **27. 真实 ArkTS 双向消费** | Rust 单测、包装测试、ArkTS 编译和设备 FFI 测试分层；覆盖正向调用与反向回调 | ArkTS 最小消费应用、设备运行器、绑定夹具、故障注入 | 原研究：OpenHarmony 提供异步工作和跨线程调用的开发说明，宿主约束需要在目标运行时验证。[异步工作](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/use-napi-asynchronous-task.md)、[线程安全](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/use-napi-thread-safety.md) | napi-ohos 与实验后端各自有目标设备报告；脚本可跳过设备或仅构建时不得记运行通过；Kotlin 中转通过不覆盖直连 |
| **28. 竞态 内存与错误路径** | 控制注册、首回调、取消、完成、退订、关闭、GC 与环境退出的交错；加入非法输入和资源失败 | 屏障夹具、压力测试、模糊测试、目标支持的 Sanitizer、资源计数 | 原研究：MadDemo 已记录错误监听器触发挂起异常，旧 mad 存在需遵守的同步回调等待协议。 | 正常路径之外覆盖队列满、回调抛错、close 超时、迟到缓冲和 Worker 退出；资源释放恰好配对 |
| **29. 跨语言诊断与符号化** | 关联操作、环境、资源和构建身份；保留错误类别；符号归档匹配实际发布二进制 | 结构化日志、HiLog 接入、Rust panic hook、ELF build-id、符号文件与栈解析 | 原研究：MadDemo panic hook 只输出位置与线程，避免输出用户载荷。 | 能定位哪个环境、操作和制品失败；日志不携带敏感载荷；环境失效后日志路径不再调用 ArkTS |
| **30. 生成与验证结论可信度** | 区分声明、静态启发式和运行证据；记录生成器、库及应用包摘要；缺口明确报告 | 确定性生成、元数据读取、正反夹具、Findings / Gaps、证据关联 | 原研究：现有 NapiScanner 主要识别 C/C++ 原始调用形态；宏和后端内部行为需新增元数据与运行验证。 | 源文件同时出现 hook 与 release 不等于所有路径正确清理；无匹配、未运行和零告警分别呈现 |

## 本路线补充

- 既有 C ABI 可作为显式接入方案研究；不自动替换 napi-ohos，也不从其他路线继承认证。

## 使用方式

按选定 profile 和本次任务读取相关条目；验收采用[公共问题目录](../../../validation/04-problem-catalog.md)、[关键交错场景](../../../validation/08-critical-scenarios.md)与任务 acceptance。本研究不维护独立进度、支持矩阵或第二套模块架构。共同约束与资料优先级见[归档入口](../README.md)。
