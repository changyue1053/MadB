# ADR-0013 ArkTS Runtime 所有权与完成通知

状态：采用，C07 完整 SDK 的实施契约；不是 profile 认证或发布声明。日期：2026-10-25。基线：0.9，关联 DEC-07/08/10、[ADR-0011](0011-arkts-native-protocol-owner.md)。不改变 C05/C06 行为、工具核心或公共 wire 格式。

## 决策依据

Engine 的业务排水、JS Promise 结算、整个 env runtime 的清理是不同阶段。只提供 Engine.close 会留下通知入口的最终 owner；把所有责任放入同一个关闭条件，又可能让关闭等待自己的 ACK。已核对的 Node-API 实现也不能证明任意未结算 napi_deferred 都可在 env 退出时自动释放，错误返回后盲目重试结算并不安全。

[完成通知探针](../reviews/c07-gateway-observation.json)已在正式 SDK26 编译、API23 Release 模拟器执行九组独立测试。其结论只覆盖原生完成记录、JS Promise、通知与 env 清理机制；真实 provider、业务 worker 和完整候选继续单独集成验证。

## 公开 owner 与语义

普通入口显式建立父级 owner：`openRuntime() → Runtime.connect() → Engine`。应用持有并最终 `await runtime.close()`；页面拥有其 Subscription。getter、模型构造和属性读取不隐式启动 worker 或订阅。

| 对象 | 责任及关闭范围 |
|---|---|
| Runtime | 持有所属 env 的通知、引用、清理机制和唯一父级关闭观察；关闭全部子代，最后确认原生环境责任归零 |
| Engine | 持有一个实际业务 worker、操作与观察；业务排水后仍由 Runtime 递送已受理的关闭 ACK |
| Operation | 终态认领、独立工作租约与结果接管；`result()` 是一次原生认领，第二个已受理请求返回 AlreadyConsumed |
| Subscription | 停止准入、源和投递排水；用户监听器退出或合法丢弃完成后才返还票据 |

第一完整切片限制每 env 一个 Runtime 初始化、一个活动 Engine generation；Kotlin 仅支持主 env。后续 Engine generation 只有在旧代所有业务、等待者、JS settling 和通知责任均清偿后才能准入。关闭的公开对象只保存不可变结果，不访问复用槽。是否允许最终 Runtime 关闭后重新初始化必须另有能力声明和测试，不能默认支持。

保留普通 `roundtrip/startOperation/publishState/publishEvent/observeState/observeEvents/requestClose/close`，操作的 `requestCancel/result/close`，观察的 `requestStop/close`。同步方法返回普通值或抛出类型化错误；Promise 方法的普通准入错误通过拒绝表达。

C07 采用 first-terminal-wins：Success、DomainFailure、Cancelled、InternalFailure 中第一个合法终态保持不变；工作退出和结果释放另外计费。取消不是撤销已经发生的业务副作用，丢弃 JS Promise 也不是取消原生工作。首个 result 注册在提交前失败，可撤销认领并重试；提交后必须完成唯一转移或合法清理，不能用第二个请求替换。

state 初始为空，不回放注册前值；保留一个最新 pending 和一个 entered。event 按每个订阅的受理顺序 FIFO；一次发布对选中的接收者全部准入或全部拒绝。state/event 是两个独立方法。监听器的同步返回不表示异步业务 ACK。

## 完成通知与预算

Promise 由 ArkTS 创建。原生持有有界完成责任，单个 coalesced wake 仅提示有变化，不拥有业务 Snapshot，也不把结果从账本移走。facade 在原生提交前完成 JS 等待者登记并预留一个自身持有的轮询计时器；同步 ready 只标记变化，不递归消费。每轮最多检查32项结果和一次审计，丢失唤醒由有界退避轮询补偿；轮询不是业务超时。

| 责任 | 本切片明确的上限与归属 |
|---|---|
| 操作、订阅 | 每 Engine 8 个操作、8 个 state/event 合计订阅 |
| event 票据 | 每 Engine 8 个接收者投递，含 source、排队、entered，不能仅计队列长度 |
| retained 字节 | 每 Engine 4096 个 canonical UTF-8 字节，包含独立副本、stored result、state 替换 staging；不是总堆内存上限 |
| 结果与关闭等待者 | 每 Engine 合计32项，覆盖 result 和 Operation/Subscription/Engine.close；ready、entered、cleanup 均继续占用 |
| Runtime.close | 在 Runtime 准入时预留一个父级控制，重复调用复用；不额外建立一套32项业务表 |
| roundtrip 同步 scratch | 每 owner env 另保留最多1024 UTF-8字节的一个临时 provider 模型；不占操作/等待者，实际配对释放后归还 scratch |

