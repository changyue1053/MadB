# C09 最终 receipt 窗口：已批准的测试切点

项目所有者于本轮明确答复“采用推荐方案”。本记录保存已批准的最小差异和验收边界，不表示新候选已通过，当前任务状态只在 catalog 维护。范围仅为 C09 对正常 Runtime.close 的补证，C07 已验收的候选、报告及结论均不修改。

## 已确认的限制

冻结 producer 为 `18256fcc4b26cf3cd1e2e79c39b86c2512e012d9`。C/C++/Rust 伴随 facade 的 `Session.tick` 在 `physicalRuntimeAck(frame)` 满足后紧接着同步调用 `native.consumeRuntimeReceipt`；C 冻结文件 `generated/facade/Boundary.ets:505–507`，Kotlin 对应文件 `:501–503`。两句之间无公开回调或 yield。已有 C `testAfterSettlement` 与 Kotlin `testSettleProbe` 只覆盖 child waiter 的 resolver/rejector 已返回阶段，并非 Runtime receipt。

因此，当前受检公开入口足够控制 ACK 前的真实 worker/exit hold；不足以确定性停留在 ACK 已成立而 receipt 尚未消费的切点。`setTimeout` 抢跑、阻塞 env 的忙轮询、覆写全局定时器、访问私有 Session/native 对象均不作为这项证据。

## 推荐：一次同步测试回调，而非新增原生资源 gate

只在 test companion 的公开 facade 追加：

```typescript
export function testBeforeRuntimeReceipt(runtime: Runtime, probe: () => void): void;
```

- 接受同一 SDK 的实际 Runtime wrapper，必须在普通 close 前且 Session open 时注册。函数只保存一次回调，不产生 native owner、线程、订阅或普通 waiter。
- 仅当原 tick 已获得真实 `physicalRuntimeAck(frame) == true`、`waiters.size == 0` 且尚未调用 `consumeRuntimeReceipt` 时触发一次；先清空回调槽，再同步调用。回调返回后继续原来的 receipt 消费和 close 完成过程。
- 停留仅覆盖同步回调调用栈；没有异步 Promise 控制、额外 native holder 或新的恢复命令。恢复规则就是回调正常返回。消费者在回调里捕获自己的断言异常、保存失败信息并返回，await 原 close 后再抛出，确保失败检查也保留真实关闭路径。
- 回调不允许返回 Promise/thenable；声明输入按现有工具链的同步 void 回调规则准确拒绝。不把 TS/ArkTS `void` 单独当作运行时 Promise 排除证明。
- 非函数、重复注册、错误 wrapper、已开始 close 的注册必须准确拒绝；推荐沿用现有 `InvalidArgument` / `Closed` 分类，最终以既有 test-helper admission 实现一致性审查确定。回调意外抛错不得吞掉，沿用现有 tick failure/close rejection 路径；此异常场景不得写成成功关闭，独立失败报告保留清理缺失。
- 成功触发后回调引用立即退休；正常 close 后未触发的槽也必须清空。测试 hook 不对 native physical ACK 作假，不抢先消费 receipt，不授予新 Runtime admission。

无需新增数字状态帧。消费者的回调现场使用现有公开观测：

| 路线 | 回调现场原始证据 | 精确拒绝与继续 |
| --- | --- | --- |
| C/C++/Rust | `testInspect(testExportKey(runtime))`：178 字段，Phase=3、ReadMode=2、ReceiptConsumed=0，原生零 owner/配对释放；`testObserveFacade(runtime)`：ParentCloseControl=1、Closing=1、Closed=0、TickInFlight=1 | `openRuntime()` 必须为 typed `RuntimeAlreadyOpen`；回调正常返回后同一 close 完成；再次 inspect ReceiptConsumed=1；新 Runtime 实际业务与关闭通过 |
| Kotlin | `testObserve(runtime)`：110 字段真实 physical ACK、ReceiptConsumed=0；`testLastAdmissionAudit()` 的当前 slot/epoch 同时保留；`testSettleInspect(runtime)` 证明 TickInFlight=1，close Promise 尚未完成 | `openRuntime()` 必须为 typed `AlreadyInitialized`（冻结 Kotlin 公开错误名）；返回后 full110 + settlement9 关闭义务满足、ReceiptConsumed=1，再做同 env 新 Runtime 正例 |

