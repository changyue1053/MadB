# L02 普通 SDK hooks 与 checked constructor 具体决定提案（修订 2）

本修订取代 SHA256 `38bdae402f8443932db91a00c128cede7bad8d2122756d5a72b2a25b5aa08930` 的首稿：首稿 record 数组无法由现行 ContractIR 的 scalar buffer 表达，现改为单条固定 event pull；旧稿仅保留审查历史，不供批准或实施。

本提案待项目所有者裁定；不登记 catalog 状态，不代表实现、编译、运行或独立验收已完成。产品审查输入为 `/private/tmp/madb-chief-round6/madb`，原始 HEAD `bc32a61e0dd0fa4f35e57421c309619304b97381`；Lab 为同批 `madb_lab`，原始 HEAD `341088a122cb36f468367f49b0f3d45d01d86a24`。本提案仅写临时审查文件，未改产品或 Lab 源码。

## 要裁定的完整增量

采用两个显式 C→Rust 诊断 profile（普通及 test-support 伴随），增加两个生产 pull API、两种固定 C 结构和下述有限事件语义；为所有本次以后新生成的 C→Rust 候选增加独立 `include/madb_rust_client.h` checked constructor。原 7 个 native 函数及其状态、资源、回调协议不变；默认 profile 仍只有 7 个 native 导出，原 test-support profile 为 7+7；新诊断 profile 为 9 个，新诊断 test-support profile 为 9+7。该增量不修改旧冻结候选，也不覆盖 C06/C07 已验收结论。

诊断 profile 对消费者可选，但这两个 profile 的实现和独立验证属于 L02 承诺交付，不可因此省略。默认 profile 与诊断 profile 分别生成、冻结，身份不可互换；本轮只认当前 C05 已有 C→Rust 首链及其实际目标，不承诺新增语言、平台或异步业务 producer。正式 explain 机器格式由另一份提案裁定，本提案不重复 CLI 归属决定。

兼容性严格分三层：旧七函数的消费者源码仍可重新编译；新增生产诊断库保留旧七二进制符号/签名，但带身份校验的旧 header/旧 binary 不得越过新的 interface/profile 身份；旧候选的摘要、导出门禁和验收范围完全不变。新 surface 要单独列出两函数、两记录和相关有限类型，不能把 lowering 的未知 symbol/record/constant 拒绝改成任意放行，也不能给旧七 profile 放宽 Lab 的 exact-export 门禁。

## 当前证据与缺口

`sdk/rust/engine/src/lib.rs:48` 的 boundary 只在 test-support 调用 `test_support::diagnostic()`；后者（`test_support.rs:85`）是隔离 panic 注入及计数。Lab `consumers/c/main.c:233` 的故障 3 用例能证明该测试故障不改变数据、资源和回调结果，不能证明生产采集开关、事件、预算、满队列或慢 exporter。普通 C→Rust 消费者现仍由 harness 提供 expected_hex 手工 bootstrap（`main.c:38`）。以上源码和旧报告只用于确定复用边界，不是本轮新增验收证据。

## 生产 C ABI

新增 Cargo feature 名称为 `diagnostics`，默认关闭；该 feature 的普通与伴随 profile 均属于正式 C→Rust 适配器配置。只允许在已有受检 backend config 增加严格布尔 `diagnostics`（缺省 false；非布尔拒绝，true 只允许 provider=rust，C-provider 不得选用），不改顶层 profile/1 wire。具体 profile 名称 `c-to-rust-diagnostics`、`c-to-rust-diagnostics-test`；公开 builder 用显式 `--diagnostics` 附加这两份候选，原默认三候选行为不变。工具链、源码、profile 与导出字节重新入锁和摘要，不能拿原候选摘要冒充新候选。

```c
typedef struct MadbDiagnosticEvent {
    uint64_t record_id;
    uint64_t symbol_id;
    uint64_t phase;
    uint64_t resource_id;
    uint64_t related_resource_id;
    uint64_t duration_ns;
    int64_t status;
    uint64_t status_present;
} MadbDiagnosticEvent;

typedef struct MadbDiagnosticStats {
    uint64_t completed_calls;
    uint64_t failed_calls;
    uint64_t completed_callbacks;
    uint64_t dropped_events;
    uint64_t capture_enabled;
} MadbDiagnosticStats;

MadbStatus madb_fixture_diagnostics_configure(
    uint32_t enabled, MadbError *out_error);
MadbStatus madb_fixture_diagnostics_drain(
    MadbDiagnosticEvent *out_event, uint64_t *out_present,
    MadbDiagnosticStats *out_stats,
    MadbError *out_error);
```

