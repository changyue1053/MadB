# Kotlin/Native ↔ ArkTS 跨语言边界工程清单

对照基线：MadB 0.9；修订日期：2026-10-24。本文为研究阅读版，保留原问题编号和历史样本；当前实施以权威规范、任务和目标证据为准。表内项目事实沿用原研究快照，本次没有重新验证旧仓库的现状，也不把风险推演当作已发生事故。

主线采用 CPF-KMP-CMP 工具链 + akinterop，CMP 仅在共享 UI 配置启用；KNOI 为 X08 独立后端，不串接到 akinterop 后面。

当前选型、关键更新与实施关联见[Kotlin/Native ↔ ArkTS 研究整理](Kotlin_ArkTS_架构总结与新版_mad-boundary_设计输入.md)；精确版本只维护在[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)。原文件、本机路径、旧行号及旧来源映射保存在[更新前快照](../snapshots/2026-10-24-before-refresh.zip)，原始记录不被改写。

本页共 28 项，稳定来源身份为 `Kotlin_ArkTS:序号`。[source-coverage.json](../../../catalog/source-coverage.json)记录修订阅读版的路径、行号、摘要及公共问题映射；原始映射可从快照中的 source-coverage.json 复核。

## 一 公开契约与数据

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **1. 调用路径与公开 API 范围** | 使用 CPF-KMP-CMP + akinterop 主线；CMP 按共享 UI 需求启用；逐接口选择类型化互调、受检载荷或二进制；KNOI 单独准入 | CPF 配套、akinterop 导出声明、Facade、能力探针；X08 KNOI 独立配置 | 原研究：MadDemo 的 `OhosPlatformFacts` 直接提供类型化宿主方法；会话页使用 subscribe / unsubscribe / dispatch 加 JSON 载荷。 | 每个接口的调用路径明确；公开声明和运行时入口一致；业务不直接操作 N-API 句柄 |
| **2. enum、sealed、泛型与语言限制** | 以锁定版本的双向能力探针为准；复杂变体采用显式标签及受检字段；只生成目标 ArkTS 能编译的类型 | akinterop 导出探针、能力矩阵、受限 class / DTO、判别字段校验 | 原研究：MadDemo 曾用 `toString()` 表达消息状态，后改为 `type + attempts + rejectCode`；避免 Kotlin 数据类文本变化改坏协议。 | 带数据变体完整保真；泛型擦除或不支持的形状明确报错；不把某版本限制写成语言永久限制 |
| **3. 整数精度与数值收窄** | 区分 int32、safe-int53、int64 / uint64；在发生有损转换前校验；全范围 64 位数采用支持该范围的 bigint 通道或规范十进制字符串 | `Number.isInteger`、`Number.isSafeInteger`、范围检查、`WireSeq`、数值边界样本 | 原研究：订阅 ID 曾存在 int32 截断后命中其他订阅的风险，改成不透明字符串；`physicalMemoryBytes` 直接互调为 bigint，会话页 seq 则声明为 0 到 2^53−1。 | 大整数、小数、NaN、无穷值与越界值不被静默截断；不能先转成 number 丢精度，再转字符串 |
| **4. null、undefined、缺字段与默认值** | 分别声明缺失、显式 null、空串和默认值；参数转换失败不得变成合法业务输入 | 受检解码器、存在性检查、参数错误检查、缺失值测试 | 原研究：MadDemo 探针发现数字载荷被转换成空串，旧逻辑再当作空对象而误接受 Send；入口随后明确拒收空载荷与转换失败。 | 错类型不被默认值掩盖；必填可空字段仍要求字段存在；取消、失败不编码为空结果 |
| **5. 对象身份、业务 ID 与运行时句柄** | 不依赖跨语言代理对象的引用相等；业务 ID 使用稳定表示；页面和订阅句柄绑定所属会话及有效期，复用时考虑 generation | 强类型 ID 外壳、句柄登记表、作用域校验、generation / epoch | 原研究：MadDemo 用 page ID 定位 Presenter、subscription ID 定位收集任务；Presenter 的生命周期仍归页面所属 owner。 | 旧页面回调不更新新页面；重复退订安全；业务 ID 可按领域规则持久化，运行时句柄不得冒充可恢复身份 |
| **6. JSON 校验与两端解析差异** | schema 定义字段、标签、未知字段策略、数值格式、字符串与数组上限；明确长度单位和特殊字符策略；解析后逐字段校验 | 受限 JSON Schema、kotlinx.serialization、schema 驱动生成器、交叉样本、畸形输入测试 | 原研究：MadDemo 从 `JSON.parse(...) as NativeState` 改为生成的受检解码；状态未知字段忽略、未知标签拒收；命令多余字段拒收。 | `as` 不充当校验；范围、缺失值、数组元素、变体组合均检查；大载荷和特殊字符有边界用例 |

