# L02 有限 Rust 值配方：局部 ABI 提案

**待批准的具体技术/API 差异；不是已实现能力或发布承诺。** 第二独立业务本来就是 L01/L02 的既定出口，本提案不重新申请是否交付。它把范围稿尚未裁定的 Rust 作者入口、字符串物理表示、错误与普通 C 入口闭合；不新增共同 JSON 字段、公共 IR 类型或功能 ID，不修改现有七／九角色 ABI。

## 1. 唯一权威作者输入与读取链

采用**自然 Rust 业务声明**作为唯一结构权威：作者只写 `pub struct`、自然 `pub fn`、必要的自然错误 enum、实现及现有项目政策。作者不写 C 物理 struct、`extern "C"` facade、out 参数、bootstrap、预期摘要或第二份字段／签名 IDL。`Reading.label` 继续是 Rust `String`，不得偷换成整数或让作者维护 C 数组镜像。

这明确增加一个有限 Rust 声明读取器；**现行直接读取作者 FFI 源的 cbindgen 配方不具备此能力**。新增读取器仅放适配器层，使用当前产品 `Cargo.lock` 已锁的 `syn 2.0.119`（crate checksum `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297`），不用同锁中的 syn 3.0.9，不升级工具或借 Rust 私有编译器 API。

读取链固定为：

1. syn 解析唯一作者模块，提取有限声明形状和原始位置；它不被当作 Rust 类型解析器。源、工具、标准库和编译条件全部冻结。
2. 用已锁 `rustc 1.99.1` 对真实作者模块及生成的签名／字段见证做实际类型检查：函数指针的精确类型、记录完整字段构造、`std::string::String` 的实际类型、enum 的正 i32 tag。禁用/拒绝会隐藏导出面的配置和宏，见下节。检查失败不产生受检计划。源码位置与类型见证各自保留来源；不得把语法解析伪称完整编译器元数据。
3. 将受检结构与现有项目政策转成既有 `/1` IR，经现有 native finalizer 解析政策、检查契约和重算摘要。采用已有 `artifact-derived` 来源精度，不新增 precision 字段值；实际编译器拒绝保留原诊断。
4. 根据同一计划**生成** Rust ABI shim、转换代码与 C 普通入口；再由固定 cbindgen 0.29.4 从生成 shim 得到实际物理头，Clang 读取真实 AST／layout 并核对计划。新配方还须检查实际生成的 `P_E_V` 整数常量与真实 Rust enum tag 一致，并纳入 ABI 常量投影；现 reader 只搜 `MADB_*` 的宏分支不能原样遗漏这些名字。声明名集合来自受检源计划，不能靠宽松前缀接受多余宏。cbindgen 继续承担物理面生成与验证，不能拿手写预期 C 头替代。
5. 实际物理 ABI 投影与本提案语义版本进入既有 native interface 摘要；同次 profile 身份经原规则取得，再嵌入 shim 和普通头。最终 shim 重查生成的声明一致性，之后按 C04 compile/link/run、Source／Build／Candidate 血缘执行。metadata 的签名见证不依赖待生成 SDK，避免构建环。

普通方法名、字段、类型和 enum tag 都来自这一个 Rust 模块；生成模板只是结构转换。允许自动产生签名见证和 FFI shim，禁止要求作者逐方法填写转换表。项目政策只描述线程、复制和交付等现有行为，不重复结构。现有内部 backend 输入仍入锁；不把未注册的 `profile.backend_config` 打开，不偷加公共配置字段。

## 2. 本配方的有限支持集