现场日志应独立标记 `receipt-window-native`、`receipt-window-js`、`receipt-window-reopen-rejected`、`receipt-window-probe-returned`、`receipt-consumed-native`、`new-runtime-result`。原始完整帧和 nonce/PID/候选/HAP/源码摘要都归档，不能仅输出布尔总结。expected 来自 Lab 显式不变量，不导入生产 predicate。

## 四路线、普通 SDK 与候选影响

四路线都已有同一责任点；C/C++/Rust 共用受管 companion facade 补丁，Kotlin 有独立 facade 测试变体。该改动仅修改 test variant 源声明/生成链，不进入普通 SDK 的公开符号、普通 facade、native binary 或普通 ABI。仍需通过普通/伴随源推导与符号检查实际证明无泄漏，不能仅凭设计声称。

每个受影响 test companion 必须重新生成 HAR、固定测试声明与源码/补丁摘要，形成新的 candidate/build/profile 身份并重新冻结。不能在 `c07-*-test` 已冻结目录补写文件，不能沿用旧 companion digest 或通过状态。普通候选只有在字节不变且既有配对规则重新核验通过时才可作为关联普通输入；否则相应普通候选也重建。

最低复验：四路线 hook 正例及参数/重复/late 注册负例；现场 ACK/receipt 分隔、准入拒绝、恢复后原 close 和新 Runtime；同步回调异常保留失败且不伪造 ACK；新 companion 源/契约/身份/符号与普通隔离检查；受影响正常/失败关闭、settlement 重入、实际 env cleanup 场景的回归；原生生命周期/并发独立复核。新 C09 场景只能声明本候选本目标的结果，不能继承 C07 全矩阵或主线认证。

## 批准范围与实施边界

采用上述测试专用公开回调及其异常/注册边界，授权新 companion 重新生成、冻结和适用复验；普通 Runtime 契约与旧冻结候选不变。为了满足既有 ordinary/test 同修订配对和推导规则，需要在独立完整输入快照中创建本地冻结提交，并生成四份普通对照和四份测试候选。普通对照的公开 API、布局和实现隔离逐项核验，字节/身份差异单列；不修改旧普通候选，不从本地新修订推导 C07 重认证或公开发布。受管补丁、profile 与 helper 契约由明确单一写入者维护，原生生命周期/并发由独立审查者复核。

## 实施时发现的异常边界

第一份本地冻结输入 `4edc8c7863659b825ba8168092494966ee9aa34f` 的普通 Error/Promise/thenable 目标案例通过后，独立审查另用其实际生成 facade 发现：Error 的 `code` getter 若再次抛错，回调异常误入仅供 native 的错误转换，可能使 close 保持 pending 并重启 timer。旧候选、目标报告及 450 文件 Lab 源快照保留；这些结果不构成该遗漏异常的通过。

修复仍限测试伴随回调边界：调用前清除 callback slot；真实 Error 保持原对象，不读取其 code/message/then 等属性；非 Error 的 null/undefined/标量/普通对象及 Error 判定陷阱使用既有 CallbackFailure 分类，避免 undefined 与既有 failed 哨兵冲突。直接进入既有 fail/close rejection，停止本 tick 的 receipt 消费；不能把用户异常再传给 nativeError。普通 Runtime、native owner、帧布局及既有错误分类不变。此处细化已批准的回调失败拒绝，未承诺保留任意 JavaScript 抛出值的 identity。

修复需重新冻结完整集成输入、生成同修订四对候选、独立复核和实际复验。主机穷举异常值与判定陷阱，目标补 Error getter 及非 Error 代表场景；原 scope/验收要求不因这次失败删减。