在本轮既有 64 位目标上，Event 为 size 64 / align 8、字段偏移 0/8/16/24/32/40/48/56；Stats 为 size 40 / align 8、字段偏移 0/8/16/24/32。原有 `MadbError` size 8 / align 4 不变。必须由正式 compiler 读取真实 repr(C) surface，再严格检查有限结构、字段、偏移和函数签名；不靠这份手写文本替代 ABI 元数据。其他 ABI 布局需要其目标单独证明，本提案不宣布全平台支持。

两个新符号的语义 public_id 分别是 `madb.engine.diagnostics_configure.v1`、`madb.engine.diagnostics_drain.v1`。新增结构和签名属于新的 native interface digest；profile digest 同时绑定 feature、runtime 与工具来源。没有新增 MadbStatus 值或 MadbError domain/code：生产诊断 API 的 `out_error` 当前始终归零，拒绝由已有 status 表示。

### 开关、排空与输出

1. 每个实际装载的 SDK image 独立拥有固定 64 槽事件 ring、开关和计数，初始 capture 为 off，ring 为空。运行期不扩容，不为单条记录堆分配，不创建线程、timer 或 exporter。
2. configure 只接受 0 或 1；其他值返回 `MADB_INVALID_ARGUMENT`。成功线性化于 ring 控制锁内；0 停止之后的采集但保留已入队记录，1 开始采集；不重置 counters / record_id / queue。重复配置同值成功且无重置。
3. drain 每次最多取一条固定 Event；out_event、out_present、out_stats、out_error 都须非空、正确对齐、有完整对象大小的可写存储，且彼此不重叠。任意地址可访问性和不重叠性仍是既有 ABI 的调用者义务，SDK 不声称能探测非法地址。没有 capacity、数组或隐含循环。消费者需批量读取时自己设定有限调用次数/时间预算，不能假定并发 producer 存在时循环一定排空。
4. 先独立清零可机械检查的四个输出（包括整个 Event）；任何 null 或对齐拒绝都返回 `MADB_INVALID_ARGUMENT`，不消费 ring，其他合法输出仍全零。合法非空 ring 调用复制并删除 FIFO 一条记录、present=1，并返回 stats；空 ring 返回 OK、present=0、event 全零和实际 stats。present 只能为0/1；record_id=0保留给零输出，不能以status=0推断存在事件。
5. configure/drain 使用一次 try_lock；锁竞争返回 `MADB_BUSY`，不改变开关或消费事件，已合法的四个输出全部归零。锁中毒返回 `MADB_INTERNAL_FAILURE`，不尝试修复业务状态。生产采集也只 try_lock，任何争用、中毒或满载都放弃该记录并计一次 dropped_events；禁止等待锁或重试循环。控制调用自身不产生诊断事件或 completed_calls，避免递归。
6. configure/drain 是同一 loaded image 的共享诊断控制，任何线程均可调用；多个调用方自行协调，不能把它解释为某个 engine 的业务 owner 或关闭能力。drain 在锁内复制固定值，不调用任何外部代码；返回后调用者拥有值副本。调用者 exporter 的阻塞、失败、异常或丢弃仅影响其处理，SDK 不进入该 sink、不持有其 context。没有“安装 sink 回调”API，因此 native callback 栈不可能同步重入 exporter；SDK 也不代替调用者递归拉取。

### 事件、默认计数与唯一责任

record_id 为成功入队记录的 image 内单调 uint64，首条 1，不因 drain/disable 重置；分配最后 UINT64_MAX 后停止再分配并对后续合格记录计 drop，不重用编号。它不是跨进程、跨 image 或候选的全局身份。ring 满时 drop-new，保留已有 FIFO。计数为 AtomicU64 的模 2^64 累计，不是资源/生命周期保护计数；不参与任何关闭、租约或配额判断。

诊断 profile 的 completed_calls（包括 bootstrap、参数拒绝和失败）、failed_calls（原返回 status != OK）、completed_callbacks 在 capture off 时仍计数；off 不读取 clock、不入队。dropped_events 仅计合格但未能入队的记录，不把 off 期间调用算 drop。Stats 各 counter 单独原子读取，可受并发推进影响，不声称构成同一时刻的资源快照；capture_enabled 表示本次 drain 持锁时的配置。不得用这些近似诊断计数代替 test-support 的资源守恒或运行期正确性计数。