- 单一显式作者 `.rs` 模块，使用标准库；本片不接 Cargo 依赖图、build.rs、外部模块或 proc-macro。工具和标准库按实际目标锁定。
- 业务记录为具名 `pub struct`，含1–16个具名 `pub` 字段，类型仅 `i32`、`u32` 或 `String`，无嵌套业务记录／借用／泛型／递归。空记录不生成非法空 C struct，超过16字段也准确拒绝；在本节物理布局成立时，最大P_R为1088字节。此局部预算限制ABI暂存，不承诺任意业务函数体的栈／分配上界。记录、函数和字段使用 ASCII Rust/C 标识符；raw identifier、不能无歧义映射的名字及生成名称冲突准确拒绝，不静默改名。
- 函数为安全同步自由函数，无泛型和 lifetime。参数形状仅 `(R)` 或 `(R, i32)`，参数必须是普通命名绑定，可带mut；拒绝 `_`、ref、子模式与解构模式。返回仅 `R` 或 `Result<R,E>`，其中输入输出是同一记录 R。四种组合均是结构规则，不以 normalize/offset 名字决定支持。
- E 为同模块 `#[repr(i32)]` 的无 payload enum，每个 tag 是显式正 i32 整数字面值且互异；其他 `Result<T,E>`、trait error、负/零 tag、payload enum、默认/计算 tag、类型别名拒绝。编译器见证再核实际类型及 tag。`Result` 和 `String` 必须解析为标准库类型，不能由同名用户类型替代。
- 只允许不改变字段/签名的标准 derive 子集 `Clone/Copy/Debug/PartialEq/Eq`（String 记录不能 Copy，由 rustc 拒绝），以及 E 的 repr(i32)。拒绝 cfg/cfg_attr、include、导出宏、proc-macro、自定义属性、pub re-export、trait/impl 公共面、extern/unsafe 公共函数等；未知声明不被忽略。实现函数体由实际 rustc 检查，业务效果仍由显式政策和独立用例约束，解析器不宣称能证明任意函数体无副作用。
- 逻辑 IR 使用现有 integer、bounded UTF-8 string、logical record、sync；Result 使用现有无 payload Variant 和 throws。需要时声明已有 `core.errors@1`、`core.variants@1`，并保留 `core.values@1`、`core.exact-numbers@1`；没有新 feature 或 profile/2 要求。
- 本提案给该有限配方选择**每个 String 最大64字节**。这是新配方限制，不是全产品上限；它限制边界值，不证明业务实现内部不会分配更多内存或阻塞。超范围输入／输出有下述失败结果。

未来扩大类型／签名／编译来源或文本容量，需要明确更新该配方与证据；不能说 syn 能解析就自动支持。

## 3. 精确 C 物理类型与名称模板

公共内联基础声明固定在生成候选的 `include/madb_value_abi_v1.h`；无新增 native intrinsic 导出。不得与 C05 的 `MadbError` 或 checked-engine result 混用：

```c
typedef int32_t MadbValueStatus;
typedef uint32_t MadbValueCallKind;
typedef struct MadbValueError {
    uint32_t domain;
    int32_t code;
} MadbValueError;
typedef struct MadbValueUtf8_64 {
    uint32_t byte_count;
    uint8_t bytes[64];
} MadbValueUtf8_64;

#define MADB_VALUE_CALL_OK                 0u
#define MADB_VALUE_CALL_BOOTSTRAP_FAILED   1u
#define MADB_VALUE_CALL_INTERFACE_MISMATCH 2u
#define MADB_VALUE_CALL_PROFILE_MISMATCH   3u
#define MADB_VALUE_CALL_OPERATION_FAILED  4u

#define MADB_VALUE_STATUS_OK                 0
#define MADB_VALUE_STATUS_INVALID_ARGUMENT   1
#define MADB_VALUE_STATUS_INTERNAL_FAILURE   8
#define MADB_VALUE_STATUS_RESOURCE_EXHAUSTED  9
#define MADB_VALUE_STATUS_DOMAIN_ERROR      10
#define MADB_VALUE_STATUS_UNSUPPORTED       11
```

数值沿用现有状态含义，但类型／宏有新值配方前缀。2–7 不是本配方允许的返回状态；不把资源句柄状态借进无资源 API。C enum 的实现宽度不进入 ABI。

`MadbValueUtf8_64` 的 byte_count 偏移0、bytes 偏移4、size68、alignment4；MadbValueError 为两个4字节字段、size8/alignment4。必须由本目标实际 compiler witness 证明，不因本机例子通过外推所有目标。字节序为所选目标原生字节序，**不是序列化 wire**；不以一般 C struct padding 字节作为稳定载荷或摘要。

P 是现有 `sdk_namespace` 中本配方接受的 C 标识符，R/F 是源记录／函数名，A 是存在时第二个 i32 参数的实际源名；例 P=`reading_demo`、A=`delta`。第一个物理参数固定名 input，输出固定名 out_value/out_error；逻辑 IR 保留原 Rust 参数名。A 与这些保留名冲突时拒绝，不静默改名。命名不使用用户隐藏的额外映射：

