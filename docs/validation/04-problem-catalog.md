# MadB 公共问题与验证目录

本目录是 [problems.json](../catalog/problems.json) 的阅读视图。48 项公共问题来自 236 条历史研究条目的归并及开源产品的分发要求；稳定编号关联审查映射、具体正例与关键交错；每条保留解决方式、不变量与不利条件。

完整的工程覆盖还需要具体 profile、目标、方向和平台特例。下列 96 个基础用例组是覆盖目录，组数不等于已实现或执行的测试数；选定配置的实际证据见[C05 记录](../reviews/2026-10-24-c05-rust-c-first-route.md)与[C06 记录](../reviews/2026-10-24-c06-kotlin-swift-rust.md)。选定配置的结果不能将全部基础组标记完成。

解决方式包括自动生成、运行时封装、诊断阻断和明确使用约束。涉及原生算法与业务事务的前提由 SDK 作者负责；对应工具检查和运行行为仍要有测试。


## CAP-001 运行时与调用路线

- 解决方式：按运行时、方向和目标选择后端能力。
- 不变量：只在已声明的能力单元生成。
- 不利条件：间接路径通过但直接路径未实现。
- 规范：[实现约束](../architecture/06-backend-profiles.md)；关联原始条目 8 条。
- 验证：`CAP-001/positive` 与 `CAP-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：指定运行时、目标、调用方向和交付形态，解析唯一匹配 profile；列出其实际生成与消费入口。

## CAP-002 接口权威源与读取精度

- 解决方式：编译器元数据或头文件为结构来源，策略只补充语义。
- 不变量：每条边界只有一个结构权威源。
- 不利条件：声明与库版本不匹配或只能启发式扫描。
- 规范：[实现约束](../architecture/03-contract-and-ir.md)；关联原始条目 6 条。
- 验证：`CAP-002/positive` 与 `CAP-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：编译元数据和补充策略合并为带精度的快照；每个导出字段能回到唯一源声明。

## CAP-003 语言特性支持

- 解决方式：按后端检查泛型、继承、闭包、枚举和可见性。
- 不变量：不支持的类型明确阻断。
- 不利条件：复杂泛型或带数据变体无法表达。
- 规范：[实现约束](../architecture/06-backend-profiles.md)；关联原始条目 11 条。
- 验证：`CAP-003/positive` 与 `CAP-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：对锁定后端支持的 record、enum、variant、callback 子集分别生成并实际调用。

## CAP-004 生成器与编译插件组合

- 解决方式：锁定版本、补丁与功能责任方。
- 不变量：同一接口每种语义只有一个桥接责任方。
- 不利条件：两个协程桥同时包装同一接口。
- 规范：[实现约束](../architecture/06-backend-profiles.md)；关联原始条目 6 条。
- 验证：`CAP-004/positive` 与 `CAP-004/adverse`；实现与证据状态为 planned / missing。
- 正例场景：一个声明只有一个协程桥责任方；生成器、模板补丁、runtime 与目标库身份一致。

## TYPE-001 数值范围与单位

- 解决方式：受检映射、固定宽度、必要时强类型单位。
- 不变量：数值不截断，单位不隐式混淆。
- 不利条件：超过 JS 安全整数或窄化范围。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 9 条。
- 验证：`TYPE-001/positive` 与 `TYPE-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：每种公开整数的最小值、最大值和单位往返；额外测试大于 2^53−1 的受支持表示。

## TYPE-002 空值与缺失

- 解决方式：区分 null、nil、undefined、缺字段和默认值。
- 不变量：空值不变成零或有效业务对象。
- 不利条件：缺字段错误地套用默认值。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 8 条。
- 验证：`TYPE-002/positive` 与 `TYPE-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：分别传送存在值、显式空值、缺字段与默认值，接收端按契约保留四种含义。

## TYPE-003 稳定身份

- 解决方式：区分业务 ID、对象身份和运行时句柄。
- 不变量：业务身份不依赖调试字符串或内存地址。
- 不利条件：用 toString 或枚举 ordinal 作为持久 ID。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 6 条。
- 验证：`TYPE-003/positive` 与 `TYPE-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：相同业务 ID 在重建对象后保持相等，不同资源实例和代次不能混用。

## TYPE-004 模型与值语义

