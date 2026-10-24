# 首链 ABI v1 的可执行设计

本目录是 C02 的审查基线：固定两条直接路径需要的声明、状态值、缓冲、句柄和停止协议。`expected.h`、`expected.rs` 都是手写的**预期形状**，不冒充 cbindgen/bindgen 输出。`consumer.c` 只做签名与布局编译检查；C03 固定上游工具和两个目标的真实生成形状，C05 交付实现及独立消费者。当前没有可安装或运行的 SDK。

## 资产与责任

| 资产 | 作用 | 后续验证 |
|---|---|---|
| [C 声明](expected.h) | Rust 提供方的预期导出面及独立 C 提供方入口 | C03 cbindgen 对照；C05 实际符号、链接及 C 消费 |
| [Rust 声明](expected.rs) | `repr(C)`、数值状态和 `unsafe extern "C"` 导入形状 | C03 bindgen 对照；C05 独立 Rust 消费 C |
| [C 消费片段](consumer.c) | 字段偏移、宽度、状态常量及函数签名 | 本机编译；不等于运行或双目标探针通过 |
| [格式与语义样本](../../../tests/fixtures/contracts/README.md) | 五个端口的独立样本、领域身份、策略来源与反例 | C02 跨实现检查；C04 端口执行、C05 真实读取器 |

C ABI 是“状态返回 + 输出参数”；IR 的 `result: engine/u64/unit` 表示逻辑结果。C05 降低到 `MadbStatus` 与 `out_*`，失败状态不混入逻辑返回类型。格式样本覆盖五个基本接口；本目录还固定观察接口与 Rust 独立消费 C 的入口。观察接口的真实读取与有效策略由 C03/C05 补齐，不据此声称样本已覆盖所有回调运行路径。

## 数值、缓冲和错误

状态权威为 `expected.h`：0 成功，1 InvalidArgument，2 InvalidHandle，3 Closing，4 Closed，5 Busy，6 Timeout，7 WouldDeadlock，8 InternalFailure，9 ResourceExhausted，10 DomainError，11 Unsupported。状态载体固定 `int32_t`，不把 C enum 的实现宽度暴露为 ABI。

- 输入是只读字节缓冲，长度单位为 byte，最大 65536；`data == NULL` 仅允许长度为 0。非零长度要求指向调用期间有效的可读内存；无法验证任意地址是否有效。
- checksum 是所有字节按无符号整数求和；空缓冲结果为 0。该上限下不会溢出 `uint64_t`。超过上限或长度无法转换为目标 `size_t`，返回 InvalidArgument。
- 所有 `out_*` 都要求非空、有效、对齐且互不重叠。进入操作时先将每个**有效**输出位置清零，再做句柄/业务校验。任意非法裸指针属于调用者违约，不能保证返回错误代替未定义行为。
- `MadbError { domain: uint32_t, code: int32_t }` 没有堆内存。成功和生命周期失败时两项为 0；仅 DomainError 携带非零稳定 domain 与领域 code。不能让异常或 panic 穿越 C ABI；可捕获失败映射 InternalFailure，abort 配置不承诺返回。
- 句柄是所属 SDK 实例的登记令牌，0 无效，单调分配、不复用，耗尽返回 ResourceExhausted。包装另记来源实例；令牌不是全局指针或安全认证凭证。

## 关闭、观察及 context

`request_close` 原子关闭新操作准入，非阻塞，重复请求返回同一次关闭的结果。`await_closed(timeout_ms)` 成功必须意味着该句柄负责的工作、消费者回调和资源清理已完成；0 表示只检查状态，尚未完成返回 Timeout。超时不强制释放，不恢复为 Open。未请求关闭时返回 Busy。

`release_handle` 只在 Closed 后移除登记；Open/Closing 返回 Busy；移除后包括重复 release 在内均为 InvalidHandle。普通业务调用在有效 Closing/Closed 登记上分别返回 Closing/Closed。`request_close/await_closed/release_handle` 接受 engine 或 subscription；checksum/observe 必须是 engine，类型不符返回 InvalidHandle。

观察的最低协议为：

- 注册可在返回前同步调用一次；后续回调在该订阅的串行源端执行序列上调用，允许回调内非阻塞请求关闭。不得持有登记表锁调用用户代码。
- callback 非空，context 可为空；context 不转移，调用者从注册开始保活到停止排水完成。注册失败在返回前排水、输出 subscription=0，此后不得再访问 context。
- 同步首回调尚未取得返回的 subscription，可使用已持有的 engine 请求关闭；若此时关闭 engine，注册返回 Closing，完成失败回滚和排水后输出 subscription=0。
- 成功返回 subscription 只表示已注册。重复值可交付，sequence 在该订阅内递增；本版本无消费者队列和历史回放，不承诺丢失事件恢复。回调必须有界返回，不得同步等待自己的退出。
- subscription 的 `request_close` 停止接纳新回调；已获取的回调许可可完成，`await_closed` 等待排水。回调内对本订阅或等待该回调的 engine 调用 `await_closed` 返回 WouldDeadlock。
- engine 关闭也关闭其订阅并排水；成功后 context 可清理。subscription 的登记令牌仍由其调用者 release，避免引擎替消费者消费令牌；源端 owner 负责保持所需记录直到排水完成。

上述是待 C05 验证的协议，不是 C02 的并发正确性证据。首链要求的错句柄、重复关闭、关闭超时、迟到回调、注册失败和同步反调见 [CRIT-03/08/11](../../validation/08-critical-scenarios.md)。

## 来源诊断与拒绝基线

以 `tests/fixtures/contracts/contract-ir.json` 的 checksum 为例：reader 标记源位置 `native/fixture.rs:20:1`，项目策略独立保留配置位置。将该调用改为 async 而沿用 call-frame 借用，纯规则产生 `SEM008`，subject 为完整公开 ID，location 指向声明；两个相同显式 ownership 也产生 `POL004`，保留两个来源位置。测试从结构样本构造反例，断言诊断代码及位置，不从失败实现反推预期。

缺释放器、错 release 签名、未知强制语义、缺 C record 布局、来源为 heuristic 都在生成前阻断。schema 正例只能证明可解码；冻结输入、策略来源、类型能力、真实字节摘要和执行证据分别检查。

本目录不包含 ABI 兼容承诺；第一份对外版本前仍须完成 C03/C05 并审查实际生成差异。改变本设计要同步声明、样本、测试与对应决定，不只修改一个语言的镜像。