```c
/* generated include/P_native.h */
typedef struct P_R { /* fields in actual Rust declaration order */ } P_R;
MadbValueStatus P_bootstrap_v1(uint32_t version, uint8_t *out_identity,
                              uint64_t capacity);
MadbValueStatus P_raw_F(const P_R *input, P_R *out_value,
                       MadbValueError *out_error);
/* optional second Rust parameter: */
MadbValueStatus P_raw_F(const P_R *input, int32_t A,
                       P_R *out_value, MadbValueError *out_error);

/* generated include/P.h; one result type per R */
typedef struct P_RResult {
    MadbValueCallKind kind;
    MadbValueStatus status;
    MadbValueError error;
    P_R value;
} P_RResult;
/* these are static-inline signature templates, with §6 algorithm bodies */
static inline P_RResult P_F(P_R input);
static inline P_RResult P_F(P_R input, int32_t A);
```

模板中的两种 F 只按该函数的实际参数数目选择，不生成重载。只有 `P_bootstrap_v1` 与实际业务 `P_raw_F` 进入 native 导出；`P_F` 是带完整定义的头内普通入口，不是额外 DLL 导出。本模板采用单下划线分隔，避免主动占用双下划线保留名称；本片仍只认证所选 C11 消费，不以此承诺 C++ 运行。C 类型名、原函数名和模板保留名冲突在生成前拒绝。字段直接按源名生成；String 字段变为 MadbValueUtf8_64，i32/u32 保持固定宽度。

E 的每个已声明 tag 生成 `#define P_E_V <正i32十进制值>`；物理错误仍用 MadbValueError，不暴露 C enum 宽度。每个函数只接受其声明 E 的合法 tag 集合；集合外的 tag 被拒绝。两个 enum 如果都有相同整数 tag，裸整数不能证明它来自哪个 enum，不声称能够辨识这种来源。

## 4. UTF-8、转换与内存责任

1. byte_count 是**字节数**，范围0–64；只解释 bytes 的前 byte_count 字节。无 NUL 终止要求；嵌入 NUL 是普通 U+0000，保留。严格拒绝非法 UTF-8，包括截断、过长编码、代理区和超出 Unicode 标量范围。不做 NFC/NFD 等 Unicode 规范化。
2. raw 输入尾部 `bytes[byte_count..64]` 可以是任意已初始化字节，既不是载荷，也不影响逻辑身份；native 转成 owned Rust String 时忽略尾部。原调用方记录不被修改。空串长度0，不读取任何载荷字节。
3. native 输出每个 String 的尾部必须全零；先构造全零的暂存记录，再复制完整合法载荷，最后才交付。成功不写 NUL terminator 到 bytes[64] 之外。空串的64字节全零。
4. C→Rust 每个文本只发生一次必需的 owned String 构造；Rust→C 每个结果只发生一次有界复制。普通 C wrapper 不再转码／分配第二份 Rust String。其输出检查是协议校验，不接管另一套业务资源。
5. 内联 C 值无需消费者 free/close。生成 shim 的 Rust 临时 String 由本次同步调用栈持有，成功、业务 Err 和可捕获 unwind 都释放；不把 caller 指针存入业务对象或全局状态。业务函数接收 owned R，不能因此推导通用线程安全或异步协议。
6. raw 输入指向有效、初始化、对齐的完整 P_R，借用仅到同步返回；输出为有效、可写、对齐对象，输入／两个输出互不重叠且调用期间无数据竞争。任意失效地址／越界分配／别名是 raw 调用者违约，**没有指针有效性 oracle 或别名安全保证**。可检查的 NULL／alignment 按下节拒绝。普通 P_F 使用自身互不重叠的栈临时值，消费者无需管理这些指针。
7. 固定文本上限不等于无堆分配。shim 的输入构造使用可失败分配；分配失败返回9。业务本身的分配、panic/abort 前提必须如实保留；选定 unwind 配置可捕获 Rust unwind，abort/OOM abort/进程故障不被包装成可恢复成功。

业务作者只维护 String 字段；该物理结构及验证／清零代码由已选配方提供。不能让作者为每个字段手写长度、C 数组或验证回调。

## 5. Native 状态、失败输出与业务错误

raw entry 的顺序和所有状态是规范，不由实现自由猜测：

