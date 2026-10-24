# ArkTS ↔ C/C++ 跨语言边界工程清单

对照基线：MadB 0.9；修订日期：2026-10-24。本文为研究阅读版，保留原问题编号和历史样本；当前实施以权威规范、任务和目标证据为准。表内项目事实沿用原研究快照，本次没有重新验证旧仓库的现状，也不把风险推演当作已发生事故。

主线为目标 HarmonyOS/OpenHarmony Node-API 的 C/C++ 适配；C 与 C++ 分别验证。ANI 属于 X03 独立扩展。

当前选型、关键更新与实施关联见[ArkTS ↔ C/C++ 研究整理](ArkTS_C_CPP_架构总结与新版_mad-boundary_设计输入.md)；精确版本只维护在[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)。原文件、本机路径、旧行号及旧来源映射保存在[更新前快照](../snapshots/2026-10-24-before-refresh.zip)，原始记录不被改写。

本页共 30 项，稳定来源身份为 `ArkTS_C_CPP:序号`。[source-coverage.json](../../../catalog/source-coverage.json)记录修订阅读版的路径、行号、摘要及公共问题映射；原始映射可从快照中的 source-coverage.json 复核。

## 一 公开契约与数据

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **1. 运行路线与后端能力** | 主线采用目标 SDK 的 Node-API C/C++ adapter；ANI 独立准入；逐目标、方向和类型登记能力，不从工具名称推导支持 | Node-API、受限 C/C++ 生成、能力探针；AKI/napi_generator/Taihe 仅为生态候选 | 原研究：Taihe 所引用的仓库快照同时列出 NAPI → ArkTS-Dyn 与 ANI → ArkTS-Sta 后端。[Taihe 架构](https://gitcode.com/openharmony/arkcompiler_taihe_ffi_gen) | C 与 C++ 消费分别验证；生态工具有该后端不等于 MadB 已集成；同一注册或释放阶段只有一个责任方 |
| **2. 接口权威源与编译上下文** | 每条边界确定头文件、IDL 或声明文件中的一个权威源；C/C++ 分析使用实际宏、包含路径、语言标准与目标 | Clang LibTooling、compile_commands.json、生成物基线、配置矩阵 | 原研究：LibTooling 使用编译数据库取得源文件对应的编译参数。[Clang 文档](https://clang.llvm.org/docs/LibTooling.html) | 同一头文件在不同 feature 下的形状可追溯；不通过正则解析任意 C++；不手工维护三份完整接口 |
| **3. 复杂 C++ 类型与公开模型** | 内部保留模板、STL 和继承；导出受支持的 DTO、枚举和资源代理；必要时增加薄适配层 | 受限 Facade、类型能力表、AKI 绑定、Taihe 投影 | 原研究：AKI 提供函数和对象绑定；Taihe 可由契约生成 C++ 投影与语言桥。[AKI](https://gitee.com/openharmony-sig/aki/blob/fe77d84fb20735fc9ce53ccc2e29ee7be8acf65c/README.md)、[Taihe](https://gitcode.com/openharmony/arkcompiler_taihe_ffi_gen) | 不将虚表、迭代器、任意模板实例和内部对象图直接作为公共协议；资源代理不被误当独立快照 |
| **4. 数值 空值与稳定 ID** | 在收窄转换前检查类型、整数性、范围与单位；明确空值和缺失；任意 64 位整数使用受检字符串或目标支持的 BigInt | 范围校验器、不透明 ID、边界值夹具、显式三态修改模型 | 原研究：旧 mad 的 read_int64_property 忽略转换状态，失败可能保留默认值；MadDemo 的相邻边界改用字符串订阅 ID。 | 覆盖 NaN、无穷、小数、负值、2^53 精度边界和空值；转换失败不伪装成有效 0；ID 与指针分开 |
| **5. 字符串 字节与 JSON** | 区分文本编码、字节长度和终止符；字节优先 ArrayBuffer / TypedArray；JSON 按明确协议使用 | 长度受限转换、UTF-8 样例、类型化 DTO、空缓冲测试 | 原研究：AKI 区分字符串与二进制缓冲；Node-API 可直接创建对象和缓冲，不要求统一 JSON。[AKI 类型说明](https://gitee.com/openharmony-sig/aki/blob/fe77d84fb20735fc9ce53ccc2e29ee7be8acf65c/README.md)、[Node-API](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-introduction.md) | 中文、嵌入零字符、空数据与大载荷正确；不会按 strlen 处理任意二进制；JSON 不掩盖类型缺口 |
| **6. 输入校验 错误与异常** | 检查参数数量、N-API 状态、句柄类型与资源状态；统一 C 错误码、C++ 异常和 ArkTS 错误；禁止异常逃出受控边界 | 生成参数校验、错误目录、try/catch 边界、无异常构建的状态返回协议 | 原研究：MadDemo Init 捕获 C++ 异常，注册后检查挂起的 N-API 异常。 | .d.ts 之外仍校验运行时输入；失败不变成空结果；noexcept 不被当作自动捕获；越界内存不靠 catch 修复 |

## 二 调用与异步协议

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **7. 阻塞性与异步执行** | 声明调用执行器和结果执行器；计算、IO、关闭等重工作进入受控线程；异步前准备具有足够寿命的输入 | napi_async_work、后端异步包装、原生工作队列、耗时预算 | 原研究：AKI 提供 Promise 接入；OpenHarmony 异步工作分离工作线程与结果交付。[AKI](https://gitee.com/openharmony-sig/aki/blob/fe77d84fb20735fc9ce53ccc2e29ee7be8acf65c/README.md)、[异步工作](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/use-napi-asynchronous-task.md) | 包装成 Promise 不等于所有同步重工作都已迁出；工作项不失去 owner；队列容量受控 |
| **8. 反向回调与完成阶段** | 区分通知、异步请求和同步返回；明确返回值表示排队、接收、执行还是持久化 | 回调 Adapter、异步应答、请求关联、完成阶段契约 | 原研究：旧 mad 的台账回调要求实际执行后才返回提交结果。 | 不把 TSFN 入队成功当成业务成功；需要同步结果时不能擅自改成 fire-and-forget |
| **9. 取消 超时与后台收尾** | 区分停止观察、停止等待、协作停止工作与撤销副作用；取消后仍为在途调用保留资源 | 原生取消令牌、原子标志、操作对象、终态确认 | 原研究：旧 mad 的 close 只发起后台关停；MadDemo 网络等待与推理停止采用不同协议。 | 请求取消返回不表示 native 已停止；超时后输入、回调上下文与迟到结果仍有管理者 |
| **10. 注册 完成与迟到结果** | 先登记再触发；同步首回调、完成、失败和取消通过统一认领；迟到资源结果按协议释放 | Operation 状态机、完成门、原生资源保活、屏障测试 | 原研究：MadDemo 请求表先登记等待者再发送，避免响应早于发送返回。 | 终态只交付一次；重复回调不重复释放；退出页面后的结果不会访问旧 UI |
| **11. 重入 锁顺序与死锁** | 默认不持业务锁调用外部代码；审查必要例外；避免 ArkTS 线程与 C++ 线程双向同步等待 | 锁边界检查、重入协议、线程断言、可控交错测试 | 原研究：旧 mad 明确提示主线程与等待台账结果的工作线程可能互等。 | 回调内取消、退订、关闭不自等待；异常退出释放锁；禁止用递归锁掩盖协议缺口 |
| **12. 事件容量 顺序与恢复** | 为状态、可靠事件、可丢事件分别指定容量和满队列策略；业务顺序显式编号；定义生产者反馈 | TSFN、有界队列、批处理、序号、溢出反馈与快照恢复 | 原研究：旧 mad 台账 TSFN 使用有限容量且不丢请求；MadDemo 存储变化按 commit_seq 发现缺口。 | 队列满和入队失败有动作；不在关键线程阻塞等待满队列；多生产者不依靠调度顺序表达业务顺序 |

## 三 所有权 内存与多线程

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **13. 所有权与分配器配对** | 每类资源明确创建者、借用者、接收所有权者及释放入口；使用对应库的释放函数 | C++ RAII、unique_ptr 自定义 deleter、受限资源包装、所有权声明 | 原研究：C/C++ 输出指针需要额外契约；仅有 Image* 不能推断应使用 delete、free 还是 image_release | 成功、错误、取消和环境退出路径均释放一次；不跨不兼容分配器释放；不直接向 ArkTS 暴露地址数字 |
| **14. 借用指针 引用与异步保留** | 明确输入与返回借用的有效期；异步前复制、转移或持有有效 owner；禁止悬空引用逃逸 | std::string、std::vector、受控共享缓冲、借用适配器 | 原研究：AKI 提醒 const char* 按引用传递，异步场景应使用适当的值传递。[AKI 约束](https://gitee.com/openharmony-sig/aki/blob/fe77d84fb20735fc9ce53ccc2e29ee7be8acf65c/README.md) | 不保留临时 c_str、栈变量地址或失效 span；取消等待不会提前释放 native 仍在读取的输入 |
| **15. 缓冲布局 可变别名与零拷贝** | 校验长度、偏移、对齐、端序和大小计算；默认复制；零拷贝单独声明共享访问和释放协议 | ArrayBuffer / TypedArray Adapter、受检算术、布局探针、内存测试 | 原研究：AKI 的缓冲访问暴露底层地址，不表示业务可任意释放；Node-API 指南强调正确管理参数和对象。[AKI](https://gitee.com/openharmony-sig/aki/blob/fe77d84fb20735fc9ce53ccc2e29ee7be8acf65c/README.md)、[开发规范](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-guidelines.md) | 不把任意字节地址直接强转为结构体；并发修改有同步；视图不会超过底层数据的有效期 |
| **16. 关闭责任与在途操作** | 一个明确 owner 发起资源关闭；关闭先停止准入，再按约定等待或终止在途使用；保留最终清理责任 | 幂等 close、状态机、使用租约、关闭结果、超时诊断 | 原研究：旧 mad 的关闭入口发起后台流程后立即返回，显示请求与完成应分开。 | 关闭超时不意味着可直接 delete；多个包装不竞争销毁；释放前确认线程、回调和外部使用均已结束 |
| **17. 包装对象 引用环与 GC** | 区分 ArkTS 包装、napi_ref、native 对象和业务资源；明确 wrap/unwrap 或后端等价机制的拥有关系；解除跨语言互持 | 后端对象包装、引用登记、弱引用、显式退订、资源计数 | 原研究：Node-API 提供作用域与引用机制；MadDemo 相邻边界通过显式退订管理监听器。[生命周期](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/use-napi-life-cycle.md)、 | 引用保活不等于业务已关闭；GC finalizer 不承担所有确定性清理；包装类型验证防止错误解包 |
| **18. env 线程与多环境隔离** | 每个环境管理其对象、回调和引用；原生线程处理原生数据，通过环境安全通道交付 | EnvScope、TSFN、线程断言、Worker 消费夹具 | 原研究：OpenHarmony 明确要求对象在所属环境访问；MadDemo 显式建立回到 ArkTS 主线程的通道。[官方 FAQ](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md)、 | 不共享全局 napi_value；napi_ref 保活不授予跨线程权限；一个 Worker 退出不误关其他环境的服务 |
| **19. 环境退出与清理顺序** | 停止新投递；区分源端保留、在途回调和排队载荷；按后端协议清理 TSFN/引用，失效 env 不参与最后清理 | cleanup hook、退出状态机、原生清理 owner、后端 RAII 配合 | 原研究：官方 FAQ 列出环境失效及清理钩子的使用问题；旧 旧 mad-boundary 曾有相关启发式规则。[FAQ](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md)、 | 源端、队列、结果各有清理者；不能等待已退出的 ArkTS；按 CRIT-03/11 注入退出和满队列 |
| **20. C++ 共享状态与析构位置** | 对共享对象显式同步；析构、取消查询和回调遵守各自执行约束；重清理使用可控入口 | mutex、atomic、串行队列、析构线程测试、显式 close | 原研究：shared_ptr 管理共享所有权，不自动保护被指向对象的业务状态；旧 mad 使用互斥与条件变量协调回调结果。 | 不因共享指针可复制就允许并发改对象；析构不假设位于主线程；跨回调访问的状态有同步依据 |

## 四 UI 集成与性能

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **21. 页面观察与共享资源** | 区分页面观察、原生订阅、业务资源与环境；页面仅清理自己拥有的观察；共享对象由 SDK 或账户 owner 管理 | Subscription、ArkUI 生命周期接入、共享分发、页面代次 | 原研究：MadDemo 页面在 aboutToDisappear 解除观察，原生存储另有常驻订阅约束。 | 页面重进不重复登记底层订阅；旧页面结果不写新页面；异步渲染使用完成前不关闭图像等资源 |
| **22. 跨界频率与内存预算** | 优先批量、增量、受限快照和工作队列预算；测量后引入缓冲复用或零拷贝 | 设备性能分析、原生计数、队列水位、真实载荷基准 | 原研究：图像帧、音频块与大量小回调都需同时考虑转换、调度和积压，单次函数微基准不足以代表端到端性能 | 覆盖吞吐、尾延迟、主线程耗时、复制量和峰值内存；慢消费者与持续运行不导致无界增长 |

## 五 工程交付

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **23. SDK 编译器与标准库 ABI** | 固定 SDK、目标架构、C++ 标准、异常和 RTTI 等构建选项；审查三方库所需运行库；明确跨二进制接口 | 工具链锁、CMake 配置、ELF 依赖检查、ABI 消费夹具 | 原研究：OpenHarmony 系统与应用使用不同 C++ 标准库，并记录过标准库版本导致 __emutls_get_address 缺失。[官方机制](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/c-cpp-overview.md) | 不混用系统与应用标准库；不假定 Android 或桌面库可直接使用；同头文件不等于二进制兼容 |
| **24. API ABI 与行为演进** | 分别检查 ArkTS API、公开 native ABI、类型布局和行为保证；接口权威源变更驱动所有派生物更新 | 头与声明基线、符号和布局探针、旧消费者测试、契约版本 | 原研究：Taihe 以契约生成投影与桥接；兼容策略仍需要覆盖接口与行为，而非只检查生成成功。[Taihe](https://gitcode.com/openharmony/arkcompiler_taihe_ffi_gen) | 签名不变但取消、线程或关闭语义改变也能被识别；不把裸结构体内存镜像当作跨版本序列化协议 |
| **25. 打包 名称与依赖装载** | 将声明、桥接库、底层库、运行库和构建身份配套交付；检查目标名、模块名、包声明与导入名 | hvigor / ohpm 消费、CMake、制品清单、ELF 依赖与符号门、干净环境构建 | 原研究：MadDemo 的模块名与包及 libraryName 需保持一致，装载时检查必需入口。 | 最终包声明/适配库/底层库匹配；bootstrap 核对实际装载身份；错库按 CRIT-12 拒绝，不能只检查打包目录 |
| **26. 动态卸载与重复实例** | 库保持装载直到其函数、回调、对象虚表和执行线程不再被使用；明确静态链接和多模块加载的实例归属 | 装载策略、依赖图、模块引用管理、初始化和卸载测试 | 原研究：MadDemo 将 Kotlin 共享库保留到进程退出，因为后续回调仍位于库内。 | 不在函数指针或对象仍存活时 dlclose；不因复制一份库产生两套互不兼容的句柄或全局服务 |

## 六 验证与诊断

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **27. 双向真实消费** | 将 C/C++ 单测、绑定测试、ArkTS 编译和设备调用分层；包含正向调用与反向宿主请求 | 最小 ArkTS 应用、设备运行器、绑定夹具、错误注入 | 原研究：AKI 有用户报告跨界回调进入 Taskpool 后出现非并发函数或序列化错误，说明可调用不等于可迁移。[上游问题](https://gitee.com/openharmony-sig/aki/issues/IB392X) | 普通 TypeScript 或桌面 Node 测试不代替目标 ArkTS；Worker、Taskpool 场景按承诺能力分别测试 |
| **28. 内存 竞态与失败路径** | 覆盖错误分配、坏输入、同步首回调、重复完成、取消、关闭与环境退出交错；验证释放配对 | Clang 静态检查、适用的 ASan / UBSan、模糊测试、屏障与压力夹具 | 原研究：ASan 可检测多类越界、释放后访问与非法释放；具体使用受目标运行时和工具链支持限制。[ASan](https://clang.llvm.org/docs/AddressSanitizer.html) | 所有权和借用协议有针对性测试；未插桩三方库与工具未覆盖的范围如实记录 |
| **29. 跨语言故障定位** | 关联环境、操作、资源及构建身份；保留错误类别；按实际发布二进制归档符号 | HiLog、原生结构化日志、ELF build-id、符号化、资源计数 | 原研究：官方 FAQ 说明崩溃栈出现在 Ark 运行库时，仍需检查原生调用中的线程与非法入参。[排查说明](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md) | 能定位哪个绑定、实例和制品失败；符号与二进制匹配；环境退出后的日志不依赖 ArkTS 回调 |
| **30. 生成与验证结论可信度** | 确定性生成，保留权威源和编译配置；区分声明、启发式观察、AST 结果与真实运行证据 | 再生成 diff、元数据、正反夹具、Findings / Gaps、制品摘要 | 原研究：NapiScanner 以源码模式判断局部风险；hook 与 release 同时出现不证明所有路径安全。 | 未匹配、未运行与零告警分别报告；宏、跨文件释放和后端运行时不被静默视为已验证 |

## 本路线补充

- 借用、分配器、C++ 标准库和异常开关继续作为原生能力前提；装饰器或 RAII 不能推断任意外部指针安全。

## 使用方式

按选定 profile 和本次任务读取相关条目；验收采用[公共问题目录](../../../validation/04-problem-catalog.md)、[关键交错场景](../../../validation/08-critical-scenarios.md)与任务 acceptance。本研究不维护独立进度、支持矩阵或第二套模块架构。共同约束与资料优先级见[归档入口](../README.md)。
