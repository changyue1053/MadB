# L02 explain：入口、来源上下文与机器结果的具体候选

状态：待项目所有者裁定，未采用，未写入生产 CLI/schema。`explain` 归 L02、只读和未知码失败的责任已批准，不重新请求该批准。本提案只定此前未确定的调用参数、文件形状、退出语义及兼容边界。L02 其他四项验收继续执行，不能凭本提案或解释查询关闭整包。

任务输入：`/private/tmp/madb-chief-round6/madb`，HEAD `bc32a61e0dd0fa4f35e57421c309619304b97381` 加本轮当前源字节。已实际读取 AGENTS、共同方法、L02 全部 documents/acceptance、architecture/10、现行 CLI、Finding、Report/common、真实 Rust/C 和 ArkTS 诊断入口。精确来源 pin 和设计样例见同前缀 design JSON。产品源码、catalog 与已有 wire 均未修改。

## 1 推荐决定与实际差异

推荐一次采用本提案第2–8节：新增 `madb explain CODE`，默认文本，可显式 `--json`；无上下文时只查随工具固定的说明表。新增一个结果 envelope `madb.explain-result/1`，以及可选、仅用于提供已发生诊断及来源字节身份的 `madb.diagnostic-context/1`。两者均离线、闭合、共享格式向量验证。

复用 Report 的诊断原子 `{code,message,location}` 及 common 的 Id/File/FileRef/Location；不把查询结果包装成 `madb.report/1`。Report 必需候选、Lab revision、selector、case、环境和执行证据，静态解释没有这些事实，不能填假值。`Finding` 是内部领域值，不直接变为公共 wire；现有多来源 Finding 的每个真实位置可分别形成一个 context，本版不把一个位置宣称为完整关联范围。

新增 context 的原因：现有诊断原子只有路径/行/列，没有来源字节摘要或列单位；仅从文本猜定位会重现 UTF-16/BOM 偏移错误。现有 ContractIR 在 reader 失败时可能尚不存在，不能强制每个错误先取得成功 IR。context 不授予诊断真实性、候选或执行认证；它只允许查询核对调用者明确提供的诊断记录、源字节和位置。可以从真实 reader 的已捕获错误与 source File 身份产生，也可由调用者显式准备；两种来源均不因自报 `compiler-derived` 获得独立证明。

替代项：仅支持 CODE 查询可先实现，但不足以交付声明来源绑定；把未知格式塞进 Report/extensions 不接受；从任意 stderr 正则推断新诊断、源码路径或编译器版本不接受。若决定暂不采用 context，本轮必须保留来源绑定的缺项，不能宣称 explain 完整验收。

## 2 CLI：精确语法与输出通道

```text
madb explain CODE [--json]
madb explain CODE --context PATH [--root DIRECTORY] [--json]
```

- CODE 必须是一个满足 common.id 的精确、区分大小写的稳定码。没有别名、前缀猜测、模糊纠错或把 `TYPE-001` 自动映射成 `TYPE001`。
- context 是根目录相对的 LogicalPath，不接受 `-`、URL 或第二个 context。`--root` 默认当前目录；显式 `--root` 仅在有 context 时有效。无 context 查询不打开工作区、配置、锁或源码，不能为了查词条运行探测。
- context 文件及其 source.path 全部相对同一个 `--root`，不是 context 的父目录。源码路径和诊断 location.path 必须精确相同；不会搜索 cwd、缓存或 basename 替代。使用已有宿主只读文件边界，拒绝路径越界、大小写别名及链接绕过。
- 不增加 `--fix`、安装、更新、执行或扫描选项。忽略文件、锁、缓存、候选、源码均不写；不启动编译器、扩展进程、设备或网络。
- 默认文本：解释成功写 stdout，查询失败写 stderr。`--json`：成功及可表达的失败均在 stdout 只写一个结果对象和换行，stderr 不混入正常说明。原六命令的参数和输出保持。
- 对已选择 explain 分支且存在独立 `--json` 标志的参数错误，也输出失败对象；非法/缺失 CODE 使结果 code=null。`--help`/`--version` 是说明入口，仍使用 CLI 的文本帮助并退出0，不是 explain-result。未知顶层命令保持现有 CLI 行为。
- stdout 写入/刷新失败返回 Host/3；管道失效时不能保证可读的完整 JSON，也不先输出成功再声称错误对象已交付。

## 3 可选 `madb.diagnostic-context/1`

顶层字段全部必需、禁止额外键：