1. 分别检查两个输出的 NULL/alignment；每个符合机械前提的输出独立清零，即使另一个输出非法。只在前述有效内存／不别名前提内承诺写入；不能读取或清理非法地址。
2. 任一输出非法，或输入 NULL／不对齐：返回1，不调用作者函数；仍合法的输出保持零。
3. 按声明字段顺序检查所有输入文本的长度／UTF-8，再构造 owned Rust 值。非法内容返回1；可报告的转换分配失败返回9。两者 error={0,0}、out_value 全部语义字段及文本存储为零，作者函数调用数0。
4. 调用该自然 Rust 业务函数**一次**。R 返回视为 Ok；Result<R,E> 的 Err(E::V) 返回10、error={domain:1,code:tag(V)}，out_value 仍零。domain=1 是本值配方的业务错误域，tag 由真实 E 声明固定；不将错误 tag 当作 status 值。
5. 返回 R 后，逐字段检查 String 字节长度。任何结果超过64返回9且 error={0,0}，释放完整 Rust 结果，不交付部分 C 结果。合法结果编码到全零暂存值，全部完成后复制到 out_value；返回0、error={0,0}。
6. 可捕获 Rust panic 返回8、error={0,0}、out_value 全零；没有成功值或业务 Err 的替代含义。不得靠 panic 实现 offset 的溢出 oracle。天然不应出现的 adapter 不变量失败同样是8。

| raw 状态 | error | out_value | 作者函数调用数 |
|---|---|---|---|
| 0 成功 | {0,0} | 完整值，所有文本尾部零 | 1 |
| 1 输入／机械指针前提失败 | {0,0}，只清有效输出 | 零，限有效输出 | 0 |
| 8 可捕获 panic／内部失败 | {0,0} | 零 | 0或1，按真实失败阶段记录 |
| 9 转换分配失败／结果文本过长 | {0,0} | 零 | 输入转换失败0；结果越界1 |
| 10 Err(E::V) | {1,该函数E的已声明正tag} | 零 | 1 |

infallible Rust 函数不能合法返回10。bootstrap 之外返回11、2–7、未知值，或状态／error／结果形态不相符，都不是合法 provider 行为。

## 6. Bootstrap 与普通值入口的唯一错误顺序

bootstrap 签名和64字节 payload 顺序复用现有机制：version1，32字节 interface 后32字节 profile。NULL 或 capacity!=64 返回1且不写；有效64字节先清零，未知 version 返回11；成功写完整身份并返回0，不调用业务。它使用新的 P 前缀，与旧 madb_fixture_bootstrap 不混同。

每次普通 P_F 调用均执行以下顺序，无全局缓存、隐藏资源或重试：

1. 初始化 P_RResult 的 kind/status/error/value 为失败安全值，actual[64] 为零；调用 `P_bootstrap_v1(1, actual, 64)` **一次**。
2. bootstrap 非零立即返回 kind1；status 保留允许的1/8/9/11，否则规范化为8；error/value为零。调用业务0次，不继续检查身份或输入。
3. 比较 interface 的32字节；不同返回 kind2/status11/error0/value0。只有 interface 相同才比较 profile；不同返回 kind3/status11/error0/value0。因此两者都错时 interface 优先。预期字节由同次生成身份写入头，不是作者／消费者参数，也不含 candidate_digest。
4. 身份成功后，wrapper 传自身 input 副本和独立全零输出／error 暂存，调用对应 raw entry **一次**。wrapper 不预先重复解析文本；raw entry 自己负责输入准入，因此非法文本时 raw entry 调用1次、自然 Rust 作者函数调用0次。
5. 核对 native 返回协议。合法非零按 kind4 返回该 status/error，value保持零。若 raw 返回0却输出非法 UTF-8、越界长度、非零尾部或非零 error；或失败却交付非零值、非法状态、未知/不适用业务 tag，则返回 kind4/status8/error0/value0。不得把不合格 provider 输出交付调用者，也不再调用业务。
6. 只有 raw 返回0且完整输出合规，返回 kind0/status0/error0/完整value。

`status` 是**该普通入口的最终状态**，不是“最近一次原生返回值”的记录；因此 identity mismatch 可明确为11，输出协议违规可明确为8。调用者先看 kind，再看 status/error；任何 kind!=0 的 value 均为零。负例报告保留真实 raw 状态和阶段作为附件，不假装它们等于公开最终状态。

错误优先级为 bootstrap失败 → interface错 → profile错 → raw输入准入 → 输入转换 → 作者调用/Err → 输出容量/转换 → wrapper协议核对。身份失配优先于非法文本；profile失配不得进入任何作者函数。missing symbol／wrong target 的链接或装载错误不冒充上述运行状态。

## 7. 两份实际作者输入与 C 预期例

实存作者源在 `l02-value-abi-examples/author-0.rs` 与 `author-2.rs`；二者是可编译自然 Rust，没有 FFI facade、IDL 或业务映射名单。

