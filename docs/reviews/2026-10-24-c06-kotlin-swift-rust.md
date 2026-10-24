# C06 Kotlin / Swift / Rust 四路线整包记录

任务：C06；规范基线 0.9，起点为 MadB `cee264bbe93544d9700282ab444a93cc7202af4c`、MadB-Lab `13f0159da92d34746b096c2c07427a897967fe48`。本记录先固定实施范围，验收结果待真实运行补齐；当前不授予路线认证。

## 最低完整配置

依据 DEC-11 的稀疏矩阵，四条路线各选一个 macOS Arm64 Release 配置，最低系统 macOS 14.0。当前宿主、SDK、工具和实际运行系统分别进入配置锁；最低部署参数不代表已在最低系统实跑。

| 路线 | 本轮消费者与交付 | 绑定责任 |
|---|---|---|
| KJVM-RUST-UNIFFI | 桌面 JVM JAR、JNA 与 Rust 动态库；独立 Kotlin 消费者 | UniFFI 0.32.2 负责 ABI、对象引用与协程接线 |
| KN-RUST-GOBLEY | Kotlin/Native macosArm64 可消费绑定与 Rust 库 | Gobley 0.3.7 + UniFFI 0.29.4；受控生成器补丁与 P03 回归 |
| SWIFT-RUST-UNIFFI | Swift 模块、公开头和 Rust 动态库；独立 Swift 6 消费者 | UniFFI 0.32.2 负责 ABI 与绑定 |
| KN-SWIFT-OBJC | Kotlin/Native 可分发 Release framework；独立 Swift 6 消费者 | Kotlin 2.4.20 Objective-C export + SKIE 0.10.15 |

Android AAR/R8、iOS、CPF/OHOS、SKIE + KMP-NativeCoroutines 组合和 `madb init` 均保持原工作包边界。本轮不能将桌面验收外推为这些配置通过。四配置仍完整覆盖声明的值、错误、对象、异步、取消、同步反向回调、观察和关闭，不用同步子集替代完整配置。

## 实施边界与契约

- `sdk/` 仅依赖语言运行库与对应绑定后端；共享 Rust 语义核心不依赖工具私有 crate，两个 UniFFI 版本保持独立构建根。
- `adapters/generators/` 读取真实作者声明或编译导出，检查类型与策略并调用后端；工具链、源与模板补丁身份进入锁。
- `adapters/toolchains/` 经既有 C04 冻结端口执行生成、构建和打包；候选继续使用公共 v1 格式，不另造认证协议。
- Lab 只消费公开候选，独立预期和真实 Kotlin/JVM、Kotlin/Native、Swift 消费代码不引用工具源码或开发缓存。
- 每个 SDK 实例拥有一个明确的串行工作执行器；后端 Kotlin 默认调度器由 Kotlin 运行时拥有，不把工作执行器数量误称为整个进程线程上限。实例准入项数和保留字节同时有界。
- 载荷上限 1024 个逻辑字节，事件队列最多 8 项 / 4096 字节，状态队列为 1 项并显式合并最新值。逻辑字节计算与实际堆内存测量分开；关闭和释放不能依赖业务队列还有空位。
- `requestCancel` 可阻止未开始工作、拒收迟到结果；不能中断正在运行的同步外国回调，也不撤销副作用。等待取消、工作取消与资源排水分别验证。
- 关闭先拒绝新工作，再等待源端租约、在途回调和本实例工作退出；同步回调可请求关闭/停止，但必须返回后由独立 owner 等待排水；禁止在同步回调中以 `runBlocking`、信号量等等待依赖自身返回的异步工作。Rust `WouldDeadlock` 只检测同一 worker 的直接重入，不能识别任意跨调度器等待环。Swift 受管 callback 的 TaskLocal 检查另有明确作用域。
- Swift/JVM 的安全 SDK 工厂先执行 UniFFI ABI/checksum 检查，Gobley 在受管模板初始化中检查；原始上游入口与安全入口的保证分别说明。MadB 的 profile bootstrap 在创建业务资源前拒绝不匹配。SKIE 的模块/接口检查与 profile bootstrap 单独验收，不冒用 UniFFI 机制。

## 验收清单

四路线分别验证类型/空值/整数/字节/变体、错误、对象、异步完成、取消与迟到结果、回调重入、容量、强关闭、身份和错库、干净 Release 消费。Gobley 另验证旧模板受控复现和修复后的单次完成、回调抛错、取消/迟到结果交错；SKIE 实际调用 Flow/suspend/enum/sealed API，并验证 custom cinterop 来源与泛型 lambda 的准确拒绝或显式适配。

必须保留 C05 回归，运行工具及 Lab 的完整基线、适用本机消费者和远程检查。生成器/运行时并发实现须独立审查。未执行、环境不可用、失败和通过分别记录，不以探针、编译成功或文档检查替代运行验收。

## 实施中明确的共同语义