- 解决方式：按需直接导出、Facade 或不可变快照。
- 不变量：独立快照的嵌套数据也满足独立性契约。
- 不利条件：struct 内仍持有可变外部对象。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 7 条。
- 验证：`TYPE-004/positive` 与 `TYPE-004/adverse`；实现与证据状态为 planned / missing。
- 正例场景：在一个明确 revision 取得快照，修改源对象及嵌套对象后快照仍符合声明的独立性。

## TYPE-005 字符串与二进制表示

- 解决方式：显式编码、长度和载荷格式。
- 不变量：字节和文本不会未经声明相互转换。
- 不利条件：嵌入 NUL、无效 UTF、空指针加零长度。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 9 条。
- 验证：`TYPE-005/positive` 与 `TYPE-005/adverse`；实现与证据状态为 planned / missing。
- 正例场景：多字节、NUL、补充平面字符和空缓冲往返；JNI 单独使用约定的 Modified UTF-8 或 UTF-16 路径。

## TYPE-006 输入与载荷校验

- 解决方式：入口验证结构、范围、容量和版本。
- 不变量：可检查的非法形状、长度和数值在边界被拒绝；未知裸指针的有效性仍需来源契约保证。
- 不利条件：畸形 JSON、超大长度或整数溢出。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 10 条。
- 验证：`TYPE-006/positive` 与 `TYPE-006/adverse`；实现与证据状态为 planned / missing。
- 正例场景：合法输入在声明的深度、容量和数值上限内被接收，输出与独立构造的期望一致。

## TYPE-007 ABI 布局与调用约定

- 解决方式：按目标编译上下文生成和验证布局。
- 不变量：签名、布局、对齐与库属于同一目标。
- 不利条件：宏、pack、位宽或调用约定不一致。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 8 条。
- 验证：`TYPE-007/positive` 与 `TYPE-007/adverse`；实现与证据状态为 planned / missing。
- 正例场景：目标编译器探针报告 sizeof、alignment、offsetof，与生成绑定及真实库调用交叉核对。

## TYPE-008 公开名称与可见性

- 解决方式：稳定命名和冲突检查。
- 不变量：生成名称在目标作用域唯一。
- 不利条件：保留字、重载、大小写或扩展方法冲突。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 2 条。
- 验证：`TYPE-008/positive` 与 `TYPE-008/adverse`；实现与证据状态为 planned / missing。
- 正例场景：同名、重载和命名空间按明确规则生成稳定公开名称，消费端逐项实际引用。

## ERR-001 错误分类与宿主失败

- 解决方式：区分缺失、业务失败、取消和内部故障。
- 不变量：失败不能伪装成成功或空结果。
- 不利条件：回调失败或 pending exception 被吞掉。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 9 条。
- 验证：`ERR-001/positive` 与 `ERR-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：不存在、业务拒绝、取消、环境关闭和内部故障各用独立实例验证可辨认的结果。

## ERR-002 panic 与异常封闭

- 解决方式：依后端约束在所属语言内处理异常。
- 不变量：无约定的异常不穿越边界。
- 不利条件：panic=abort、C++ 异常或析构异常。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 9 条。
- 验证：`ERR-002/positive` 与 `ERR-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：声明为可处理的原生失败在所属边界内转换；消费者收到约定错误且无需捕获越界异常。

## ERR-003 失败后的对象一致性

- 解决方式：声明可恢复、失效或致命故障。
- 不变量：返回错误后对象状态可判定。
- 不利条件：部分修改后再次调用不一致对象。
- 规范：[实现约束](../architecture/04-data-and-errors.md)；关联原始条目 1 条。
- 验证：`ERR-003/positive` 与 `ERR-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：可恢复错误后再次调用成功；失效错误后拒绝操作但仍执行声明的安全清理。

## LIFE-001 所有权与释放器

- 解决方式：每个资源记录分配方、持有方和释放函数。
- 不变量：释放器匹配且最终释放一次。
- 不利条件：混用 free、delete 与 Rust from_raw。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 11 条。
- 验证：`LIFE-001/positive` 与 `LIFE-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：创建、返回、关闭与配套释放路径计数匹配；正常结果仅交给一个 owner。

## LIFE-002 借用与异步保留

