# 第十一批 I02 doctor：公共入口与只读结果提案

状态：待独立审查、待项目所有者裁定新增公共入口参数及结果格式；未实施。I02 归属、只读责任及验收已批准，不重复提请。依据实际产品 HEAD `9a72a6417858b7ee1a141ce8b13fa47ba7ef2f94`（检查时干净），AGENTS、共同方法、catalog I01/I02/I03、ADR-0019/0023/0024、architecture 07/14、validation 06。旧报告、冻结安装和工具均未修改。

## 1. 推荐范围与复用结论

推荐首片为**已有冻结工程上下文的 doctor**，沿用 `check/inspect/plan` 的 project/read-request/snapshot 输入及显式根；检查当前选择的 profile 在请求级别的静态前提。首个可执行配方观察仅覆盖现有 macOS Arm64 Rust→C values 的固定安装组合。doctor 不调用它来生成、编译、运行、修锁或安装，不把预检满足称作构建或 SDK 验收通过。

这样不新增输入 wire、模板选择机制、工具发现协议或包下载接口。模板选择式 doctor（创建前 `--template/--profile/--targets`）留作同一 I02 后续子范围：它能复用现行 CheckedTemplate/库存/requirements，但不能在首片用空 project/lock/snapshot 伪造同一上下文。I02 整包和 I03 前置不由此解除。

现有顶层结果不能无损复用：

| 既有对象 | 可复用 | 不能借用的语义 |
|---|---|---|
| `QueryRequest/QueryOutcome`、`SnapshotAdapter` | 输入路径、完整 read/lock/profile/IR 交叉校验、声明输入复读 | 只有 Checked/MetadataRequired，未观察真实工具/SDK，无机器结果 envelope |
| `madb.init-result/1` | 七字段 ToolObservation、Diagnostic、Recovery、退出类别 | 普通成功要求真实 CommitNew；dry-run 要求冻结 init 请求/输出/空 checks，doctor 不能冒用 |
| `madb.report/1` | 环境及诊断行、执行状态词汇 | 强制 candidate/lab revision/case catalog/case digest；本机预检不存在这些事实 |
| `madb.explain-result/1` | 稳定码和已有修复说明 | 单码解释/来源 context 不是多工具与 SDK 观察；不能把整份检查塞进 message |
| `madb.execution/1`、lock/profile | 只读解码、身份关系与有限配方判定 | execution 是执行请求，不能新增 doctor operation 旁路现有五操作 |

因此推荐新增**一个输出格式** `madb.doctor-result/1`，复用共同字段定义；不改上述旧格式、不增加 extension operation、不扩展 report.extensions 偷渡新语义。不新增 DoctorRequest wire。

## 2. CLI 精确定义（本次新增公共差异）

```
madb doctor --root <DIRECTORY> --project <PATH> --request <PATH> \
  --snapshot <PATH> [--verify structure|build|run] [--json]
```

- `--root` 默认 `.`，仅作打开的只读工程根。其余三个文件参数必填，均沿现有 LogicalPath/同根引用规则，不接受绝对路径、`..`、链接逃逸、URL 或 shell。
- `--project` 沿 SnapshotAdapter 接受现行 project JSON/TOML；`--request` 是现行 `madb.read-request/1`；其受检 selector 唯一选择 boundary profile。`--snapshot` 指现行 IR 的预期路径，文件缺失是 MetadataRequired，不隐式运行 reader。
- 不添加第二个 `--profile` 覆盖器：profile 由 read-request 的完整 selector 和 FileRef 决定，并与 project/lock/IR 交叉核对；不由字符串名称换锁。报告保留该 selector。
- `--verify` 默认 `build`，只选择**前提检查集合**：structure 检查配置/声明输入/IR/有限配方和相应宿主；build 再核对固定工具、SDK 与实际执行资源；run 包含 build 前提及该配方实际运行宿主要求。三个值均不启动生成、编译、链接、冒烟、IDE 或外部工具的 `--version`。
- `--json` 使 stdout 为一份严格 JSON 加换行；无此选项输出人读摘要，退出值相同。帮助/版本保持既有 Clap 行为。
- 不提供 out/open/update/install/fix/template/tool-root/command/env/URL 参数。固定工具路径来自编译内置受信安装记录；不搜索 PATH、不修改缓存，不要求用户提供新私有安装文件。
- 在有限首片外的 profile/SDK/recipe 返回 unsupported/InvalidInput；profile/2 即使可解码也不因此获得执行环境认证。显式未知参数为 InvalidInput。
- 有效但声明输入或快照陈旧仍可报告能独立确认的工具事实，但整体非零、context检查未通过；绝不在旧 IR 上声称当前声明已验证。无法建立受信 profile/lock 时，不再读取由未受检数据指定的工具或 SDK 路径。