## 二 调用与异步协议

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **7. Kotlin 异常、ArkTS Error 与 pending exception** | 统一错误类别与稳定码；检查 N-API 返回状态及挂起异常；同步调用向当前调用方报告，异步投递走明确错误通道；禁止异常穿越不支持的 ABI | 类型化错误、`napi_status` 检查、pending exception 检查、错误适配器、原始原因记录 | 原研究：MadDemo 把数字当监听器的探针先触发 Kotlin 空指针；只捕获 Kotlin 异常后，又因挂起 N-API 异常触发 jscrash。 | 错误不被吞成成功；异步失败不遗留异常污染后续调用；清理异常时保留诊断信息 |
| **8. suspend、Promise 与回调协议** | 单次调用只完成一次；允许契约声明同步完成；注册、取消和首次回调可重入；Promise 与协程需显式建立生命周期及错误映射 | Promise Adapter、`suspendCancellableCoroutine`、原子终态机、两阶段注册、资源交接清理 | 原研究：mad-boundary 的 `observe()` 可同步回放该快照中的值，用户回调可能在 observe 返回前执行，不能假定此时外部订阅变量已经赋值。 | 同步首回调、重复完成、回调抛错、取消与完成相撞都可验证；有 Promise 包装不等于自动拥有完整取消语义 |
| **9. 取消、超时与释放引用** | 区分停止观察、停止等待、请求停止工作、撤销副作用；单独管理回调引用和 native 资源；结果未知时使用查询或幂等机制 | 订阅 token、Job 取消、操作 ID、取消握手、终态机；引用计数与删除独立处理 | 原研究：MadDemo 的 unsubscribe 取消 Flow 收集，不负责撤销已发送动作；OpenHarmony 对 async_work 的取消结果有专门说明。[异步任务规范](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/use-napi-asynchronous-task.md) | 不把超时当工作已停；不把 unref 当取消；超时不能授权释放仍被执行线程使用的内存 |
| **10. Flow、状态与事件交付** | Flow 映射为声明清楚的订阅协议；状态允许合并，事件另定回放、顺序、容量、丢弃和确认策略；编码失败、协议失败、投递失败分别处理 | StateFlow、监听器、订阅登记表、序列号、受控队列、事件 ack | 原研究：MadDemo 将编码失败从退订条件中拆出：普通编码失败跳过本帧，投递失败才终止订阅；越界 seq 按领域决策降为 null，避免页面长期停在旧帧。 | 慢消费者不导致无界积压；状态与可靠事件的保证不同；数据降级不能由生成器自行猜测 |
| **11. 动作已转交、业务已受理与业务已完成** | 为同步入口返回值、受理回执和最终结果分别定义语义；必要时通过 request ID 关联；重试遵守业务幂等规则 | 类型化 receipt / outcome、操作 ID、持久化回执、状态机 | 原研究：`conversationDispatch()` 返回 true 表示载荷可解码、页面存在并已调用 Presenter；它没有等待落库或服务端确认。发送草稿按后续业务受理结果清空。 | true、Promise resolve 或成功回调究竟代表哪一阶段，文档和测试一致；UI 不提前显示成功 |