- 解决方式：借用限制在有效范围，异步改为保活或复制。
- 不变量：等待者消失不提前释放工作输入。
- 不利条件：原生在 Future 丢弃后继续读缓冲。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 8 条。
- 验证：`LIFE-002/positive` 与 `LIFE-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：同步借用在窗口内使用；允许的异步调用复制输入或持有明确 owner，调用方退出不提前回收。

## LIFE-003 别名与移动

- 解决方式：按实际可变性提供借用、Pin 或不透明对象。
- 不变量：别名与移动符合两端约束。
- 不利条件：共享 Rust 引用期间原生写入或移动 C++ 对象。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 7 条。
- 验证：`LIFE-003/positive` 与 `LIFE-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：移动后旧 owner 不再使用资源；受支持借用与可变访问遵守其别名/Pin 协议。

## LIFE-004 回调引用与引用环

- 解决方式：注册对象持有上下文，注销后排水再释放。
- 不变量：上下文存活覆盖所有获准回调。
- 不利条件：同步首回调、GC 引用环、注销后迟到回调。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 12 条。
- 验证：`LIFE-004/positive` 与 `LIFE-004/adverse`；实现与证据状态为 planned / missing。
- 正例场景：注册前建立 context，源端与队列票据排水后解除引用，跨语言引用环可被显式 close 打断。

## LIFE-005 关闭与句柄准入

- 解决方式：关闭原子地停止准入，排水后释放。
- 不变量：关闭检查与实际借用没有竞态窗口。
- 不利条件：关闭和调用并发、句柄复用或重复关闭。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 11 条。
- 验证：`LIFE-005/positive` 与 `LIFE-005/adverse`；实现与证据状态为 planned / missing。
- 正例场景：调用许可和关闭共享同步协议；新调用在 Closing 被拒绝，已获准调用结束后只释放一次。

## LIFE-006 父子资源依赖

- 解决方式：持有关系或借用寿命表达资源依赖。
- 不变量：子资源使用期间依赖资源仍有效。
- 不利条件：先释放模型再使用上下文。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 1 条。
- 验证：`LIFE-006/positive` 与 `LIFE-006/adverse`；实现与证据状态为 planned / missing。
- 正例场景：子资源存在时按契约保留父 owner；最终按子后父的合法顺序关闭。

## LIFE-007 环境退出与代码寿命

- 解决方式：环境退出执行拒绝、撤销、排水与释放。
- 不变量：回调和析构不进入已销毁环境或已卸载代码。
- 不利条件：Worker 退出、动态卸载或未完成异步清理。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 6 条。
- 验证：`LIFE-007/positive` 与 `LIFE-007/adverse`；实现与证据状态为 planned / missing。
- 正例场景：env/Worker 或 JVM 线程退出按所属机制处理引用；动态库保持至最后一次回调与析构结束。

## LIFE-008 结果交接

- 解决方式：结果必须由接收方或回收方唯一认领。
- 不变量：取消竞争不丢失有资源的结果。
- 不利条件：完成后恢复前取消，或重复通知携带同一资源。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 10 条。
- 验证：`LIFE-008/positive` 与 `LIFE-008/adverse`；实现与证据状态为 planned / missing。
- 正例场景：正常结果由消费者接管；取消后未交付结果由清理方接管，最终责任计数恰好一次。

## ASYNC-001 取消与超时效果

- 解决方式：分别表达停止等待、观察、工作与业务撤销。
- 不变量：取消效果不强于后端实际保证。
- 不利条件：超时后工作继续或副作用已经发生。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 11 条。
- 验证：`ASYNC-001/positive` 与 `ASYNC-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：分别请求取消等待、停止观察和停止工作，验证公开效果与独立确认事件。

## ASYNC-002 完成与注册竞争

- 解决方式：先登记后启动，统一终态认领。
- 不变量：每个操作只有一次终态交付。
- 不利条件：同步完成、启动失败、重复完成和迟到完成。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 10 条。
- 验证：`ASYNC-002/positive` 与 `ASYNC-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：注册过程中同步完成、正常异步完成及完成通知后异常均保持一次终态，结果交接另行检查。

## ASYNC-003 通知与受理确认