## 3. 输出 envelope 与所有字段

目标 schema ID：`urn:madb:schema:doctor-result:1`，文档 `schema="madb.doctor-result/1"`。严格 JSON、整数子集、重复键/未知字段拒绝、1 MiB 文档上限沿现有协议。所有下列字段均必需；nullable 必须显式 null。所有对象 `additionalProperties:false`，无 extensions。输出是观察资料，不是执行许可。

```
{
  schema: "madb.doctor-result/1",
  tool: Component | null,
  request: Request | null,
  context: Context | null,
  scope: "preflight-only",
  truncated: boolean,
  checks: Check[],
  recovery: Recovery[],
  diagnostics: Diagnostic[],
  exit_code: 0 | 1 | 2 | 3 | 4
}
```

`Component` 精确复用 common.component（id/version/sha256）；`Id`、`Path`、`Digest`、`SafeUInt`、`FileRef`、`Selector` 均复用 common 同名定义。`RootPath` 复用 InitResult 的 output 对象字段 `{path:string,redacted:boolean}`，本首片限定 `redacted:false`，path 为成功规范化的绝对根（1..4096 字符，禁止控制字符）；不增加 URI 或未实现的匿名化语义。结果内没有任何 FileRef 以结果文件所在目录为基准。

`tool` 是本次实际加载的 MadB 镜像身份；成功捕获才非空。参数语法失败且尚未尝试捕获时可为 null、InvalidInput/2；实际捕获失败时为 null、Host/3。null 本身不重分类错误。无法建立实际镜像身份时不填编译常量摘要或重新取 current_exe 冒充加载镜像。

`Request` 所有字段必需：

```
{root: RootPath, project: Path, read_request: Path,
 snapshot: Path, verify: "structure"|"build"|"run"}
```

成功解析参数且规范化根后才非空；它记录请求，不意味着文件存在。root 成功打开后写绝对 path 与 redacted:false；根尚不能规范化时整项 null，并由诊断保留安全截断的用户错误位置。不把错误路径强行当合法 Path。

`Context` 所有字段必需：

```
{project: File, project_format: "json"|"toml",
 read_request: FileRef, lock: FileRef, profile: FileRef,
 snapshot: FileRef|null, selector: Selector, recipe: Component|null}
```

- 所有 File/FileRef 都相对 `request.root.path`，绑定本次真正读取的原字节；project 用 File 加 project_format 明确 JSON/TOML，不给 TOML 字节冒贴 JSON 编码，不把它替换成 JSON 摘要。File 复用 common.file。
- 四份配置原文完成 SnapshotAdapter.prepare 已有的格式及其局部交叉检查后才有 Context；这是已捕获引用，不表示全部配置关系通过。snapshot 缺失/未被接受时为 null。只有 SnapshotAdapter.check/现有 check_frozen_inputs、validate_port_pair、check_contract、policy-resolution 及请求/锁/profile精确FileRef关系全部通过，snapshot 才非空，context.configuration 才能passed。prepare尚未覆盖的project边界/profile/tool/lock关系不补一个宽松判定；缺IR时该configuration行blocked，具体见§5。
- recipe 精确复用 `TrustedTemplateRecord::bundled` 的既有构造，不另算opaque摘要：`id = madb_plan::project::VALUES_RECIPE = "madb.init.rust-c-sdk.values@1"`；`version = tool.version`（本次实际Cargo工具版本，不是模板0.1.1）；`sha256 = tool.sha256`（同一已加载main镜像，包含编译内置recipe和runtime资产/pins）。主工具先由 `FrozenInitializationTool::capture` 取得实际映像事实；随后可信0.1.1 catalog的archive/manifest/inventory/profile原文匹配，`.recipe(VALUES_RECIPE)`确认条目。完整context.configuration尚未通过则recipe为null、行blocked；完整配置通过但未匹配此有限记录则null、unsupported。它不取caller给出的标记、不用磁盘上另一份main、不用工具版本字符串/源码revision代替镜像摘要。
- Context 非空仍可能有失败行；任何字段不是 commit、cache 或运行授权。