边界在进入时采样 capture 开关；仅进入时为 on 且完成时取得 ring 锁后重核仍为 on 的操作才入队。configure disable 和最终重核/入队使用同一锁线性化，保证 disable 成功返回后不会再入队该次关闭前已完成的记录（随后显式 re-enable 的新周期除外）。disable 可省略已开始的记录，不算 drop；不会改变其业务状态或排水语义。duration_ns 使用单调 elapsed（超过 uint64 时饱和），不含 wall time；计时仅诊断开启时启用。此预算承诺的是固定内存、有限步骤、无排队等待；不虚构纳秒延迟上界，实际 overhead 必须采样记录。

| symbol_id | 原符号/阶段 | resource_id | related_resource_id |
|---|---|---|---|
| 1 | bootstrap 返回 | 0 | 0 |
| 2 | create 返回 | 成功产生的 engine，否则 0 | 0 |
| 3 | checksum 返回 | 调用参数中的 opaque engine token | 0 |
| 4 | request_close 返回 | 调用参数中的 opaque token | 0 |
| 5 | await_closed 返回 | 调用参数中的 opaque token | 0 |
| 6 | release_handle 返回 | 调用参数中的 opaque token | 0 |
| 7 | observe 返回 | 调用参数中的 engine token | 成功产生的 subscription，否则 0 |
| 8 | state callback 已返回 | subscription token | parent engine token |

原七入口每次调用的 boundary-returned 只有一条尝试，phase=1，status_present=1，status 精确保存原 MadbStatus。callback-returned 独立为 phase=2，status_present=0、status=0 占位；void callback 的返回不冒充业务成功，未返回的回调不会制造完成事件。callback 捕获数值 token，无 context 指针；在实际回调返回后记录，不持有 registry 锁、不延长或缩短 SourceLease。observe 的同步初始回调和 observe 返回是两个不同语义阶段，不重复统计同一责任。checked helper 只复用 bootstrap/create，不发第三条便利层事件。

事件不含 payload、字符串、异常内容、路径、环境、context 地址或内存地址；resource 字段只是既有公开 opaque token 的数值（失败参数也可能无效），不具备反解地址或复活资源的能力。已释放的 token 可保留在值记录中，不持有资源。调用者将 drain 来源与同一 loaded image 的 bootstrap interface/profile 身份以及外部冻结 candidate 绑定；最终 candidate digest 不嵌入库或事件，避免自引用。不能把无来源 ring 合并后声称可追溯到候选。

所有采集均隔离于业务 catch/return：诊断内部失败只放弃诊断，不替换原返回码、有效输出、callback 次数、资源 ownership/admission 或关闭顺序。必须独立复核锁顺序、回调外部代码位置与实际候选；不得将 try_lock 设计文本当并发正确性证明。

## 现行 ContractIR 的精确投影与 reader 隔离

不增加 common IR 类型、buffer 元素能力、共享 schema 或新能力 token。配置 feature 选择两份明确 surface：默认严格只允许原7函数与 MadbError；诊断严格允许原7+2函数以及 MadbError/DiagnosticEvent/DiagnosticStats 三个 C record。未知额外符号/record/typedef/constant 继续拒绝。i64 复用现有 integer64 signed；Event/Stats 两个 C record 从实际 surface 得到字段及各自 compiler layout witness。native status/out_error 沿用已有 first-route ABI 到 logical 的固定降级约定，原 status 不伪造成业务 result。

新增纯值 logical 类型 `diagnostic_pull` 为 layout=logical 的 record，字段顺序 `present:u64, event:diagnostic_event, stats:diagnostic_stats`；它不是 C ABI struct，没有伪造 compiler layout witness。native 四 out 参数分别绑定 logical 三字段与原错误槽；present=0时logical event全零，不引入 nullable/variant/数组。现有 schema 的 record layout 已支持 logical，TYPE007 对纯值嵌套record可通过；仍须实际走原核心验证，不允许据此绕过 finalizer。