真实接口要求补齐 `throws` 与 `backend-handle`，详细规则归[契约规范](../architecture/03-contract-and-ir.md#c06-的错误通道与后端句柄)。该改动不在核心引入语言后端；它解决错误结构变化无法进入符号摘要、第三方整数句柄可能被误标为安全 registry 的共同模型缺口。可选字段保持旧文档摘要，新能力必须显式协商；错误类型、后端资源、能力拒绝及旧摘要兼容均有针对性正反例。

生成器、原生资源实现、Swift 并发包装和取消模板正在进行独立审查。工作执行器、外国调用与 waker 的析构不能在内部状态锁中发生；相关交错须有可控回归，不能用普通成功调用代替。

## 审查落实的工程决定

- Rust 异步就绪阶段保留 Rust 所有权，只有领取结果时才降为 FFI 值；取消未领取结果和 panic/error 缓冲均有释放路径。反向异步结果采用单一接收所有权守卫，迟到结果不能遗漏释放。两版运行时分别打补丁与复现，不把一个版本的证明迁移给另一个版本。
- Kotlin/Gobley 完成通知在业务错误捕获范围之外，防止完成回调抛错触发第二次完成；原生 future 的 `finally` 覆盖派发前取消。Swift 取消与 free 通过受管模板同步，不允许 cancel 在句柄释放后访问它。
- 外部回调、waker、最后引用释放与析构均在内部锁外。Rust 的 stop/close 只交出有界清理责任，实例 worker 执行可能阻塞的外国引用释放；清理 ticket 与订阅共用准入额度，关闭等待源租约实际归还。保证 SDK 引用交接完成，不承诺 JVM GC 最终回收时刻。
- Kotlin `Job.isCompleted`/`join` 不等于完成处理器清理结束；使用独立清理 ACK。worker 终止在锁外、非自有 worker 的共享控制执行器执行一次，多名关闭等待者共享最终确认；取消等待不取消关闭本身。
- Swift facade 保持单一引擎 owner、单次 collector 准入和入口等待预算；只管理 Swift 回调投递与排水，不再增加第二个生产队列。DTO 在跨 actor 前复制为 Swift 值；不为所有 Kotlin 类添加 `Sendable`。
- 保留正常 UUID，不为可复现性剥离已有调试信息；SKIE 保留 DWARF，Rust 使用既定 Release 配置，本包不承诺交付 dSYM。SKIE framework、Rust provider 编译和私有 native smoke 使用显式、按输入身份分配的受管冷构建目录；独占文件锁、owner/marker/symlink 校验、取消收尾及冷清空保护目录复用。它不是缓存，产物仍回到 C04 工作区，由宿主独立复算并校验字节。私有 smoke 与公开 SDK 的发布范围分开。
- Cargo 的主 crate 身份包含工作区路径，单独追加 `-C metadata` 不能覆盖该影响；Rust provider 复用同一受管冷构建机制，身份包含源、profile、功能和工具闭包，正常与错 profile 分开构建。产物在独占锁内复制回请求工作区，不复用历史编译结果、不改写二进制。
- 编译器进程组的 leader 保持可等待直到最后一次组信号发出，避免 PGID 复用；Darwin 通过系统 `libproc`、Linux 通过 `/proc` 有界观察同组存活进程，只有确认停止后才删除运行标记。观察失败保留现场。该保证针对不另建会话逃逸的受管编译器；Darwin 系统 `libproc` 由已观测的 macOS build 约束，不宣称任意守护进程隔离。
- C06 显式申请每阶段 900 秒执行预算与 6000 毫秒终止宽限，后者允许编译器 owner 清理其独立进程组；CLI 既有默认值保持不变。强制中断留下的所有权标记不能自动删除并冒充清理完成。
- Python 的真实解释器、标准库与必要动态库进入工具身份；隔离导入禁止 site 配置和未锁缓存。Swift 多调用工具保留调用别名，同时校验真实镜像和链接目标。辅助解析器的源码闭包与当前补丁配方必须对应冻结作者输入，旧安装不能借内部自洽继续使用。
- 固定 Apple 链接器的 chained fixups 会在私有 SKIE smoke 的重复 `_objc_msgSend` GOT 槽间产生不稳定选择。该探针单独使用 `-no_fixup_chains`，保留 UUID/DWARF 和逐字节门禁；公开 framework 不变。五次冷链接及真实运行已验证稳定，整链另行验证。
- Python.org framework 的内部悬空头文件链接作为“精确链接目标 + 缺失状态”进入库存；空目录目标后来出现同样改变摘要。循环、权限错误和目录逃逸仍失败；实际解释器和本机依赖必须存在。生产者与 Lab 分别实现校验。
- 生成器返回前拒绝符号链接、特殊文件、路径逃逸和未声明类型的产物；临时 HOME、API 提取与日志留在清理后不会发布的 scratch。C04 只接收声明输出。

## 当前证据

正在实施。实际命令、固定提交、候选摘要、报告和 CI 结果在整包验收后补齐；C06 的 `verification` 保持 `missing`。