`Check` 所有字段必需：

```
{id: Id,
 kind: "configuration"|"input"|"recipe"|"host"|"tool"|"sdk"|"resource",
 subject: string,
 status: "passed"|"failed"|"error"|"unsupported"|"blocked",
 error_kind: "check-failed"|"invalid-input"|"host"|"conflict"|null,
 expected: Fact|null, observed: Fact|null,
 diagnostics: Diagnostic[], blocked_by: Id[]}
```

`subject` 长度 1..4096（禁止 NUL/控制字符），仅展示路径/组名，不授予 IO。id 在同一结果唯一，输出按依赖后 id 稳定排序；blocked_by 有序唯一，只引用前面的真实未通过行，不能自环。passed 要有 observed，diagnostics 可有非错误说明但不能掩盖失败；blocked 必须 expected/observed 均 null、blocked_by 非空；其余行 blocked_by 为空。passed/blocked 的 error_kind 为 null；failed 对应 CheckFailed/InvalidInput，error 对应 Host/Conflict，unsupported 对应 InvalidInput。其余错误行必须带实际 error_kind 及非空 diagnostics；每个行分类保留原错误。未选择级别的行不输出，不用 skipped 混淆请求范围。

`Fact` 是以下封闭 union，所有字段必需；它只记录已观察的种类，不提供任意 JSON 扩展槽：

```
{kind:"bytes", sha256:Digest, size:SafeUInt}
{kind:"digest", domain:Id, sha256:Digest}
{kind:"tool", method:"locked-bytes", value:ToolObservation}
{kind:"host", os:Id, arch:Id, version:string|null, build:string|null}
{kind:"state", value:"checked"|"supported"|"missing"|"unsupported"|"not-checked"}
```

- ToolObservation 精确复用 report/init 的七字段 `{id,version,sha256,host,target,source,arguments}` 及其限制。expected 来自 profile/lock/固定安装；observed 只有在实际普通文件内容和固定镜像 pin 相符后才能给 tool fact。版本来自与镜像字节绑定的既有安装收据，`method:"locked-bytes"` 明确它不是本次启动工具观察 `--version`。错 bytes 只能输出 observed bytes fact，不能仍宣称观察到预期版本；缺文件为 state/missing；读取失败为 null + 实际 Host 诊断。
- digest 的 domain 是封闭枚举 `madb.project/1|madb.profile/1|madb.source/1`，已逐项核对 `madb-protocol/src/canonical.rs` 的 DigestDomain 常量及 contracts/projections.rs 的调用；复用现有投影，不新增摘要算法。
- host 的 version/build 非空时长度 1..128、禁止控制字符；os/arch 使用现有 HostObservation 的规范值（首片 macos/aarch64）。host 从本次 OS 查询得到；未查到的 version/build 为 null 且相应必需检查失败，不填模板版本。expected host 的 null 表示该维度本配方不约束，observed null 表示未观察，二者不能混用为已满足。
- bytes 可表示某固定记录/输入的原字节。SDK/资源组的 expected bytes 绑定固定 inventory 原文，observed state/checked 仅在整组实际文件/链接/类型/OS类别都核过时成立。不能把“inventory JSON 字节相同”当整个资源树存在。

`Diagnostic` 精确复用 report 的 `{code:Id,message:string(1..4096),location:Location|null}`。保留现有 QueryError 的实际稳定码，不为 doctor 改掉 FILE/LOCK/HOST/PLAN/META 分类。既有错误不改码。本提案仅新增 `DOCTOR001`（doctor-result-limit）：观察表无法在1MiB输出预算内完整编码，属于Host错误；它不同于 HOST003 的文件读取预算，不滥用原码。其含义/触发/修复说明随L02已有explain码表机制登记，修复建议为保留本次已报告错误并缩小输入范围/采用后续受支持查询，不运行安装或构建。该新增稳定码属于本次待裁定公共差异。