- 解决方式：声明单向通知、同步返回、异步应答及阶段。
- 不变量：入队不能冒充受理或持久化完成。
- 不利条件：先返回成功而宿主实际执行失败。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 4 条。
- 验证：`ASYNC-003/positive` 与 `ASYNC-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：方法返回受理确认后继续观察真实完成事件，两个阶段及操作身份可以区分。

## ASYNC-004 流与背压

- 解决方式：区分状态和事件，声明容量、顺序与溢出。
- 不变量：数据丢弃和恢复策略可观测。
- 不利条件：慢消费者、乱序、多生产者、溢出和重订阅。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 10 条。
- 验证：`ASYNC-004/positive` 与 `ASYNC-004/adverse`；实现与证据状态为 planned / missing。
- 正例场景：慢消费者下验证状态合并、事件顺序域、有界容量和明确溢出策略，计数与期望一致。

## ASYNC-005 任务与执行器归属

- 解决方式：明确运行时、作用域和清理责任。
- 不变量：操作生命周期有归属且后台收尾可追踪。
- 不利条件：关闭运行时后 Future 或任务仍被唤醒。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 8 条。
- 验证：`ASYNC-005/positive` 与 `ASYNC-005/adverse`；实现与证据状态为 planned / missing。
- 正例场景：正常结束与等待者退出均有明确收尾 owner；其作用域关闭后无无主后台任务。

## THR-001 运行环境线程约束

- 解决方式：通过环境专属入口调度并隔离引用。
- 不变量：环境句柄不跨线程或跨环境滥用。
- 不利条件：JNIEnv、napi_env 或 UI 引用被错误复用。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 5 条。
- 验证：`THR-001/positive` 与 `THR-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：宿主引用只在合法 env/线程访问，外部生产者使用该 profile 支持的投递机制。

## THR-002 线程亲和性与阻塞

- 解决方式：分别声明固定线程、串行执行与阻塞等级。
- 不变量：串行调度不冒充固定 OS 线程。
- 不利条件：把同步 I/O 或 native join 放到 UI 线程。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 15 条。
- 验证：`THR-002/positive` 与 `THR-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：固定线程调用及析构回到同一线程；串行执行器只验证互斥，耗时操作不阻塞 UI。

## THR-003 共享状态同步

- 解决方式：统一两端的访问协议和线程能力声明。
- 不变量：Arc、GC、const 不作为线程安全证明。
- 不利条件：未经证明生成 Send、Sync 或 Sendable。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 8 条。
- 验证：`THR-003/positive` 与 `THR-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：所有语言入口同时访问共享状态，经同一同步协议得到一致版本，无未经证明的 Send/Sync/Sendable。

## THR-004 重入与锁顺序

- 解决方式：持锁区避免调用外部代码，采用两阶段处理。
- 不变量：回调与关闭没有自等待和环形等待。
- 不利条件：回调内重入关闭或反向获取同一把锁。
- 规范：[实现约束](../architecture/05-lifecycle-and-concurrency.md)；关联原始条目 10 条。
- 验证：`THR-004/positive` 与 `THR-004/adverse`；实现与证据状态为 planned / missing。
- 正例场景：宿主回调重入查询或发起停止正常返回；外部 owner 等待确认，无持锁调用或自等待。

## UI-001 页面与订阅生命周期

- 解决方式：页面只持有观察令牌，共享资源由业务 owner 管理。
- 不变量：退订不误关闭共享资源。
- 不利条件：反复进入页面重复订阅或后台继续无界更新。
- 规范：[实现约束](../architecture/08-runtime-and-idiomatic-api.md)；关联原始条目 8 条。
- 验证：`UI-001/positive` 与 `UI-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：两页面共享引擎，各自拥有观察；关闭一个页面后另一页面继续更新，最终 owner 关闭。

## UI-002 输入与权威状态

- 解决方式：版本和来源标记保护草稿及输入法组合态。
- 不变量：旧快照不覆盖新的本地输入。
- 不利条件：回声、重排、中文组合态和受理前清空。
- 规范：[实现约束](../architecture/08-runtime-and-idiomatic-api.md)；关联原始条目 2 条。
- 验证：`UI-002/positive` 与 `UI-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：编辑期间交错旧快照和新输入，保留组合态与新草稿，仅在契约的受理阶段清空。

## UI-003 响应式模型与列表身份