| role | logical parameters | logical result | 本增量 |
|---|---|---|---|
| bootstrap | requested_version:u32 | identity_bytes（原u8 buffer,64） | 不变 |
| create | 无 | handle | 不变 |
| checksum | engine:handle, data:bytes（原u8 buffer,65536） | u64 | 不变 |
| request_close | handle:handle | unit | 不变 |
| await_closed | handle:handle, timeout_ms:u32 | unit | 不变 |
| release_handle | handle:handle | unit | 不变 |
| observe | engine:handle, callback:state_callback | handle | 不变 |
| diagnostics_configure | enabled:u32 | unit | 新增：原C enabled参数直接投影 |
| diagnostics_drain | 无 | diagnostic_pull | 新增：固定out_event/out_present/out_stats/out_error关系 |

两个新符号的 call policy 为 threading=caller、blocking=non-blocking、delivery=work-completed、cancellation=none、cancellation_confirmation=not-supported；drain return ownership=copy，无资源transfer/borrow/releaser，configure 无 return ownership。enabled 是纯值；本次不修改原七符号 policies。ring 控制属于 SDK image 诊断模块，原业务 registry/SourceLease 的唯一 owner 不移交诊断模块；新调用也不增加业务 callback 或 host port。

cbindgen 0.29.4 是解析源文件，不会因 rustc feature 关闭就自动隐藏 cfg 声明。实施必须新增受锁的 diagnostics 专用 config：普通 cbindgen.toml 明确 exclude 两新函数和两诊断record；diagnostics config 保留原 test-support exclude，声明 `[defines] "feature = diagnostics" = "MADB_DIAGNOSTICS"` 并在生成头内 after_includes 固定 `#define MADB_DIAGNOSTICS 1`。实际 rustc 同时只在新 profile 加 `--cfg feature="diagnostics"`。专用 config 本身纳入声明输入/depfile/工具链和候选身份，不能由消费者自己定义开关伪装库能力。两 config 的真实生成头分别经原 Clang reader 和 exact-surface 规则验证；header声明集、实际nm导出集、selected profile三者不一致必须拒绝。原7/profile 与新9/profile不可仅通过放宽集合合并。

## 生成 checked constructor

新增公开、头文件内 static inline helper，仅包含生成的 `madb_rust.h`；其文件不回灌 cbindgen/native reader，不成为 native interface 事实源。它在默认普通、默认 test-support 及两种新 diagnostics 候选均生成；原 C-provider Rust Client 的已实现行为保持原范围。

```c
typedef uint32_t MadbCheckedEngineCreateKind;
enum {
    MADB_CHECKED_ENGINE_OK = 0,
    MADB_CHECKED_ENGINE_BOOTSTRAP_FAILED = 1,
    MADB_CHECKED_ENGINE_INTERFACE_MISMATCH = 2,
    MADB_CHECKED_ENGINE_PROFILE_MISMATCH = 3,
    MADB_CHECKED_ENGINE_CREATE_FAILED = 4
};
typedef struct MadbCheckedEngineCreateResult {
    MadbCheckedEngineCreateKind kind;
    MadbStatus native_status;
    MadbError native_error;
    MadbHandle handle;
} MadbCheckedEngineCreateResult;
static inline MadbCheckedEngineCreateResult madb_engine_create_checked(void);
```

Result 在既有 64 位 C 目标的预期 size 24 / align 8，偏移 kind0/status4/error8/handle16；此类型由 consumer 自己编译，不新增 native ABI 导出。生成器和声明检查仍须验证布局/可编译性，固定宽度 kind 避免 enum 宽度承诺。

调用顺序必须完全固定：

1. 结果初始化 handle=0、native_error={0,0}；调用原 `madb_fixture_bootstrap(1, identity, 64)` 一次。expected interface32/profile32 字节来自本次已受检 `identity.json`，不让普通调用者传摘要，也不从环境、仓库或最终 candidate digest 推断。
2. bootstrap 非 OK：kind=BOOTSTRAP_FAILED，native_status 原样保存，其余保持零，不调用 create。
3. bootstrap OK：先比较前32 interface 字节；不同为 INTERFACE_MISMATCH。仅接口相同时比较后32 profile 字节；不同为 PROFILE_MISMATCH。两种 mismatch 的 native_status 都是实际 bootstrap 的 OK=0，**调用者必须以 kind 判断整体成功**；不伪造 native 错误码，native_error/handle 仍为零，不调用 create，不需要释放资源。
4. 两个身份均匹配后调用原 create 一次。失败为 CREATE_FAILED，native_status/native_error 保留 create 真实结果、handle=0（原 create 失败清零义务仍须验证）；成功为 OK，返回唯一新 owned engine。helper 不缓存或创建第二个 engine，也不偷偷关闭已交付 engine。
5. 成功后的 request_close → await_closed → release_handle，以及 subscription/context lifetime，完全沿用原公开资源义务。原 bootstrap/create 仍可供高级消费，helper 不取消旧 ABI。规范一致的 create 必须在成功时返回非零 handle；非规范库不在兼容承诺内，不能编造额外状态掩盖它。