`Recovery` 精确复用 InitResult `{kind:"retry"|"manual"|"inspect",message,arguments:string[]|null}`：message 为 1..8192；arguments 非空时 1..4096 项，单项 0..4096、首项 1..4096；所有 argv 字符串禁止 NUL。argv 是展示，不执行。必须由内置有限修复表产生，禁止拼接不受信报错文本为 shell，不建议重复 init 覆盖现有根；缺完整 metadata-execution 时仅给 manual/inspect，不虚构可运行的自动修复命令。

预算：checks 最多 4096，顶层 diagnostics/recovery 各最多 128，每行 diagnostics 最多 16，blocked_by 最多 4096；字符串沿复用类型上限。固定资源组在行内报告首个实质错误，不能宣称穷举所有文件缺项。报告须明确组状态及检查停止位置；独立工具组继续检查。若生成报告会超过 1 MiB，按§5的精确缩减规则生成另一份完整JSON；不能截断JSON字节，也不能把已有Conflict改成Host。truncated只描述完整检查表是否因输出预算被缩减，不表示通过、所有缺项已穷举或IO读取是否成功；正常结果为false。

## 4. 必需检查集合与等级

集合由受检配置和内置配方决定，调用方不能提供 required 列表。有限匹配明确复用可信内置模板 0.1.1 的包/manifest 校验及其原 profile/SDK：待检查 profile 原字节必须与该已锁 boundary_ref 相同，SDK FileRef 与原文同时匹配；之后从该模板同一 profile 的 requirements 分级取宿主/工具，不维护第二张版本表。该查表是实现内部使用已有目录，不新增 CLI template 模式。其他 valid profile（含旧 0.1.0）可以解码及报告自身输入检查，但 recipe unsupported，不能偷选新版工具。首片必需表如下（各“组”必须按下述行全集派生，不是可删的建议）：

| 行/组 | structure | build | run | 实际观察与拒绝 |
|---|---|---|---|---|
| configuration（project/read/lock/profile） | 必需 | 必需 | 必需 | 现行格式、语义摘要、profile FileRef、reader组件、工具锁与selector一致 |
| declared-inputs + snapshot | 必需 | 必需 | 必需 | 已声明 bytes/IR pair/规则、复读；缺失或陈旧 MetadataRequired，不自动准备 |
| recipe | 必需 | 必需 | 必需 | 有限 Rust/C values 原支持范围，不能由route字符串单独准入 |
| 当前工具及宿主 | 必需 | 必需 | 必需 | 加载MadB身份及正式版本；复用同一模板宿主 min_inclusive/max_exclusive（当前 macos/aarch64，26.6.2 至不含26.6.3）；build/run还要求编译时真实source revision，原范围保持 |
| 12个固定工具 | 无 | 必需 | 必需 | profile中native10与固定recipe CMake/Ninja；绝对稳定路径、摘要/size，版本与host/target锁一致 |
| SDK | 无 | 必需 | 必需 | profile.sdk精确原文及当前有限native SDK全部120项观察；后续inventory变化必须重新固定，不写死“120即完整” |
| installation/Python/compiler资源组 | 无 | 必需 | 必需 | 当前受信私有记录的完整已定义组；Python五项隔离前提，有限7镜像loader链和OS build类别；不声称完整compiler安装树/OS沙箱 |
| run-host | 无 | 无 | 必需 | 当前配方host-run的实际宿主/target条件；不运行普通C程序，不生成运行passed |

行ID固定为 context.configuration、context.declared-inputs、context.snapshot、recipe、tool.madb、host；build/run再加入 tool.<安装记录逻辑名>（原12项）、sdk、resource.installation、resource.python、resource.compiler、tool.madb-source；run再加入run-host。tool.madb 与外部 tools 分开。源/IR不能建立时依赖行blocked，独立tool/宿主事实只在已受信配置/配方足够确定后收集。不得只依靠输出数组非空而省略内置行。

其中 structure 不要求编译/运行专属 SDK，与既有架构一致。这里是有限工程配方的静态环境前提，不能取代 I03资源/异步首链、其他平台、IDE或任何 Lab suite。首次匹配未知 profile/SDK 为 unsupported/2；通用可解析不等于本recipe支持。