作者输入 A（完整）：

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub value: i32,
    pub label: String,
}
pub fn normalize(mut value: Reading) -> Reading {
    value.label = value.label.trim().to_owned();
    value
}
```

对应 C 预期：

```c
typedef struct reading_demo_Reading {
    int32_t value;
    MadbValueUtf8_64 label;
} reading_demo_Reading;
/* size72/alignment4; actual target must prove it */
typedef struct reading_demo_ReadingResult {
    MadbValueCallKind kind;
    MadbValueStatus status;
    MadbValueError error;
    reading_demo_Reading value;
} reading_demo_ReadingResult;
MadbValueStatus reading_demo_raw_normalize(
    const reading_demo_Reading *, reading_demo_Reading *, MadbValueError *);
/* full static-inline body is generated from §6 */
static inline reading_demo_ReadingResult reading_demo_normalize(reading_demo_Reading input);
```

作者输入 B（完整）：

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub value: i32,
    pub label: String,
    pub quality: u32,
}
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingError { Overflow = 1 }
pub fn normalize(mut value: Reading) -> Reading {
    value.label = value.label.trim().to_owned();
    value
}
pub fn offset(mut value: Reading, delta: i32) -> Result<Reading, ReadingError> {
    value.value = value.value.checked_add(delta).ok_or(ReadingError::Overflow)?;
    Ok(value)
}
```

对应 C 预期在 A 的 record 最后增加 `uint32_t quality`（偏移72、size76），P_RResult.value随之变大；新增：

```c
#define reading_demo_ReadingError_Overflow 1
MadbValueStatus reading_demo_raw_offset(
    const reading_demo_Reading *input, int32_t delta,
    reading_demo_Reading *out_value, MadbValueError *out_error);
static inline reading_demo_ReadingResult reading_demo_offset(
    reading_demo_Reading input, int32_t delta);
```

C 普通调用示例：

```c
reading_demo_Reading input = {0};
input.value = 7;
input.label.byte_count = 10;
/* UTF-8 bytes of "  温度  "; ordinary model data, not an identity */
const uint8_t label[10] = {32,32,0xe6,0xb8,0xa9,0xe5,0xba,0xa6,32,32};
memcpy(input.label.bytes, label, 10);
input.quality = 42; /* only B */
reading_demo_ReadingResult result = reading_demo_normalize(input);
/* expect kind=0,status=0,value={7,"温度",42}; no handle/free/CLI */
```

模型值的普通构造不要求调用方写 hash、UTF-8→Rust 转换、原生操作句柄或释放器；边界负责检查实际字节。未来若另加方便的字符串构造函数，另按实际需要提案，本次不预先扩大公开 API。

业务 trim 采用作者实际 Rust `str::trim`；oracle固定实际输入／预期，不能由生成器推断。这里没有宣称 Rust 与参考 `.ets` 在所有 Unicode 空白字符上完全相同；保留 label 与独立值业务要求，跨语言全字符集合等价不偷偷加入本次承诺。

## 8. 必须预先固定的正反向量

以下是采用后实现／运行验收向量，**除下节声明检查外尚未执行**：