Snapshot 单条文本最多1024 UTF-8字节，值为严格 Int32；保留 NUL/Unicode，拒绝孤立 surrogate、非有限数、小数和越界。roundtrip 的 native scratch 在有用户代码可能性的字段读取结束后取得独占许可，clone/read/输出自有属性期间不调用任意用户代码；重入占用准确拒绝，不等待。调用级 UTF-16/UTF-8 转换临时存储、字符串容量与对象头另外列入实际内存观察，不能用4096概括全部占用。

原生 gateway 的32项索引只引用同一 Core 的 ResultClaim、ResultDelivery 或 CloseWaiter，不增加第二套配额或复制业务载荷。共享协议的 clone 用类型化结果区分非法输入、分配失败和内部失败；所有非空返回值，包括失败携带的值，都必须先取得配对释放责任，再报告失败。

一次 `tryTake` 在消费 guard 内复制结果；成功和受控转换失败均在真实析构后归还原生票据。guard 前失败保留原生残留，不能声称已经消费。JS settling 槽持续到 resolve/reject/finally 完成，避免重入绕过容量。输出定义自有数据属性，避免赋值触发继承 setter。state/event 则在 `Delivery.invoke` 内同步调用真实监听器，直到监听器返回或抛错、清理完成才退票据。

## 关闭、重入与最终确认

`close()` 先执行幂等关闭请求，再注册观察。32项满额可使这次观察返回 ResourceExhausted，但关闭请求不撤销；之后可以重试观察。已经关闭时直接返回已知结果，不重新持有原生等待者。

Runtime.close 通过不占业务 waiter 的 request-close 停止子代，等待真实 worker join、Core 业务排水、Core waiters=0、JS settling=0 以及 entered/排队通知归零，再清理环境引用与 TSFN。Core.closed 本身不足以证明这些条件，因为其 ACK 等待者可能仍在递送。facade 对自己的 JS owner 负责；原生释放同时独立校验其原生 owner，不信任调用方传入的零计数。

NativeWorker 的提交入口包含 Core 准入与唤醒。外部预留清理任务在非 env、非业务线程执行真正的 join，线程及 TLS 析构完成后才归还 Executor；不得把工作函数返回或 Kotlin Worker Future 当成物理 join。自身判断使用真实线程作用域，不能把可能复用的历史线程 ID 当作仍存活的业务线程。

输入 getter 可以重入并关闭同一个 Engine。入口必须保活原 generation，在字段读取后再次按统一准入协议检查状态，再提交；关闭条件包含尚在执行的入口和 scratch。不得在第一次 Open 检查后跨任意用户代码继续使用旧状态。

实际 env 退出使用预先登记的 async cleanup owner：停止 worker、在合法 env 期间清理引用、丢弃不可递送的 Core token、在清理执行器完成 join，最后在所属线程移除 handle。null-env 回调仅清理其合法原生 owner，不运行 JS/NAPI。平台 finalizer、载荷析构、context 析构分别记录真实完成，失败保留残留，不能清空指针伪造归零。

普通 Runtime.close 通过静态 bootstrap 通道取得最终物理 ACK，停止未来轮询后才 resolve。正常关闭的 registry slot 要等所属 facade 消费最终 receipt 后才可复用；真实 env 退出由系统清理 owner 收尾。epoch 不复用，容量、清理失败和过期身份要明确，不采用无界墓碑或固定生命周期次数冒充并发上限。

## 实施与验证

私有句柄使用严格、无溢出的十进制字符串并绑定 route/env/generation/资源种类，不无检查转为 ArkTS number。生成 facade 带独立预期的 ABI/profile/制品身份，bootstrap 在 dispatcher ref、业务 worker 和 TSFN 之前验证。Kotlin 的主 env 检查须在可能越过 C 边界的初始化之前以可恢复错误拒绝。

C/C++/Rust 按 ADR-0011 共用协议及 env 实现，各自仍真实分配模型、执行算法和释放；Rust 保留实际 napi-ohos 前端。Kotlin 保持唯一 managed owner、实际 KSP/ArkProxy 与独立 worker 责任，不能用原生路线的结果替代其证明。

按顺序完成首条 C 完整接合、C++/Rust 接合、Kotlin managed 接合、真实声明生成与错误定位、两阶段 Build、HAR 候选和独立 Lab。必需检查包括终态竞争、满项/满字节、源已取得但尚未入队、entered 回调停止、waiter 满载关闭、错误后的恢复、同 env/旧代拒绝、实际系统清理以及错库/ABI/profile。所有规定路线和候选证据齐全后才能标记 C07 完成；机制探针和宿主单元测试不替代它们。