| 字段 | 精确类型 | 约束 |
|---|---|---|
| schema | 常量 `madb.diagnostic-context/1` | 未知版本拒绝 |
| diagnostic | 既有 `{code,message,location}` | code 为 common.id；message 非空、最多4096字符；location 为 common.location 或 null；code 必须等于 CLI CODE，不从 message 改写 code |
| source | common.file 或 null | `{path,sha256,size}` 绑定原始源字节；不是语义摘要，也不使用假的 schema 标记源码 |
| position_encoding | `byte-lf`、`utf16-js` 或 null | 精确定义见下；不根据文件扩展名或 code 猜测 |
| reported_precision | `compiler-derived`、`artifact-derived`、`schema-derived`、`heuristic` 或 null | 与现有 IR 精度枚举一致；仅记录输入的声明，不升级其可信度 |

`diagnostic.location`、source、position_encoding 必须同时非 null 或同时 null。缺位置的已有诊断可被解释，但结果 source=null，不能补行号。三者为 null 时 reported_precision 必须为 null；有位置而精度未知可为 null。

- `byte-lf`：原始字节中的一基行、一基字节列，只以 LF 分行，BOM/CR 均是原字节；用于现有 Rust/C reader 的 offset→位置规则。不要求任意二进制是有效作者声明，也不宣称它提供符号级来源。
- `utf16-js`：源必须是合法 UTF-8；与现有 ArkTS reader 一样只移除首个 UTF-8 BOM 后计列，CRLF/CR/LF/U+2028/U+2029 分行，列按 UTF-16 code unit 一基计数。代理对中间、行外位置拒绝。结果 byte_offset 始终指向原始 UTF-8 字节（包括初始 BOM）。
- 允许位于一行末尾或文件末尾的合法诊断点，不将其误称为已定位的声明范围；不提供不存在的 end、symbol ID、模板映射或 producer 身份。
- context 使用既有1 MiB/64层/安全整数字面量/重复键拒绝规则；本版单个源文件也最多1 MiB，与现有 ArkTS helper 的边界一致。超过上限明确失败，不截断后成功。
- 首次捕获就不符 source.sha256/size 是 InvalidInput/2；已捕获并核对后、本次复核期间来源变动是 Conflict/4。位置根据固定已读字节计算。此检查不是“原编译器确实发出了这条诊断”的证明。

## 4 `madb.explain-result/1`

所有字段必需，所有对象闭合，未知字段拒绝；不引入新摘要域。机器字段与枚举稳定，面向人的解释可以改进。

| 字段 | 类型 | 规则 |
|---|---|---|
| schema | 常量 `madb.explain-result/1` | 独立版本，不改 Report 或 IR |
| tool_version | 非空字符串，最多128字符 | 实际 `CARGO_PKG_VERSION`；不伪造未取得的工具镜像摘要 |
| code | common.id 或 null | 实际查询码；未形成合法码时 null |
| context | common.file_ref 或 null | 只有实际读入的 context 才填；schema 必须是 diagnostic-context/1，path 相对本次 --root，hash/size 为读入原字节；未读或解析失败可 null |
| explanation | 下述对象或 null | 只有整个查询成功才非 null；有词条但源绑定失败仍不返回部分成功 |
| source | 下述绑定对象或 null | 无来源/位置时 null；失败时 null；不从“有位置”推导事故已验证 |
| diagnostics | 既有诊断原子数组，最多64项 | 查询自身的失败；不把原事故的失败码改为查询退出码；成功为 [] |
| exit_code | 0、2、3、4 | 仅本次解释查询的结果；不会把原编译/业务失败再当作查询失败 |

explanation 精确为 `{meaning, triggers, repairs, limits}`：meaning 非空文本；triggers/limits 为有序文本数组（每项最多4096字符、最多64项，triggers非空）；repairs 为1–16项。每项 repair 为 `{kind,instruction,location}`，kind 取 `edit-author-input|inspect-configuration|restore-locked-input`，instruction 非空且最多4096字符，location 是已绑定的真实位置或 null。

修复入口是实际作者文件/策略/锁的检查或编辑动作，**不是自动修复命令**。有已绑定来源时提供可定位入口；仅 CODE 查询给具体动作及所缺上下文，不制造路径或不存在的 argv。指令必须要求修改权威作者输入或显式恢复固定工具/输入，不指导手改生成物、删必需 feature 或降级安全保证。需要执行命令的后续体验须使用真实可用入口和完整参数；本候选不增加 shell/任意命令 wire。