- 解决方式：按框架生成受限适配与稳定键。
- 不变量：状态更新和列表身份遵循 UI 框架约束。
- 不利条件：装饰器不触发更新或列表复用错行。
- 规范：[实现约束](../architecture/08-runtime-and-idiomatic-api.md)；关联原始条目 1 条。
- 验证：`UI-003/positive` 与 `UI-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：正确构造可观察实例并在 UI 隔离域更新；列表增删重排保持稳定业务键。

## PERF-001 跨界性能与内存

- 解决方式：测量复制、频率、队列、保留量后优化。
- 不变量：优化保留生命周期与数据正确性。
- 不利条件：大缓冲保活、细粒度调用或全量列表刷新。
- 规范：[实现约束](../architecture/10-diagnostics-and-performance.md)；关联原始条目 12 条。
- 验证：`PERF-001/positive` 与 `PERF-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：固定 Release、设备、预热和负载，记录原始延迟样本、复制字节、队列及内存峰值。

## BUILD-001 工具链与目标上下文

- 解决方式：锁定编译器、SDK、宏、特性、ABI 和原生依赖。
- 不变量：构建身份覆盖会影响输出的参数。
- 不利条件：交叉编译意外读取宿主头文件。
- 规范：[实现约束](../architecture/09-build-packaging-release.md)；关联原始条目 11 条。
- 验证：`BUILD-001/positive` 与 `BUILD-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：从锁重建目标，记录 sysroot、宏、编译器/SDK、标准库及依赖摘要，消费端完成链接调用。

## BUILD-002 装载与重复实例

- 解决方式：显式打包依赖和装载拓扑。
- 不变量：同一逻辑实例不会被重复静态链接无意复制。
- 不利条件：ClassLoader、多个 Framework 或动态注册顺序冲突。
- 规范：[实现约束](../architecture/09-build-packaging-release.md)；关联原始条目 11 条。
- 验证：`BUILD-002/positive` 与 `BUILD-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：同一应用的两条受支持入口读写同一个测试内核实例，链接清单与创建/释放计数一致。

KNOI 平台特例：依赖库装载顺序、多个模块参与桥注册及 dlopen 后模块不可调用，需在真实目标验证；上游资料及职责见[后端约束](../architecture/06-backend-profiles.md)。

## BUILD-003 符号可达性

- 解决方式：验证导出、裁剪、R8 和反向入口。
- 不变量：Release 中回调入口仍可达。
- 不利条件：Debug 正常而 strip 或 R8 后失效。
- 规范：[实现约束](../architecture/09-build-packaging-release.md)；关联原始条目 2 条。
- 验证：`BUILD-003/positive` 与 `BUILD-003/adverse`；实现与证据状态为 planned / missing。
- 正例场景：Release/R8/链接裁剪后真实正向与反向入口仍可达；JNI 注册名与实际 class 描述符对应。

KNOI 平台特例：Release 混淆保留、sharedLib 注册完整性及 initBridge 查找失败；模板生成的声明和保留规则必须经真实 HAP 消费。

## BUILD-004 制品与全新消费

- 解决方式：绑定、库、运行时和包作为一组交付。
- 不变量：干净消费工程只靠声明依赖能构建。
- 不利条件：依赖作者绝对路径、缓存或本地仓库。
- 规范：[实现约束](../architecture/09-build-packaging-release.md)；关联原始条目 10 条。
- 验证：`BUILD-004/positive` 与 `BUILD-004/adverse`；实现与证据状态为 planned / missing。
- 正例场景：无作者仓库的新目录安装冻结包，绑定、native 库及运行依赖完整，公开 API 实际可调用。

## BUILD-005 平台装载限制

- 解决方式：按目标校验页大小、最低系统和二进制属性。
- 不变量：声明支持的设备可以装载制品。
- 不利条件：Android 大页设备或旧系统装载失败。
- 规范：[实现约束](../architecture/09-build-packaging-release.md)；关联原始条目 1 条。
- 验证：`BUILD-005/positive` 与 `BUILD-005/adverse`；实现与证据状态为 planned / missing。
- 正例场景：在声明的最低 OS、目标 ABI 和页大小条件下安装并真实加载全部传递原生库。

## COMP-001 API 与 ABI 兼容