| 向量 | 固定输入 | 精确预期 |
|---|---|---|
| 普通文本 | value7、`"  温度  "`、B quality42 | kind0/status0，value7，6字节UTF-8“温度”，尾部零，quality42；raw=1、作者=1 |
| 空／NUL／多字节 | 空串；`a\0b`；补充平面字符；恰好64合法UTF-8字节 | 无终止符截断或非法转码；长度按字节，按作者行为返回，尾部零 |
| 非规范尾部输入 | 合法短载荷且尾部非零 | 尾部不作为载荷；调用者input不变；输出尾部全零 |
| 输入边界拒绝 | byte_count65，截断、多余续字节、过长UTF-8、代理区序列 | identity正确后 raw=1/作者=0；kind4/status1/error0/value0 |
| 结果过长 | 独立作者正向返回65字节String | raw=1/作者=1；kind4/status9/error0/value0；Rust临时值清理，不提交半个输出 |
| 普通业务 Err | offset(i32::MAX,1) 或 i32::MIN,-1 | raw=1/作者=1；kind4/status10/error={1,1}/value0；不 panic |
| 错身份优先级 | 文本非法同时 bootstrap失败/双digest错/仅profile错 | 分别kind1/2/3；全部raw业务=0、作者=0；两digest错以interface优先 |
| bootstrap顺序 | 错capacity；有效容量+未知version | 前者status1、不写；后者64字节清零/status11；作者0 |
| NULL/对齐 | raw的一个输出NULL而另一个满足有效前提；input NULL | status1，合法输出独立零、作者0；不使用任意野指针或重叠对象伪造“安全拒绝” |
| 原生异常 | 受控Rust unwind；可失败输入分配 | 分别8/9，error0/value0；按真实阶段核作者0或1；不称能捕获abort/OOM abort |
| provider协议违规 | 0+错误error/坏UTF8/非零尾；失败+非零值；status5；不适用错误tag | 普通入口kind4/status8/error0/value0，raw仅1次，不重试；使用独立坏provider测试镜像，不进入正常候选 |
| 四个作者变更 | author-0→1加offset→2加quality→3改名canonicalize→4删offset | 每步实际生成/编译/候选消费；原样本见证中的旧签名/字段错误准确失败；删除无陈旧头/导出/责任项 |
| 声明拒绝 | 空/17字段record；参数解构；Vec/借用/浮点/payload错误/cfg/宏/外部类型/同名String或Result | 在读取／编译见证阶段定位拒绝，不写新Source；不改业务名白名单 |
| 来源与事务 | 改源/工具/schema字节；错policy selector；生成中途失败 | 现有冻结/政策/事务拒绝；旧完整批次及用户文件保留 |
| 跨版本混装 | A头+B库、B头+A库、新值头+旧engine库 | 正确区分文件/链接/identity拒绝；不试调未知业务ABI，不借旧7/9通过 |

正常导出集：A 只有 bootstrap+raw_normalize；B 只有 bootstrap+raw_normalize+raw_offset；rename/delete随真实声明变化。不能通过“所有 madb_ 前缀都算允许”检查 native 符号。

## 9. 本次实际检查与未执行边界

`l02-value-abi-examples/revision-1/declaration-check.json` 保留最初工具真实路径、版本、SHA-256、命令及输出；`source-snapshot.json` 保存该次 C 源字节。五份自然 Rust 作者源与五份手写独立类型见证经 rustc 1.99.1 metadata 编译，10项exit0且源字节未变。将提案函数名改为单下划线后，两个**手写 C 预期**重新通过实际 Clang 的 C11 声明及布局断言检查，记录于 `declaration-check-002.json`。最终例子共有12项适用的类型／声明检查；没有将旧 C 字节结果当作修订后结果。

这只证明所给作者输入和预期声明可表达／可编译；尚无新增 syn reader、真实生成 shim、cbindgen生成新头、运行时实现、Candidate或独立Lab消费。见证当前手写用于设计审查，不是已实现的自动读取链；C例不是假称生成产物。正式实现必须按固定工具重新生成、编译、运行和独立验证，不能沿用这个计数当验收。

## 10. 兼容、责任与本次批准边界

新值配方是独立局部语义版本（建议 `madb.native-values/1`，此处尚未登记），通过已有内部 recipe、profile/1与输入锁选择；旧 `madb.first-route/1` 继续精确拒绝新增记录／函数。新数据不自动进入旧reader，旧Profile/1未知必需功能拒绝和摘要含义不变。

既有 native-interface/1 摘要域继续携带所选语义版本、bootstrap版本和实际完整ABI投影；String容量、函数／字段／错误tag、物理布局改变会改变实际身份。新值身份不等于旧engine身份或candidate_digest。以后改变此值 ABI 的公开含义／容量／失败责任按局部版本规则处理，不承诺64→其他容量二进制兼容。

批准范围仅为：上述自然Rust有限读取／compiler见证链、固定文本和同步值C ABI、Result无payload错误映射、自动身份普通入口、对应责任／真实生成／独立消费。没有新的 JSON schema／字段／feature，没有 profile/2、资源／async／回调、C++ RAII、通用Rust支持、诊断API扩充或新目标承诺。

DEC-07 的 required 从已受检IR／有效政策及本有限规则独立建立；provided来自实际生成步骤，继续使用核心比较器并核Source/Package真实资产。native转换、wrapper身份检查、输出协议检查各有唯一职责；无需为纯值捏造订阅或排水。

现有默认/test/diag/combined的7/14/9/16导出、configure/drain、madb_engine_create_checked及其所有历史候选结论均保持。新配方不依赖旧engine来取得身份或错误类型，不加载MadB工具核心；消费者无CLI/编译生成器依赖。现有候选验证可继续，不等待本提案采用。