source 精确为 `{reference,location,position_encoding,reported_precision,byte_offset}`，reference 为已核对 common.file；location/position_encoding/reported_precision 与合法 context 一致；byte_offset 为0起始安全整数。reference.path 必须等于 location.path。该对象只声明源字节和位置核对成功，reported_precision 仍是报告者声明；不表示 compiler/reader 调用被独立认证。

成功的联合条件：exit_code=0、code非null、explanation非null、diagnostics=[]；source可为null。失败联合条件：exit_code非0、explanation/source均null、diagnostics非空。未提供context时context/source均null。stdout无法交付除外，不允许“字段说失败、实际退出0”。

## 5 支持码与错误分类

首个完整实现的说明表精确覆盖以下47个已经存在的码；每码都有独立 meaning/触发分支/修复/限制，不按前缀匹配，不用一段通用文字假装覆盖全部码。此目录不是承诺这些码对应的所有语言或高级语义都可执行：例如能解释未支持能力，不表示该能力已支持。

| 码集合 | 权威实现/含义边界 |
|---|---|
| NATIVE001–NATIVE006 | rust_c driver/support/native_ast/lowering 的实际分支；分别覆盖配置、工具镜像、工具执行、受支持导出子集、ABI形状/布局、来源/冻结元数据；同码多个分支全部说明，尤其 NATIVE006 不仅是文件变动 |
| SEM001–SEM009 | contract.rs：缺必需策略；取消效果/确认不匹配；stream交付/停止缺失；event错误合并；资源关闭/值复制错误；不可注销回调缺source lease；释放器逻辑签名错误；异步借用超出call-frame；返回借用随调用失效 |
| POL001–POL010 | policies.rs/contract.rs：选择器非唯一、slot缺失、默认策略冲突、显式策略冲突、非法authority/family、危险推断默认、family缺失、重复有效slot、缺符号、策略键值不符 |
| CAP001–CAP004 | CAP001：已解析 profile/target/direction 选择不符；CAP002：未知/受限/不支持的必需 feature；CAP003：heuristic 结构来源精度不能授权安全 wrapper（不是调用形态不支持）；CAP004：应有 feature 未声明，包含 resource close feature 缺失 |
| TYPE001–TYPE009 | 重复类型、缺类型、递归/过深值布局、重复参数/字段/常量、非法标量/缓冲、variant名称/tag重复、嵌套资源/回调或非标量缓冲不支持、variant tag类型不符、错误通道类型不符 |
| ID001–ID006 | public/source重复、source key摘要不符、派生public ID不符、已退役ID复用、来源非一基 |
| NUM001 | 实际精确整数/浮点位模式/常量校验分支；不将安全JSON数字与业务精确数值混为一谈 |
| ARKTS_DECL_UNSUPPORTED_MODIFIERS / ARKTS_DECL_CALLBACK_RESULT | 复用已交付两个精确解释；未编码reader失败和新的纯值reader失败不得自动套入 |

查询自身新增 EXPLAIN001–EXPLAIN004 四个稳定码，也有静态说明；总说明表51项。EXPLAIN001 精确意为“本版本说明表没有该码”，不声称其他模块不存在它。查询尚未列入本表的旧码也明确非零，不猜解释；以后添加词条必须有实际产生方、完整触发含义与正反例，不能悄改已有码含义。

| 场景 | 查询诊断 | exit |
|---|---|---|
| 已知码；可选来源全部合法 | [] | 0 |
| CODE不在本版51项说明表 | EXPLAIN001 | 2 |
| CLI/JSON/schema/未知字段/码不符/位置模式/首次源pin或位置不符 | EXPLAIN002（可保留现有格式诊断作为cause项） | 2 |
| 明确文件不存在、权限或读取/输出I/O失败 | EXPLAIN003 | 3 |
| 捕获核对之后本次固定输入发生变化 | EXPLAIN004 | 4 |

不使用 CheckFailed/1：查询不执行原检查。解释 SEM007 成功退出0只表示解释已提供，不表示释放器问题已修复。参数合法性、说明表查找、context解析、来源读取/核对依次进行；未知码在读context之前返回2，不启动工作区处理。取消/崩溃/系统信号沿现有进程事实记录，不虚构完整JSON成功。

## 6 来源接线与实现边界

