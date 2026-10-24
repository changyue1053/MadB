# Swift ↔ C/C++ 跨语言边界工程清单

对照基线：MadB 0.9；修订日期：2026-10-24。本文为研究阅读版，保留原问题编号和历史样本；当前实施以权威规范、任务和目标证据为准。表内项目事实沿用原研究快照，本次没有重新验证旧仓库的现状，也不把风险推演当作已发生事故。

Swift↔C 使用 Clang importer 和明确 C 可见导出；C++ 按源码直接互操作、C ABI 分发、Objective-C++ 适配三类正式配置选择。

当前选型、关键更新与实施关联见[Swift ↔ C/C++ 研究整理](Swift_C_CPP_架构总结与新版_mad-boundary_设计输入.md)；精确版本只维护在[技术栈规范](../../../architecture/16-technology-stack-and-versions.md)。原文件、本机路径、旧行号及旧来源映射保存在[更新前快照](../snapshots/2026-10-24-before-refresh.zip)，原始记录不被改写。

本页共 24 项，稳定来源身份为 `Swift_C_CPP:序号`。[source-coverage.json](../../../catalog/source-coverage.json)记录修订阅读版的路径、行号、摘要及公共问题映射；原始映射可从快照中的 source-coverage.json 复核。

## 一 接口与数据

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **1. 路线与类型支持范围** | C 使用 Clang importer/C 可见导出；C++ 按源码直接互操作、C ABI 分发、ObjC++ 适配选择正式配置；分别记录方向和工具链 | Swift/Clang 编译探针、Clang AST、头文件或导出声明基线、能力表 | 原研究：Swift 官方支持双向 C++ 互操作，但支持范围受类型和方向约束。[官方指南](https://www.swift.org/documentation/cxx-interop/)、[支持范围](https://www.swift.org/documentation/cxx-interop/status/) | SwiftPM 正向支持不自动覆盖 C++→Swift；反向使用已验证导出流程；不支持的签名明确拒绝 |
| **2. 值语义 引用语义与模型** | 区分可复制值、借用视图、共享引用和需关闭资源；优先直接使用合适的导入类型；按需增加 Facade 或独立快照 | 编译器导入规则、所有权注解、受限 Swift 包装、复制测试 | 原研究：C++ 类型可按值导入，复制和销毁涉及其 C++ 特殊成员；适用类型也可映射为引用类型。[类型映射](https://www.swift.org/documentation/cxx-interop/) | 复制包装不产生两个关闭责任者；含共享对象的结构不被误称为深度独立快照；不默认生成三套模型 |
| **3. 数值 空值与枚举** | 明确位宽、符号、单位、范围、空值和未知枚举策略；在收窄或无符号转换前校验 | 固定宽度类型、受检转换、Optional 适配、边界值夹具 | 原研究：MadDemo 的原生事件使用固定宽度字段，并检查结构大小和偏移。 | 覆盖极值、负数、无效枚举和空指针；size_t 转 Int32 不静默截断；Int64/UInt64 不因其他语言习惯一律改成字符串 |
| **4. 字符串 字节与布局** | 文本声明编码和长度单位；二进制显式携带长度；读取结构或零拷贝前检查大小、对齐、初始化和别名条件 | String、Data、长度受限转换、受检算术、布局探针 | 原研究：mad_link.h 明确字符串采用指针与长度，并在返回前拷贝；业务额外限制字符串内容。 | 不以 strlen 处理二进制；Unicode、空数据和大载荷正确；不把任意 Data 地址强转为已初始化的原生结构 |
| **5. 错误与异常** | 将 C 状态码和必要的 errno 信息及时转换；C++ 异常留在 C++ 边界处理；反向 Swift 失败通过显式结果协议返回 | 错误目录、Swift Error/Result、C++ try/catch、无异常构建的状态协议 | 原研究：Swift 不能捕获越界传播的 C++ 异常；旧 mad 包装 Darwin 调用错误；MadDemo 部分 C++ 入口捕获异常并返回内部错误。[官方限制](https://www.swift.org/documentation/cxx-interop/status/)、 | noexcept 不被当成自动错误转换；失败不伪装成有效空值；错误读取不被后续调用覆盖；不使用异常处理掩盖内存破坏 |

## 二 所有权与内存

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **6. 创建 释放与分配器配对** | 每类资源明确创建者、借用者、接收所有权者、释放入口和允许的执行位置；复用后端已有引用管理 | RAII、专用 deleter、受限资源包装、所有权声明 | 原研究：原生引擎分别提供 new、stop、free，释放接口承担自身资源回收。 | 不混用 free/delete/专用释放器；成功、失败、取消路径回收配对；多个语言包装不各自销毁同一资源 |
| **7. 临时指针与异步保留** | 同步借用限制在有效作用域；异步前由原生侧拷贝、转移所有权或建立明确稳定的存储 | withUnsafeBytes/withCString、原生缓冲 owner、操作保活 | 原研究：Swift 文档明确临时指针不能逃逸其有效作用域；旧 mad 在 withUnsafeBytes 内同步完成写入。[临时指针](https://docs.swift.org/latest/documentation/diagnostics/temporary-pointers/)、 | 仅保留 Data 对象不被当成临时地址长期有效的证明；取消等待后不提前释放仍被 native 使用的缓冲 |
| **8. C++ 视图 借用与可变别名** | 记录返回视图依赖的 owner、修改失效条件和访问权限；优先限制借用范围，必要时复制 | 生命周期与边界注解、API Notes、Span、安全互操作检查 | 原研究：官方安全互操作支持借用和边界表达，部分包装能力依赖工具链与功能开关。[安全互操作](https://www.swift.org/documentation/cxx-interop/safe-interop/) | string_view/span 不超过底层存储寿命；owner 活着但容器重分配仍可能失效；注解与实现一致；未覆盖的契约明确保留人工审查 |
| **9. 回调上下文寿命** | C 回调使用可调用入口与 context；登记、保活、注销及在途访问采用同一协议；支持的 C++ std::function 路线复用其闭包管理 | C trampoline、受控 Unmanaged、回调注册对象、在途计数 | 原研究：直接 C++ 互操作在受支持版本可用捕获闭包构造 std::function；这不等同于 C 函数指针可捕获上下文。[回调能力](https://www.swift.org/documentation/cxx-interop/#using-stdfunction) | 未注册成功也释放上下文；注销返回含义明确；仅在未来及在途回调均不再访问 context 后释放；重复回调不重复消费 retained 引用 |
| **10. 跨语言引用环** | 显式绘制强引用关系；区分回调保活和业务生命周期；通过弱引用或注销入口断开循环 | weak、回调 token、显式退订、资源计数、内存图 | 原研究：Swift 对象持有原生对象，原生闭包再捕获 Swift 对象时可能形成循环；MadDemo 页面对观察任务使用弱捕获和销毁时取消。 | 关闭后回调和页面可回收；页面弱引用不替代原生 context 保活；ARC 与 shared_ptr 不被当作循环回收器 |
| **11. 关闭 析构与在途使用** | 关闭先停止准入，再等待或终止在途操作，最后释放；区分请求关闭与关闭完成；析构执行位置符合原生要求 | 幂等 close、Open/Closing/Closed 状态机、使用租约、清理执行器 | 原研究：MadDemo 区分 stop 和等待 I/O 线程退出的 free。 | 关闭超时不授权强行销毁；最后引用释放不假定在主线程；异步渲染结束前资源仍有效；回调内关闭不等待自己 |

## 三 异步 多线程与生命周期

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **12. 阻塞性与执行位置** | 声明同步耗时、可能阻塞和回调执行位置；重工作进入容量受控的执行设施 | 工作队列、专用执行器、耗时预算、主线程检测 | 原研究：MadDemo 的 poll 可选择等待；原生 free 会等待线程退出，不能只依据 Swift 方法是否 async 判断阻塞。 | Task 不被当作自动后台线程；actor 上的长时间同步工作仍计入阻塞预算；禁止无限扩张任务承接过载 |
| **13. Swift 并发隔离与共享状态** | 将原生对象放在明确隔离域；共享时提供真实同步；结果到 UI 前进入 MainActor；同时约束原生线程访问 | Actor、MainActor、Sendable 检查、锁或原生串行化 | 原研究：Sendable 表示并发语义保证，unchecked 由开发者负责正确性。[Sendable](https://docs.swift.org/latest/documentation/swift/sendable/) | 不用 unchecked Sendable 或 preconcurrency 掩盖未解决的数据竞争；actor 隔离不自动阻止 C++ 自有线程访问同一内存 |
| **14. 原生线程亲和性** | 分开声明串行、固定线程、主线程和可并发；为真实固定线程需求提供专用设施 | 专用线程、线程断言、执行器适配、创建与销毁线程测试 | 原研究：架构约束示例：原生图形或第三方句柄要求特定线程调用；普通 actor 的串行保证不等于绑定一个 OS 线程 | 创建、调用、回调和销毁符合各自约束；原生线程同步回调 Swift 时不假设已进入正确 actor |
| **15. 回调转 async 与完成认领** | 先登记再触发；成功、失败、取消通过统一终态认领；允许同步首回调；终态清理与恢复等待者分别定义 | CheckedContinuation、Operation 状态机、锁或等价同步、可控交错测试 | 原研究：continuation 需要恰好恢复一次；MadDemo 相邻边界采用登记等待者后发送的顺序。[官方约束](https://developer.apple.com/documentation/swift/checkedcontinuation)、 | 覆盖注册失败、同步完成、重复完成和取消竞争；不只检查是否非空后无同步地 resume；永不回调时有约定的终止方案 |
| **16. 取消 超时与后台收尾** | 区分停止观察、停止等待、停止工作和撤销副作用；原生停止请求与终态确认分开；迟到资源仍有 owner | cancellation handler、原生停止接口、取消状态、后台清理责任 | 原研究：MadDemo 网络等待取消不代表远端工作停止；C 接口的 send 返回只表示投递阶段。 | 已取消后注册仍安全；等待结束不释放原生仍使用的地址；取消查询支持其实际调用线程；业务副作用不伪装成已撤销 |
| **17. 重入 锁顺序与死锁** | 默认不持业务锁调用宿主；必要例外显式审查；避免原生线程与 Swift 执行器相互同步等待 | 锁顺序、回调分类、异步应答、重入与关闭测试 | 原研究：原生同步回调 Swift，Swift 再同步关闭并等待原生线程，是需要主动构造的失败场景 | 回调内取消、退订和关闭不自等待；结果转换不在持锁区执行任意用户代码；不靠递归锁掩盖生命周期问题 |
| **18. 流 背压与页面作用域** | 明确容量、丢弃、顺序、恢复和多观察者分发；区分页面离开、对象销毁和资源关闭 | AsyncStream/AsyncSequence、有界队列、序号、Subscription、UIKit/SwiftUI 适配 | 原研究：MadDemo 页面的观察在 viewDidLoad 启动、deinit 取消，不等于每次离页取消；底层存储另有单次常驻订阅约束。 | 慢消费者和队列满有确定行为；有限缓冲不自动形成生产者背压；多页面不重复登记唯一底层订阅；离页和销毁分别测试 |

## 四 构建 性能与交付

| 问题 | 处置与约束 | 工具或手段 | 历史样本／机制来源 | 验收重点 |
|---|---|---|---|---|
| **19. 编译环境与模块配置** | 固定 Swift、Clang、SDK、目标、宏、C++ 标准及互操作模式；读取实际构建参数 | SwiftPM、CMake、Xcode、Clang AST、编译数据库、工具链锁 | 原研究：直接 C++ 互操作需要配置模块和编译模式，也可能影响下游 SwiftPM 消费配置。[构建说明](https://www.swift.org/documentation/cxx-interop/project-build-setup/) | 实际编译并调用每种配置的公开面；向下游传递 interop 要求；Clang module map 不等于 C++20 modules；反向构建另验 |
| **20. API ABI 与行为演进** | 分开管理 Swift API、C/C++ ABI、行为契约和源码/二进制分发；明确可替换边界 | API 基线、符号与布局探针、旧消费者、Swift 模块接口检查 | 原研究：Swift Crypto 明确其源码分发不承诺稳定 ABI；Swift 官方描述 C++ 互操作兼容版本机制。[源码分发实例](https://github.com/apple/swift-crypto)、[兼容说明](https://www.swift.org/documentation/cxx-interop/#source-stability-guarantees-for-mixed-language-codebases) | 签名不变但取消或线程语义改变也可识别；extern C 不等于完整 ABI 协议；swiftinterface 不替代 C++ 标准库和布局兼容 |
| **21. 多平台制品与首次消费** | 区分同源构建、公开 C ABI 与 ObjC++ SDK；声明、头/modulemap、库和依赖配套；独立消费者覆盖真机/模拟器与 Release | XCFramework、xcodebuild archive、SwiftPM binaryTarget、干净环境消费 | 原研究：MadDemo 专门处理首次构建缺制品及仅有 arm64 模拟器切片时的构建配置。[Apple 打包](https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle) | 不要从本机直接互操作推导跨 Xcode 二进制支持；C++ ABI、Swift 模块和消费端配置分别验证 |
| **22. 依赖 符号与代码寿命** | 明确标准库、传递依赖、静态/动态链接和单实例归属；可卸载场景先确认对象、线程和回调均结束 | nm、otool、符号白名单、依赖图、装载测试、构建身份 | 原研究：MadDemo 将内核合入唯一 MadKit，不让壳再次链接同一内核，并校验应用符号。 | 不因重复库产生两套全局状态；运行库与工具链相容；仍有函数指针或析构代码被使用时不卸载对应库 |
| **23. 隐式复制与端到端性能** | 测量字符串、容器、结果快照、跨界频率和积压；先优化批量与数据形状，再按契约引入零拷贝 | Instruments、原生计数、复制计数、真机基准、性能预算 | 原研究：官方指出部分 C++ 容器操作可能有额外深拷贝。[性能约束](https://www.swift.org/documentation/cxx-interop/status/) | 同时覆盖延迟、吞吐、主线程耗时、复制量和峰值内存；不以一个空函数微基准代表真实 SDK；保活成本纳入零拷贝评估 |
| **24. 真实验证与故障定位** | 分层验证正反调用、内存、失败路径、并发及制品；记录证据对应的构建和覆盖范围 | Swift Testing/XCTest、适用的 ASan/TSan/UBSan、模糊测试、dSYM UUID、资源计数、结构化日志 | 原研究：mad-boundary 的 Verifier 将检查与制品、输入及证据关联；MadDemo 使用真实应用链接后的符号检查。 | 仅编译、仅主机测试、模拟器和真机证据分开；未运行不写成通过；符号匹配实际二进制；未插桩原生库的覆盖局限明确 |

## 本路线补充

- bindgen/cbindgen 服务 Rust 边界，SKIE/NativeCoroutines 服务 Kotlin→Swift；它们不是这里的通用导入器。

## 使用方式

按选定 profile 和本次任务读取相关条目；验收采用[公共问题目录](../../../validation/04-problem-catalog.md)、[关键交错场景](../../../validation/08-critical-scenarios.md)与任务 acceptance。本研究不维护独立进度、支持矩阵或第二套模块架构。共同约束与资料优先级见[归档入口](../README.md)。