相关初始失败不妨碍读取同一已受信固定记录下的独立组，按固定顺序报告。例如 rustc 缺失、Ninja摘要错、SDK文件缺失可以同时输出三行；文件错误行不伪造 observed tool。配置不能建立时，后续安装检查 blocked，禁止继续顺着任意用户路径探测。

## 5. 成功与错误分类

退出 0 **只表示请求级别的全部静态前提已核对**：tool/request/context齐全，recipe识别，独立必需集合非空且全部passed，所有已读输入/工具/资源在返回前完成规定重验。scope固定preflight-only。不能把0显示成 build/run/IDE/Candidate/Lab passed。

MetadataRequired 的首片精确映射与现有命令保持：

| 到达的真实分支 | 行状态与错误种类 | 后续/总体 |
|---|---|---|
| 声明输入缺失或字节陈旧，现query产生META001 | context.declared-inputs = failed / invalid-input | snapshot blocked_by declared-inputs；configuration blocked_by snapshot；退出2（若另有更高优先实错则按汇总） |
| 声明输入匹配，但snapshot路径缺失，现query产生META001 | context.snapshot = failed / invalid-input，observed=state/missing | configuration blocked_by snapshot；退出2 |
| snapshot检查返回现query转换MetadataRequired的LOCK005或PORT002 | context.snapshot = failed / invalid-input，保留原code | configuration blocked_by snapshot；退出2 |
| IR存在但格式/其他交叉检查失败 | context.snapshot按真实QueryError分类，不能统称MetadataRequired | configuration blocked；实际退出类保持 |
| prepare已失败（如LOCK002） | context.configuration按真实QueryError失败，Context=null | 依赖检查blocked，不能探测未受检配置提供的工具路径 |

`SnapshotAdapter.prepare` 不单独产生 configuration passed。正常顺序是局部prepare→declared-inputs→snapshot完整检查→configuration完整通过→recipe→对应环境组；失败早于prepare时以configuration错误为根。由此形成真实无环blocked_by图，不能制造configuration↔snapshot互相依赖。配置完整性尚未成立时recipe及依赖环境组blocked；仅已独立建立的当前main工具身份可报告，不用局部profile匹配偷开其余检查。即使schema/IR能解码，语义/原文关系未检查也不输出passed。

错误种类沿现有映射：CheckFailed=1、InvalidInput=2、Host=3、Conflict=4。初始缺/错受检文档按当前对应适配器分类；初始工具/SDK缺文件保留Host，错已声明版本/摘要/host或未知recipe保留InvalidInput；冻结后漂移映射Conflict。不能为了所有“缺项”同一退出值而改既有错误种类。

多组失败首次引入汇总，需要本次明确采用：只收集真正发生的错误，不按最后读取的标志重分类；总体优先级 Conflict(4) > Host(3) > InvalidInput(2) > CheckFailed(1)，每行保留自身诊断。该优先级只用于doctor最终摘要，不改变 init/C04 的先发生结果准入语义。

JSON与例外精确定义：启动后的分发器确认第一个子命令是doctor，且在首个独立`--`之前出现独立`--json`参数，就选择机器模式。帮助/版本请求仍按Clap正常文本输出0，是显式例外；程序尚未进入分发器的OS装载失败、外部终止/OOM/进程崩溃及stdout不可写，不承诺完整JSON。除此之外，进入doctor机器分发后，即使Clap解析、schema、身份捕获或文件读取失败，也必须输出一份schema有效错误JSON；不“尽量”，不先把人读错误写stdout。语法错误为2、request/context=null；工具尚未捕获时tool=null。

输出预算规则：正常结果先在有限内存中完整编码，算入末尾换行，至多1MiB才一次写stdout。若超限，新增真实Host/DOCTOR001错误，设truncated=true，并构造固定小摘要：保留已取得的tool/request/context（其字段独立有界，总和低于64KiB）；checks仅保留此前最高优先级错误的一行，保留其id/kind/status/error_kind/subject和首条原诊断，expected/observed均置null且blocked_by为空；原始消息按UTF-8字符边界缩到1024字符并标明省略，不改变code。没有已有错误行时checks为空。顶层diagnostics加入DOCTOR001；recovery只保留一条manual说明。摘要不再描述完整集合，绝不返回0。