- 说明表由固定工具携带；纯查询/汇总逻辑进入现有 app/model 职责的小模块，JSON转换由 protocol/contracts 适配层承担，文件读取留给host，CLI只解析/展示。无需新增crate、任务系统或运行时。
- 现有 ArkTS `declaration_diagnostics.py` 是Python私有实现，只支持两码。实施必须避免Rust CLI与Python解释含义漂移：采用一份受版本控制的说明数据、两端只读装载/生成，并保留精确reader错误匹配与BOM映射。不因字典存在便接受任意带同名字段的stderr。
- context捕获使用真实调用的结构化失败、原始source字节和相应位置规则；不能从用户文案猜字段。现有 Rust/C NativeError 尚无统一位置时保留location=null，或用受检实际声明/策略来源补齐，不伪造编译器精度。解释查询本身不重跑reader/编译器来“补位置”。
- 本版context来源证明只到“提供字节和点位置一致”，不认证producer、完整符号范围、模板展开链或候选。以后若需要独立重验发生事实，沿已有read request/IR/执行证据身份，不把context升级成证据证书。
- 不更改普通SDK错误、资源owner/线程/关闭、生成端口payload和原六CLI行为。新format进入现有SchemaRegistry及共同向量，旧参与者认识不到就拒绝，不能降为Report或塞入extensions。

## 7 正反例与检查条件

设计JSON给出完整成功/失败结果与context实例，使用明确合成源字节；设计脚本只验证形状、组合约束、精确码表和来源位置片段，不能代替生产CLI/host/reader运行。

采用后必须在真实实现上检查：

1. CODE文本/JSON、无工作区查询、51码独立说明；未知码、大小写/旧拼写、非法参数都准确失败，输出和退出一致。
2. 来自真实Rust/C资源/回调规则失败及真实ArkTS reader失败的context；有来源则实际字节/位置一致，没有位置则明确null；命令不启动reader、工具、网络或写入。
3. BOM、astral字符、CRLF/CR/LS/PS、UTF-16代理对中点、byte列、错path/sha/size、源替换、未知位置模式、重复键/超预算；不只测试手造成功字典。
4. 非法context/失败查询无partial explanation；输出写失败返回Host；不把原事故exit1当解释失败，也不把解释exit0写为原问题修复。
5. 保留现有诊断关闭保护、旧6CLI、旧wire/IR/Report向量、源码精度及候选身份；新说明数据/生成helper纳入实际工具输入身份。

## 8 L02其余验收与第二个SDK决定问题

| L02原验收 | 可继续的已授权内部实现 | 仍需证据/明确差异 |
|---|---|---|
| 声明复用、普通入口独立消费 | 复用C05真实cbindgen/bindgen/Clang与既有lowering来源 | 旧C→Rust消费者由harness手填expected_hex；新普通构造入口必须由生成装配带入身份，不能借用旧报告宣称当前DX-01完成 |
| 诊断关闭保护 | 既有准入/关闭不变；内部有界hook与故障对照可推进 | 现有test-support panic sink不是生产hook；公开configure/drain或稳定事件记录是新的SDK契约，独立具体提案 |
| 首链资源/回调与诊断 | 复用现有7普通ABI、同步初始回调和显式request/await/release，不增加producer/队列/异步业务 | 普通/诊断关闭的新候选由独立Lab消费，缺释放器/错误线程策略等来源诊断与覆盖分别说明 |
| 每符号阶段责任表 | 从真实lowering roles、已解析策略及锁定组件派生内部值，由核心核查独立必需集/唯一owner | 现有runtime-protocol的五项组件级mechanisms不够；公开责任JSON若成为必需交付资产另审，不能为C05 profile/1伪造composition |
| explain | 采用本提案后实现CLI、字典、来源绑定和机器正反例 | 本设计检查不等于该项验收，更不代替前四项 |

checked constructor 推荐生成独立 `include/madb_rust_client.h`、静态内联构造helper，使用同一已核验 identity.json 的interface/profile字节先bootstrap再创建；错误时不调用create、不把candidate摘要嵌入自身。其新公开名称、返回错误结构及资源所有权由主协调安排的第二个SDK提案统一定稿，本文件不批准具体ABI。生产hooks、constructor与任何native并发变更须独立复核并绑定新候选。

范围仍是L02 S1基础及已有ArkTS诊断首片；不是所有语言、全部Swift/ArkTS扩展或L03–L07整包。C05/C06/C07既有冻结结论不改，新增源码/接口不继承旧候选的运行通过。未知公共格式仍等待项目所有者决定，已授权内部工作继续。