- 解决方式：比较导出形状、布局、符号与旧消费者。
- 不变量：兼容判断与分发模型一致。
- 不利条件：新增重载造成歧义或布局变化。
- 规范：[实现约束](../architecture/09-build-packaging-release.md)；关联原始条目 9 条。
- 验证：`COMP-001/positive` 与 `COMP-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：旧消费者对声明兼容的新 SDK 执行编译、链接和行为冒烟，核对公开符号与布局差异。

## COMP-002 行为兼容

- 解决方式：把线程、取消、受理阶段和关闭策略纳入版本。
- 不变量：行为变更不会悄悄穿过接口检查。
- 不利条件：签名不变但同步返回变为仅入队。
- 规范：[实现约束](../architecture/09-build-packaging-release.md)；关联原始条目 8 条。
- 验证：`COMP-002/positive` 与 `COMP-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：升级前后比较回调线程、取消效果、受理阶段与关闭后条件；兼容声明由独立预期支持。

## GEN-001 生成确定性与追溯

- 解决方式：可再生输出、来源映射、目录所有权与漂移检查。
- 不变量：相同输入生成相同结果且不覆盖手写代码。
- 不利条件：失败留下半套代码或非受管文件被删除。
- 规范：[实现约束](../architecture/07-code-generation.md)；关联原始条目 10 条。
- 验证：`GEN-001/positive` 与 `GEN-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：不同临时路径重生成得到相同文本；漂移检查不改有效输出，失败后手写与旧生成内容保持完整。

## VERIFY-001 真实双向消费

- 解决方式：按目标从消费端验证公开制品。
- 不变量：替代环境的结果不升级为真实目标通过。
- 不利条件：Node/JVM 模拟代替 ArkTS/Kotlin Native 运行。
- 规范：[实现约束](../validation/01-testing-strategy.md)；关联原始条目 10 条。
- 验证：`VERIFY-001/positive` 与 `VERIFY-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：从冻结候选在真实消费语言执行正向调用和已声明反向接口，按目标保存实际结果。

## VERIFY-002 能力与证据可信度

- 解决方式：区分声明、实现、运行证据和证据精度。
- 不变量：未运行、过期、不支持与通过互不混同。
- 不利条件：启发式零命中或 skipped 被当作合格。
- 规范：[实现约束](../validation/01-testing-strategy.md)；关联原始条目 6 条。
- 验证：`VERIFY-002/positive` 与 `VERIFY-002/adverse`；实现与证据状态为 planned / missing。
- 正例场景：报告分别记录实现、执行结果、证据层级和新鲜度；只有对应候选的有效结果可进入认证。

## OBS-001 诊断与符号化