总体退出继续取本次真实错误集合最大优先级：已有Conflict4仍是4；原Host3仍是3；只有InvalidInput2/CheckFailed1或原本全通过时，由新增输出预算Host错误变3。不把所有超限无条件改3。正常结果的truncated=false；缩减摘要的truncated=true且exit>=3，Context/checked行的存在仍不授予运行或提交资格。若固定摘要编码本身内部失败或stdout写失败，stderr可报告本机输出故障并退出3；这是无法交付机器结果的I/O/内部异常例外，不能声称成功或丢失了原错误报告仍算完整结果。

普通信号退出沿现有CLI约定，不新增取消或子进程生命周期协议。

## 6. 只读与信任边界

- app端口只持 ReadWorkspace 与独立环境读取能力；不提供 CommitNew、ProcessRunner、cache write、下载、IDE启动、修锁接口。CLI唯一组合根不将execute路径交给doctor。
- Host复用descriptor/普通文件/链接规则及现有有限资源预算；实际系统查询使用OS API/受控系统信息读取，不执行安装记录中的任意命令。
- 保留 `InstalledValuesTools` 只能由完整成功构造的执行凭据语义。doctor的部分观察是另一个纯事实值；不得返回部分InstalledValuesTools，也不能被init接受为已验证安装许可。
- 配置/工具/资源初次观察与返回前重验分别记录错误。未读取文件不产生摘要；同路径替换/别名/链接变化准确拒绝。此为受信本地FS上的冲突检测，不声称对同权限恶意写者的原子快照或执行sandbox。
- 不创建证据目录/日志/锁/缓存；只写stdout/stderr，用户可自行重定向。JSON里引用输入根，故搬动结果文件不改变其引用基准。

## 7. 验收向量与文件边界

首片正例：有效精确上下文structure；同上下文build静态前提；run只增加run-host并保持preflight-only；PATH为空/污染仍读取同一pin；中文空格工程根；snapshot存在/原bytes重验。

必须反例：未知参数/等级；路径穿越/链接；错project/lock/profile/request/IR关系；未知recipe/profile2/旧SDK组合；缺snapshot和源变化准确MetadataRequired；缺rustc、错Ninja bytes、缺SDK三错误汇总；初始错version/digest/host；读取后镜像/SDK/目录链接漂移Conflict；无读权限Host；自镜像无法可靠绑定tool=null；资源记录存在但实物缺失；只structure不探测构建工具；run不产生run证据；错误JSON字段/重复键/非法数值/超预算；stdout失败非零；所有成功与错误路径目录树不变、无子进程/网络/缓存写入；错误行不能伪造observed tool版本。用可控端口证明独立组继续检查、依赖失败blocked、部分观察不能转成执行许可。

批准后建议独占落点：

- CLI：`crates/madb-cli/src/doctor.rs`、`bootstrap/doctor.rs`；shared command/module注册由主协调单写。
- app：`crates/madb-app/src/use_cases/doctor/**`，只读请求/事实/汇总；尽量复用query捕获逻辑，不复制锁规则。
- contracts：`adapters/contracts/src/doctor/**`、`query.rs`最窄受检投影复用；不导出wire Document给app。
- Host：`adapters/host/src/initialization_execution/installation.rs`中不改变verify语义的复用抽取；单独doctor observation模块，源镜像捕获沿initialization_source。冻结安装JSON/工具实物不变。
- protocol：只在获批后注册新输出schema/正反向量，旧schema不改；对应定向测试归各模块。最终共享导出、文档/catalog/root集成由主协调。

本轮没有实施这些代码或运行目标、编译；提案获批与独审完成后才能准入新增公共decoder/CLI结果。

## 8. 真正需裁定的公开差异

建议一次裁定：①先采用现有冻结上下文输入的doctor首片及明确未交付的template模式；②采用以上CLI参数与三档仅静态前提含义；③采用独立 `madb.doctor-result/1` 输出（复用共同类型、无新输入wire），、仅输出预算DOCTOR001新稳定码及精确JSON保证/例外、多失败汇总优先级。归属、默认只读、不隐式安装/构建、拒绝错误锁和未运行不通过早已批准，不重复询问。