## 三 所有权 内存与线程

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **12. napi_env、Worker 与线程归属** | 普通 ArkTS 对象与 env 绑定；按环境隔离上下文；跨 Worker 选择受支持的序列化、转移或 Sendable 机制；明确禁止的调用场景 | env context、线程断言、环境 generation、TSFN、Worker 消息；受支持的 Sendable API | 原研究：MadDemo 使用 `mainEnv` 与主线程宿主代理，是单主环境使用方式；OpenHarmony 禁止普通 Node-API 对象跨引擎实例访问。[官方规范](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-guidelines.md) | 不将全局 mainEnv 设计默认为支持任意 Worker；对象、引用、回调属于同一有效环境 |
| **13. Main 调度、阻塞与跨语言死锁** | 在正确 env 上初始化主循环调度；UI 入口不得同步等待需要回到该线程的任务；阻塞 I/O 与计算移出 UI 线程，同时遵守对象访问约束 | `initMainHandler(env)`、Dispatcher、线程守卫、异步宿主接口、等待依赖检查 | 原研究：mad 的主循环桥记录：未初始化时 fork 调度器可能回退到调用线程，引发 ArkUI 跨线程中止；台账桥也注明主线程与工作线程反向等待会死锁。 | `Main` 名称与实际线程一致；主线程不 runBlocking 等待自己；切到后台不能顺便带走 ArkTS 代理对象 |
| **14. TSFN 队列、生产者与退出竞态** | 按条目数和字节数约束队列；明确满载策略、生产者计数、源端与排队载荷 owner；退出时保持原生清理通路 | TSFN acquire / release / abort、受控队列、计数台账、关闭状态机、环境清理钩子 | 原研究：mad 的台账 TSFN 容量为 8；销毁期间 env 或回调为空时仍需唤醒等待线程，不能让它永久等待。 | CRIT-01/03/11 验证排队后停止、源端已取 context 和 env 退出；不能在消费线程阻塞等空位 |
| **15. napi_value、napi_ref、StableRef 与 GC** | 局部值遵守 handle scope；跨调用保活使用受管引用；引用计数、引用句柄删除和 Kotlin StableRef 释放分别配对；解除跨语言引用环 | handle scope、`napi_create_reference` / `napi_delete_reference`、StableRef owner、显式退订、引用诊断 | 原研究：mad 的 ProductHandoff 用 StableRef 交接，壳取出后立即释放桥接引用；mad-boundary 的页面客户端把观察关系与账户资源分开。 | 局部句柄不跨 scope 使用；保活引用成对清理；正常 GC / finalizer 不能直接算作泄漏 |
| **16. ByteArray、ArrayBuffer 与零拷贝** | 为二进制接口声明长度、复制、借用、转移、共享及释放责任；TypedArray 同时校验偏移和长度；异步借用需有保活协议 | ArrayBuffer / TypedArray、受检批量转换、外部缓冲区 finalizer、必要时内存复制 | 原研究：akinterop 1.1.0 发布源码中的 TypeConverter 为 ByteArray 提供 ArrayBuffer 转换路径，说明二进制不必经过 JSON/base64；具体复制与保活仍应核对底层实现。 | 不越界、不重复释放；转移或分离后的缓冲区不再被旧侧使用；不能只因用了 ArrayBuffer 就宣称零拷贝 |
| **17. 关闭协议与环境销毁** | 资源 owner 控制关闭；先封闭准入，再停止生产者、协调在途操作和回调，最后按允许的环境释放；业务关闭与 env 清理分别建模 | owner Scope、关闭状态机、cleanup hook、环境 token、在途计数、超时报告 | 原研究：OpenHarmony 文档说明，创建 TSFN 不自动注册 cleanup hook。mad-boundary 将 env 清理纳入 N-API 规则。[平台说明](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/reference/native-lib/napi.md#napi_create_threadsafe_function)、[清理钩子](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/use-napi-about-cleanuphook.md) | env 失效后不访问 ArkTS；仍在执行的任务有明确收尾；cleanup hook 不等于页面生命周期或任意退出场景下的持久化保证 |

## 四 UI 与性能

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **18. 页面可见性、销毁与前后台** | 分别定义显示、隐藏、销毁、应用后台的订阅策略；取消定时器与延后任务；页面只释放自身观察关系 | `aboutToAppear` / `aboutToDisappear`、宿主生命周期转发、Subscription、可见性状态机 | 原研究：mad-boundary 的 ArkUI 客户端分开提供 setVisible / setForeground / destroy；页面退出不调用账户端口 close。 | 反复进入不重复订阅；隐藏和销毁行为可区分；旧定时器不操作新页面；页面关闭不误关账户 |
| **19. 输入回声、焦点与 Compose 混排** | 区分本地编辑与权威更新；按 revision 更新草稿；明确焦点、触摸分发、输入法组合态和布局完成时机 | draftRevision、输入状态机、ArkUIView 交互配置、输入法与旋转测试 | 原研究：MadDemo 的 ArkUIView 使用 stopPropagation，避免 Compose 宿主抢走触摸与输入焦点；原生页按 draftRevision 更新输入框。 | 中文组合输入、回车、焦点切换、旋转后草稿保留均验证；旧状态不覆盖正在输入的内容 |
| **20. ArkUI 响应式模型与列表身份** | wire DTO 保持与 UI 框架解耦；按 V1 / V2 生成或提供对应 ViewState 适配；优先稳定 ID 加可观测更新，必要时用 revision 强制重建 | V1 `@State` / `@Observed` / `@ObjectLink`；V2 `@ObservedV2` / `@Trace`；Selector、列表 diff | 原研究：MadDemo 手测发现只以 id 为键时 Pending → Sent 不刷新，改为键包含状态字段；这是该页面的修复方式，需要评估重建带来的局部状态损失。 | 列表字段变化能刷新，滚动与局部状态稳定；把 JSON 断言为 class 不会自动获得响应式装饰器行为 |
| **21. 高频跨界、序列化与 UI 线程预算** | 测量端到端开销；减少逐字段互调；按状态语义合并、分页或增量传输；在安全隔离下预编码；为队列和载荷设预算 | DevEco Profiler、HiTrace、hilog 计数、批量接口、Selector、性能基准 | 原研究：MadDemo 在 Main 收集整份 UiState，逐行映射并编码 JSON，再由 ArkTS 解码、更新列表；另有宿主同步 flushSync，均应测量实际耗时。 | 明确帧大小、更新频率、P95 延迟、UI 线程耗时、队列深度与内存峰值；没有测量前不把潜在成本称为瓶颈 |

## 五 工程交付

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **22. Kotlin fork、KSP 与 ArkTS 工具链组合** | 官方 Kotlin 与 CPF 分开 Gradle 根、插件、输出与缓存；分别运行 KSP2/KSP1 前端；精确配套集中引用技术栈规范 | CPF profile、独立 Wrapper、KSP 前端适配、公共依赖与设备探针 | 原研究：MadDemo 的 akinterop 1.1.0 使用 KSP1，KSP 分析任务 API 级别降至 1.9，而真正 Kotlin 编译仍按 fork 2.2；不能只升级一个插件。 | 不跨编译器复用不兼容 KLIB；不全局替换版本掩盖错制品；生成代码在真实 ArkTS 目标编译 |
| **23. 模块加载、注册与初始化顺序** | 校验 so 名称、SONAME、模块名与注册符号；定义重复初始化和失败回滚；按实际依赖顺序初始化调度器、Compose、导出表与页面 builder | CMake、`dlopen` / `dlsym`、`napi_module_register`、初始化状态机、加载诊断 | 原研究：MadDemo 装载壳加载 `libmad_shell.so` 并检查两个注册符号；原生页面 builder 必须在对应 Compose 页面创建前登记。 | 分别验证 akinterop 的桥注册与加载；KNOI 的预载/initBridge 问题只归其独立配置；重复初始化、失败回滚和库寿命有证据 |
| **24. 声明、wire 与行为兼容** | 分别检查 d.ts 调用面、native 导出、wire 载荷和业务语义；为未知标签、字段演进及旧客户端定义策略；生成物禁止手改并检查漂移 | d.ts 基线、schema diff、golden、旧消费者测试、`tools/wire.sh check` | 原研究：MadDemo 在 d.ts 不变的情况下调整了会话页 JSON 的类型和校验，说明签名不变不代表载荷或行为兼容。 | 新旧双方按支持策略互通；字段、标签和整数语法变化都被评审；签名检查不能代替 wire 检查 |
| **25. HAR / HAP、native 依赖与制品一致性** | 声明、K/N so、原生内核及载荷生成物成组追溯；官方与 CPF 使用独立坐标和根元数据；组合根保证既定实例归属 | Gradle 与 hvigor 任务依赖、manifest / digest、SONAME、ELF 依赖与链接映射、HAP 解包检查 | 原研究：MadDemo 用组合根链接唯一 K/N so；构建流程复制 so 和声明，symbols 检查读取最终 HAP 内的库。脚本同时说明 release 构建仍采用社区调试签名材料。 | 最终 HAR/HAP 的库和声明匹配；正式 SDK、Release 与生产签名分别验证；Beta 本机结果不代表正式分发通过 |

## 六 验证与诊断

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **26. 两端测试与真实 ArkTS 消费** | 分开覆盖编码与解码、strict 编译、真实 N-API 调用、生命周期竞态和设备运行；拒收样本包含边界值与错误监听器 | Kotlin / JVM 与 JS 差分测试、hvigor CompileArkTS、Hypium / UI 测试、hdc、故障注入、支持范围内的 Sanitizer | 原研究：mad-boundary 的 D 阶段分别记录 JVM / Node 协议测试及真实 HAR strict 编译；MadDemo 的错误监听器崩溃只能由实际桥接探针验证。 | Node 测试不能替代 ArkTS / N-API 测试；编译通过不能代替运行；同步回调、关闭竞态、队列满和 env 退出有明确用例 |
| **27. 跨语言日志、崩溃与符号化** | 关联 operation / subscription / env / generation；记录编码失败、投递失败和队列状态；保留原始错误来源；配对归档 so 符号与包摘要，日志脱敏 | hilog、HiTrace、jscrash / cppcrash、faultlog、DevEco 符号化、LLVM 工具、Build ID / 摘要核对 | 原研究：MadDemo 的同一类错误监听器问题分别出现过 cppcrash 和 jscrash；K/N 故障探针曾只给出 so 地址，需要对应符号定位。[平台崩溃说明](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/dfx/cppcrash-guidelines.md) | 能定位错误属于哪层、哪个操作和哪个制品；符号与随包二进制匹配；不把正文、凭据直接写进诊断 |
| **28. 生成器与检查证据可信度** | 明确生成器能力边界；验证输出确定性与实际导出结果；区分错误、缺证据和启发式未发现；测试证据绑定具体输入与产物 | 能力矩阵、正反夹具、受控反例、漂移检查、输入指纹、Gap、已知漏报探针 | 原研究：mad-boundary 用故意违反 strict 的代码验证编译检查确实生效；MadDemo 复盘指出 WireSeq 包装曾让启发式数值检查认不出类型，零命中不能直接算证明。 | 工具不认识的类型不能悄悄算通过；旧证据不能冒充新候选证据；生成器测试通过不覆盖所有手写映射和运行时协议 |

## 本路线补充

- JSON 数字语法支持指数形式；历史协议拒绝 1e3 属于更窄的规范化策略。完整 64 位值不可先经 number 丢失精度再转字符串。
- napi_reference_unref、删除引用、停止业务任务和退订是不同操作；TSFN 不使普通 ArkTS 对象具备跨线程访问能力。

## 使用方式

按选定 profile 和本次任务读取相关条目；验收采用[公共问题目录](../../../validation/04-problem-catalog.md)、[关键交错场景](../../../validation/08-critical-scenarios.md)与任务 acceptance。本研究不维护独立进度、支持矩阵或第二套模块架构。共同约束与资料优先级见[归档入口](../README.md)。