新 header 自身纳入 OutputSet/FileRef、package closure 和 candidate 字节校验，expected 身份来自同一生成上下文；错误 header 配实际 library 必须在 create 前拒绝。新头文件不会改变既有默认7的 native interface digest，但新增生成资产改变 package/output/candidate 字节身份；不得复用旧候选摘要。

## 逐符号责任表的边界与剩余项

本提案继续使用既有 profile/1，除了明确申请的 rust-c-backend/1 `diagnostics` 布尔开关，不在 backend_config 偷塞跨组件共享职责 wire，也不新增公开责任 JSON。architecture/08:87 要求的逐符号、语义阶段、实例作用域和唯一 owner，不能由现有 runtime-protocol.json 的五项组件说明替代。主协调另行负责从真实受检 symbol/政策/组件身份产生内部有限责任值，由核心验证缺项/重复，并将检验依据绑定实际候选。

这份内部证据不自动证明一个尚不存在的公开责任资产已交付。如果完整 L02 审查确认必须提供可消费公开附表，则须以已批准 composition/1 + profile/2 的具体首链配方和责任格式接入为单独、明确的实现范围；本提案不默认接入已经存在、不新造第三种共享 wire，也不因此宣布 L02 完成。

## 临时设计检查（非 SDK 验收）

`l02-native-design-check.json` 记录了 C11 布局及六种 mock constructor 分支检查；`l02-native-feature-header-check.json` 记录了固定 cbindgen 0.29.4 对临时声明副本的两份 config 生成及未修改 native reader 检查。后者实际得到默认7函数/1record、诊断9函数/3records，单条drain四参数与Event64/Stats40符合提案；工具、输入、生成头摘要均已记录。临时副本的新增函数体是占位，不属于产品实现；两个设计检查均未运行生产 ring、真实 SDK consumer、finalizer 新 logical 投影或独立 Lab 候选，不能据此登记通过。

## 批准后有限写入与检查条件

产品允许实施范围建议：`sdk/rust/engine/{Cargo.toml,cbindgen.toml,cbindgen-diagnostics.toml,src/lib.rs,src/runtime.rs,src/tests.rs,src/diagnostics.rs}`；`adapters/generators/rust_c/{driver.py,lowering.py}`（可新增同目录 helper emitter 以维持单文件职责）；`adapters/toolchains/rust_c/{project.py,worker.py,pipeline.py}`；`scripts/build_first_route.py`；实际相关局部测试。`runtime.rs` 仅插入 callback-returned 数值记录，不改 ownership、状态机、close/lease 或业务 producer。若必须扩展本表外公共 wire/字段，先返回主协调审查，不能借实施扩大批准。

Lab 建议新增 `consumers/c/l02_diagnostics.c`、独立 L02 runner 与相应有限 cases；保持原 C05 cases/历史 reports 结论。新 runner 可复用公开 candidate verify/工具锁/安装器，但不得读产品私有源或绕过 candidate provenance。主协调唯一写 catalog、共享文档/契约和最终集成；独立审查者复核 native 并发。

验收向量见同目录 `l02-native-sdk-vectors.json`。最低退出包括：正式 compiler 对新两个结构/两个入口和旧七入口逐项验证；默认7、原 test14、新9、新 test16 的真实 nm；checked helper 五种结果及 interface 优先、no-create-on-mismatch；新普通安装候选的数据/资源/同步回调对照；off/on/full/exporter 不工作时的 close/context/lease 后条件；队列 FIFO/有限容量/单条 FIFO pull、拒绝与四输出清零、非重入/并发/独立实例；真实低开销采样。注入只用于测试伴随或 Rust 私有单测，不能把它冒充普通 profile 已提供的事件 API。

新生产/test profile 与更新的默认 profile 必须从实际干净源 revision 生成、固定工具、冻结 candidate 后，由干净独立 Lab revision 消费并重新绑定 case 源摘要；dirty/provisional、编译通过或源码断言不写成独立验收。完整 L02 还需 explain/逐符号责任表/来源诊断等其他退出条件共同满足。本提案不授权发布、push、GitHub Actions 或扩大平台矩阵。