- 解决方式：稳定诊断码、关联 ID、符号摘要和脱敏。
- 不变量：故障能追溯到边界、阶段、目标和制品。
- 不利条件：符号文件不匹配或日志泄露业务载荷。
- 规范：[实现约束](../architecture/10-diagnostics-and-performance.md)；关联原始条目 9 条。
- 验证：`OBS-001/positive` 与 `OBS-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：一次跨端故障关联符号、操作、目标和正确 build ID/dSYM，日志脱敏且不含业务载荷。

## DIST-001 开放分发与供应链

- 解决方式：依赖许可、制品校验、来源和可复现安装。
- 不变量：发布包与验证包是同一内容。
- 不利条件：依赖未声明、下载被替换或生成物许可不清。
- 规范：[实现约束](../delivery/02-open-source-and-contributing.md)；关联原始条目 0 条。
- 验证：`DIST-001/positive` 与 `DIST-001/adverse`；实现与证据状态为 planned / missing。
- 正例场景：从公开候选通道下载相同字节，核对来源与许可清单并运行安装冒烟，发布不重新构建。

## 覆盖维护

[用例组登记](../catalog/case-groups.json)保存 96 个基础组；每组在适用路线展开为真实用例。原始条目的特殊条件通过 source_ids 关联到具体子用例，不能因映射到了同一公共问题而省略。

DIST-001 来源于开源产品交付要求，未引用历史研究条目。新增问题先登记编号与验收，再实施解决方案；调整问题含义需同时检查来源映射、案例、规范和版本承诺。

## 基线 0.7 的平台审查场景

本节对应 [source-coverage.json](../catalog/source-coverage.json) 的 `audit_sources` / `audit_mappings`、各问题的 `review_cases` 及基础组的 `platform_scenarios`。9 份原始来源、236 条历史映射、48 个问题和 96 个基础组分别管理；12 个审查来源、10 个审查条目不代表运行结果。具体案例实例仍需按 profile/目标/方向展开。

| 审查条目 | 分类与处置 | 既有问题 | 探针 / 工作包 |
|---|---|---|---|
| AUD-001 foreign future 双完成 | 旧模板风险；受控复现、修复并锁生成器补丁；成功完成后抛错不触发第二次错误完成 | ASYNC-002、LIFE-008、ERR-001、CAP-004 | P03 / C06 |
| AUD-002 Android 模块与 native 包装 | 平台配方；AGP 新 KMP、独立 app、jniLibs 或 native library，显式 host/device tests | BUILD-001、BUILD-004、VERIFY-001 | P11 / I04、C06 |
| AUD-003 官方/CPF Maven 发布 | 平台配方；首版独立 GAV/根 metadata，错坐标负例与独立消费 | BUILD-004、COMP-001、DIST-001 | P05、P08 / I05、C10 |
| AUD-004 SKIE 二进制分发 | 平台配方；显式可分发 framework 和跨机器消费 | BUILD-004、COMP-001 | P04、P08 / C10、L04 |
| AUD-005 SKIE 不可调用 API | 后端类型规则；custom cinterop 来源与 lambda 泛型参数处置，实际调用承诺接口 | CAP-003、TYPE-008、VERIFY-001 | P04、P12 / C06、L04 |
| AUD-006 Swift C++ 构建/方向/包装 | 平台配方；direct-source 与 SDK 包装分别验收，SwiftPM 不冒称反向直接导入 | CAP-001、BUILD-001、BUILD-004、COMP-001 | P07、P11 / C08、I06 |
| AUD-007 UniFFI ArkTS beta | 新实验候选；独立版本和实际设备运行，不继承主线证据 | CAP-001、CAP-004、VERIFY-001、VERIFY-002 | P13 / X07 |
| AUD-008 JNI 生成来源 | 平台配方；真实 class 描述符、注册、ClassLoader/R8；Java 与 Kotlin 声明入口分开 | CAP-002、TYPE-007、BUILD-003、VERIFY-001 | P11 / C08 |
| AUD-009 CPF 正式 SDK | 既有设计待验证；正式 SDK、sysroot 与设备分开，Beta 参考不升级为 Release 认证 | BUILD-001、BUILD-005、VERIFY-001 | P05 / C07、I05 |
| AUD-010 napi crate/CLI | 配置精度澄清；napi crate 家族与 Rust ohrs CLI 各锁来源、版本和组合 | CAP-004、BUILD-001、DIST-001 | P06 / C07 |

关键原始依据：[UniFFI 上游修复](https://github.com/mozilla/uniffi-rs/pull/2733)、[Gobley v0.3.7 模板](https://github.com/gobley/gobley/blob/v0.3.7/crates/gobley-uniffi-bindgen/src/templates/ffi/Async.kt)、[Android KMP 插件](https://developer.android.com/kotlin/multiplatform/plugin)、[KMP publication](https://kotlinlang.org/docs/multiplatform/multiplatform-publish-lib-setup.html)。

Swift 相关依据：[SKIE 分发](https://skie.touchlab.co/configuration/swift-compiler)、[custom cinterop 限制](https://skie.touchlab.co/known-issues/cinterop)、[lambda 泛型限制](https://skie.touchlab.co/known-issues/lambda-types-in-type-argument)、[Swift C++ 构建与方向](https://www.swift.org/documentation/cxx-interop/project-build-setup/)。ArkTS 实验和工具配置依据：[UniFFI ArkTS](https://github.com/ohos-rs/uniffi-bindgen-arkts)、[ohrs CLI manifest](https://github.com/ohos-rs/ohos-rs/blob/ohos/cli/cargo-ohrs/Cargo.toml)；JNI 注册依据：[JNI specification](https://docs.oracle.com/en/java/javase/25/docs/specs/jni/functions.html#registernatives)。版本及平台来源统一回到[技术栈选型](../architecture/16-technology-stack-and-versions.md)。

“上游已知风险”不等于 MadB 已复现线上事故；“平台配方”不等于已交付代码；“既有设计待验证”也不应重复统计为新架构缺陷。全部状态仍为 planned / missing。

## 基线 0.9 的具体场景

48 个正例组已分别描述输入、动作或独立观察，不再全部使用“合法输入及正常生命周期”。复杂时序按[CRIT-01 至 CRIT-12](08-critical-scenarios.md)展开，`case-groups.json` 的 `scenario_refs` 保存对应关系。主线、扩展和不支持能力分别实例化；这些仍是待实施规范，没有增加运行通过数。
