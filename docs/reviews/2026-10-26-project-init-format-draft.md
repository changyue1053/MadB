# I01 / I02 最小格式审查草案

本文是供主协调入口审查的具体建议，未写入任何 schema、catalog 或产品源码，不是采用决定、第二份状态表或验收报告。任务为 I01 模板/结果格式收敛；I02 的 doctor 归属、默认只读、输出及拒绝条件已获项目所有者确认，不再次申请同一批准。

## 1 输入与实际差异

读取位置：`/private/tmp/madb-chief-round2/madb`；HEAD 为 `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`，工作副本含接管首批未提交实现和目录修订，不以 HEAD 代替输入。已读 AGENTS、共同方法、I01/I02 当前目录、architecture/14、validation/06、schemas/README、common/project/profile/lock/execution/report/extension-manifest，及首批责任决定和现行 `ProjectPlan`。

| 现有依据/实现 | 具体缺口 | 本草案的最小处理 |
|---|---|---|
| architecture/14 已要求 `madb.project-template/1`、模块/文件/IDE/任务/验证清单 | schemas 中尚无此格式；普通 `madb.project/1` 是边界工程输入，不应塞入 IDE 和模板字段 | 新增独立模板格式，不改 ContractIR 或既有 project 字段 |
| `ProjectPlan` 已有 `ProjectModule`、`ProjectFile`、四类 `FileOwnership` 和拓扑/路径检查 | 尚无模板身份、参数、profile 选择、IDE/检查要求、真实文件提交 | adapter 显式转换；现有纯计划只承担其已实现断言，不称为完整模板计划 |
| `madb.report/1` 要求 candidate/Lab/案例身份 | init 和 doctor 在没有候选时也必须产生失败结果；伪造候选或 Lab revision 会污染验收证据 | 新增 `madb.init-result/1`，复用检查状态、诊断和 FileRef 的含义，不能冒充 Lab Report |
| `madb.execution/1` / BuildTask 执行前要求真实、固定的输入/输出身份 | 模板尚未渲染时无法知道所有生成物摘要，不能把带占位符的 execution 当可执行任务 | 模板只引用有限、受检的工程配方及其根；配方利用现有 prepare/冻结/TaskGraph/执行入口产生真正任务 |
| 现行产品规范已要求 `--json`、默认 verify=build、dry-run/offline 和失败保留工程要求 | 具体字段、默认展开、退出结果尚未固定 | 下文明确字段与条件，不增加新的命令、平台或默认网络行为 |

I01/I02 以及相关文件的实现状态仍只以产品 catalog 为准。这里没有执行新模板、Wrapper、IDE 或设备验证。

## 2 共同格式规则

建议继续使用 Draft 2020-12、严格 JSON、1 MiB 文档/64 层深度、数组最多 4096 项及现行安全整数规则；对象一律 `additionalProperties:false`。不允许未知字段静默忽略。集合唯一性及跨文档引用在 adapter/纯规则层检查，不能只靠 `uniqueItems` 证明身份唯一。

复用 `common` 的 `id`、`path`、`digest`、`file`、`file_ref`；不重建另一份 FileRef。所有 FileRef 的 sha256/size 是实际文件字节身份。MadB/template 的有限版本语法、组件比较及预发布向量须在决定中明确；现有扩展版本 regex 不等于完整 SemVer 比较。OS/IDE 由对应 adapter 的明确版本规则及已验证组合处理，无法比较的版本拒绝；不承诺跨厂商统一区间求解。

下文“必需”指字段必须存在；允许空数组或 null 的位置明确列出。没有含义的默认值不由解码器补成成功。省略仅可用于 `extensions`；若保留该字段，仅允许现有非语义元数据，不影响选择、执行或结果判定。

## 3 `madb.project-template/1`

### 3.1 顶层字段

| 字段 | 建议类型/必需性 | 精确规则 |
|---|---|---|
| `schema` | 必需，常量 `madb.project-template/1` | 与扩展协议/ContractIR 分别版本化 |
| `id` | 必需，Id | 模板身份，不从目录名推导 |
| `version` | 必需，版本字符串 | 同 id/version 不得发布不同内容；包摘要由解析输入/锁提供，不嵌入包内容自引用 |
| `madb_compatibility` | 必需，`{min_inclusive, max_exclusive}` | 有界版本区间；两个端点都必须存在且前者小于后者；预发布版本按版本组件比较，不以 branch/latest 表达能力 |
| `default_profile` | 必需，Id | 精确引用本清单的一个 profile，消除“数组第一项就是默认” |
| `parameters` | 必需，参数数组，可空 | 参数身份唯一，未知调用方参数拒绝；默认展开后再校验 |
| `profiles` | 必需，非空数组 | 有限显式组合，不生成目标/语言/扩展笛卡尔积 |
| `modules` | 必需，非空模块数组 | 身份唯一；包括模板可能选用的模块，按 profile 选出闭包后验证 |
| `boundary_refs` | 必需，数组，可空 | 仅引用现有 boundary profile，不重复定义类型/所有权/线程 |
| `assets` | 必需，资产数组，可空 | 唯一输入资产身份、输出文件 owner；空资产不表示模板已可用 |
| `ide_entries` | 必需，数组，可空 | 打开入口与运行目标分开；空数组只能用于未宣称 IDE 支持的格式/开发例 |
| `task_roots` | 必需，数组，可空 | 引用明确受检工程配方；不能含 argv、shell、postCreate 或自定义进程操作 |
| `validation` | 必需，非空检查数组 | 声明逐目标、逐级别检查；本机运行结果与模板认证引用分开 |

模板 decoder 成功只表示格式有效。初始化可执行性还要求实际资产/锁/配方/宿主都可解析，所有所需子集支持。没有推荐资格字段；推荐目录仍由实际认证证据决定。

### 3.2 参数

每项必需 `id`、`type`、`required`、`default`；`type` 取 `string|boolean|integer`。`default:null` 表示无默认，不是一个可传入的空值。`required` 表示默认展开后的必填，允许已验证的非 null 默认；应用先按现行 init 规范展开 `--name` 的输出目录 basename 默认和 `--namespace` 的模板默认，再检查必需性，不新增“必须显式输入”约定或表达式求值器。

- string：另必需 `min_length`、`max_length`、`pattern`、`choices`；`pattern:null` 表示无附加字符模式，`choices:[]` 表示无有限枚举限制。模式只使用固定非回溯 regex 子集；不支持的模式在加载模板时拒绝。名称/命名空间的实际合法字符由选定语言配方进一步检查，不能只靠通用 Id。
- integer：另必需 `minimum`、`maximum`；范围在现行安全整数区间内，默认值/请求值均须满足边界。
- boolean：无额外字段，默认只允许 bool 或 null。
- 非 null 默认必须符合同项类型/限制；`minimum>maximum`、`min_length>max_length`、默认不在 choices 等模板自身矛盾在 Preflight 之前拒绝。

参数不能直接变成 shell 片段、工具路径、文件输出路径或任意环境变量。v1 的模块/资产路径固定；输出工程名放在调用方最终根目录，不通过插值改写 package 内路径。需要随参数改变文件集合时由经审查的语言配方生成明确文件计划，再做同样的所有权检查，不能绕过资产计划拼路径。

### 3.3 模块、边界与 profile

`modules[] = {id, role, directory, dependencies}`，字段全必需。`role` 为受检开放 Id，具体配方必须认识；`directory` 为 `Path|null`，null 唯一表示工程根，不允许 `.`。`dependencies` 为 Id 集合。选中模块依赖必须全部在所选组合中；禁止缺边、重复依赖、自环和多点环。目录相同或目录前缀大小写别名拒绝；嵌套模块目录允许，但每个文件只有一个显式 owner。

`boundary_refs[] = {id, profile}`，`profile` 为 `FileRef`，引用 MadB profile 格式；只接受当前 decoder 明确支持的版本，首片仅 `/1`，`/2` 随其格式决定及共同向量接入，未知版本拒绝，不做通配版本兼容。其 route/consumer/provider/direction/target/工具身份使用被引用文件的既有规则；模板不在旁边复制一份可漂移字段。工具/SDK 解析仍使用已锁版本，不因模板选择而偷偷升级。

`profiles[]` 每项必需以下字段：

| 字段 | 类型与规则 |
|---|---|
| `id` | 唯一 Id |
| `targets` | 非空 `{id,family,boundary_refs}` 数组：id 标识本 profile 的精确目标项，family 对应用户 `--targets` 的平台族，boundary_refs 引用确定目标/ABI/SDK 的边界配置；每项 id 唯一，不从 `ios` 自动推出 device/simulator 两种配置 |
| `modules` / `boundary_refs` / `assets` / `ide_entries` / `task_roots` / `validation` | 各自引用的 Id 集合；除 modules/validation 外可空，不能引用缺失对象；文件与模块依赖闭包完整 |
| `requirements` | 对象，四个必需键 `structure/build/run/open`，值为下述宿主前提对象；不请求的级别不要求其专用工具，但依赖级别仍需满足 |

宿主前提对象为 `{hosts, tools, sdks, ide_entries}`，四字段均为数组：`hosts[] = {os, arch, min_inclusive, max_exclusive}` 表示允许的宿主备选（OR），其他三类为必须同时满足（AND）。`tools` 使用 common.component 固定工具 id/version/image 摘要；实际来源/定位由当前精确锁和宿主解析输入给出。`sdks` 为 FileRef（对应实际 SDK 清单字节，不把 sysroot 当正式应用 SDK）；`ide_entries` 为清单中的 IDE 入口 Id。hosts 空数组表示无额外宿主限制，但不豁免 renderer/受控执行器本身的平台支持。

无 `--profile` 且未传 targets 时选 `default_profile`；传了 targets 时按 family 集合匹配公开配置，精确目标/ABI/SDK 必须在解析结果中展示。多种精确配置的 family 集合相同时，若 default_profile 是其中之一则使用这个明示默认，否则要求指定 profile，不能默默选第一个。显式 profile 的 family 集合与请求不一致拒绝。若要允许某个 profile 的多个子组合，在清单中列出多个明确 profile；首版不采用隐藏自动裁剪规则。此规则是本草案需审查的用户可见选择细化，尚未采用。

### 3.4 资产与 renderer

`assets[] = {id, module, source, output, ownership, renderer}`，全必需：

- `module` 精确引用 owner；`output` 是工程根相对 Path，必须位于 owner 的目录内（根 owner 除外）。输出集合统一交给 ProjectPlan 检查；重复归一化路径、父目录大小写别名及 file/directory 碰撞均拒绝。
- `ownership` 对应已采用的 `user|managed|local-state|provenance`。user 文件仅 init 创建一次；managed 文件仅由声明 owner 的生成事务更新；provenance/锁更新必须显式；四者都不是绕过当前文件基准核对的授权。
- 非 local-state 项的 `source` 为 common.file，必须属于模板包清单、摘要/大小匹配且实际为普通文件；符号链接、硬链接、路径越界由 host 拒绝。`renderer` 为下述严格变体。
- local-state 只允许 `source:null, renderer:null`，表示预留本机路径，不携带作者机器 SDK 路径/签名内容，也不自动创建占位文件。之后实际写入由拥有该状态的宿主用例负责。

建议第一份可执行 schema/格式向量只提供两个明确 renderer 变体，避免把尚无实现的“通用模板语言”写成已支持：

1. `{id:"madb.render.copy@1"}`：逐字节复制，无参数替换；输入输出摘要相同。
2. `{id:"madb.render.json-values@1", bindings:[{parameter, pointer}]}`：source 必须是严格 JSON；pointer 为 RFC 6901 指针，只能替换已存在的标量或 null 节点，不能改 key/追加数组/创建路径；同一节点重复绑定拒绝。参数作为已验证 JSON 值写入，最后使用既有规范 JSON 编码，不能把字符串作为原始 JSON 拼接。若输出声明自己是 MadB 文档，还须用相应已有 schema 再验。

Kotlin/XML/CMake/Swift 等具体源码参数渲染需其专用 renderer 的精确语法、转义和负例；copy 不能在复制后再偷偷全局替换。新增 renderer 是必需执行语义，旧工具明确拒绝。首版只交付这两个 renderer 不等于 I01 所有平台配方、语言模板与 IDE 探针都完成，也不能据此删除已承诺的后续 renderer/配方工作。

当前 ProjectPlan 仅接受 ASCII 逻辑资产路径，host managed path 也有 ASCII/最多32组件限制。建议把这个事实写成首片的显式限制与准确预检；**不要**把它偷改成整个产品永远不支持中文路径。最终输出根、工具安装路径及已承诺 INIT-10 的空格/中文移动场景仍需平台实测。

### 3.5 IDE、任务引用及验证

`ide_entries[] = {id, module, ide, path, run_target, extensions}`，全必需。`ide={id,min_inclusive,max_exclusive}`；`path` 为相对工程路径，可以指向清单中的文件或已计划目录；`run_target:Id|null`；`extensions` 为 `{id,version}` 的精确列表。工具不执行清单给出的任意启动命令，已注册的 IDE adapter 解释其 id；未知 IDE 或版本范围拒绝。打开成功只证明 launch。

`task_roots[] = {id, module, recipe, boundary_refs, targets}`，全必需。`recipe` 为**宿主已登记、版本化、有限集合中的工程配方 Id**，不是外部进程命令、不是新的 E02 操作、也不是任意依赖求解器。示意身份可为 `example.init.rust-c-sdk@1`，但这不是现行已实现的配方。

配方的输入是已解析的模块/参数、精确 boundary profiles/工具锁和实际源码；输出仍是现有 prepare 结果、固定 TaskGraph/执行配方及必须执行的根。模板的 task root id 精确绑定到该结果，发现不存在、目标不匹配或递归调用当前应用根时拒绝。未来产物摘要只在真实准备后绑定，模板里不得使用零摘要或假 execution 占位通过冻结检查。此配方引用是现有设计“受检任务描述”的具体候选表示；需格式审查后才能成为公共字段。初始只允许随 MadB 一起审查的配方，第三方自定义过程式工程生成仍按既有协议演进处理。

`validation[] = {id, level, target, task_root, required, minimum_evidence, prerequisites}`，全必需：

- `level` 取 `structure|build|run|import|debug`；target 为精确 Id；task_root 为 Id 或 null，纯结构/独立 IDE 人工检查可为 null，但不能据此伪造通过。
- `required` 为 bool，推荐模板的必需检查由 INIT-01..12 和平台承诺决定，模板不能自行把应必需的项改成 optional。
- `minimum_evidence` 使用既有层级 `static|generated|compiled|linked|host-run|target-run|device-run`；IDE import/debug 还必须有对应工具/人工检查附件，不能仅靠层级枚举认定通过。
- prerequisites 为本清单检查 Id 集合，无环；失败下游为 blocked。`--verify build` 包含本次适用 structure/build 项，run 再包含 run；import/debug 沿独立认证或显式执行记录，不因本次未请求而填 passed。

## 4 `madb.init-result/1`

### 4.1 顶层与身份

建议下列字段全必需；JSON 输出及本地保存使用同一格式。结果是本机一次请求观察，不是模板认证、候选认证或发布资格。

| 字段 | 类型/具体规则 |
|---|---|
| `schema` | 常量 `madb.init-result/1` |
| `request_digest` | Digest 或 null；合法请求按下面定义绑定，无法形成合法请求投影时为 null 且必须 InvalidInput，不能用零摘要；不复用 project/candidate 摘要 |
| `tool` | common.component，当前 MadB 工具身份；实际 image 身份缺失时不得冒用其它组件摘要 |
| `template` | `{id,version,manifest,package_sha256}` 或 null；manifest 为 FileRef/schema=project-template；解析前失败可为 null，不能填零身份 |
| `template_profile` | Id 或 null；未解析/无匹配时 null |
| `boundary_profiles` | FileRef 数组，可空；成功解析的实际引用，不从路线名猜 |
| `request` | `{targets,verify,open,dry_run,offline}` 或 null；targets 是默认展开后的平台族集合，verify 为 structure/build/run，open 为 IDE Id 或 null；请求非法而无法形成该值时 null，不能伪造默认成功请求 |
| `output` | `{path,redacted}` 或 null；本地 path 为实际最终绝对路径，脱敏导出可用稳定标签；输出无法合法解析时 null；详见下文 |
| `plan` | `{files,task_roots,downloads}` 或 null；解析成功后给出文件 owner/路径/所有权投影、将调用的根 Id 和明确下载项，失败时 null；仅计划，不代表执行 |
| `creation` | `not-created\|created`；只在无覆盖源码提交实际完成后记 created，不以渲染完成代替 |
| `termination` | `completed\|failed\|cancelled`；取消不改写已经真实执行过的检查结果，但不能汇总成功 |
| `exit_code` | 0..255；建议延用现 CLI 的 0 成功、1 CheckFailed、2 InvalidInput、3 Host、4 Conflict；取消作为明确诊断及 termination=cancelled，暂沿用失败码1，不擅增130约定 |
| `checks` | 下述逐目标结果数组，允许空（例如参数解析前失败）；成功请求所需检查不得为空/缺项 |
| `ide_entries` | 已解析的 IDE 入口清单；路径相对最终工程根，没有解析成功则空 |
| `open` | `{requested, status, diagnostics, attachments}`；requested 为 Id/null，status 取六态；未请求固定 skipped，无 passed 推论到 import/debug |
| `recovery` | 恢复步骤数组，可空；格式见下文；不自动执行 |
| `diagnostics` | 复用 Report 的 `{code,message,location}` 结构；location 可 null，保持稳定错误分类和来源，不从输出文案推断类别 |
| `logs` | common.file 数组，可空；相对本地报告目录，摘要为真实日志字节；未取得日志不伪造引用 |

`plan.files[]` 精确为 `{module,path,ownership}`，复用 ProjectPlan 的受检结果；`task_roots` 为本次选择的根 Id 集合；`downloads[] = {id,version,source,sha256}` 只列已有可信元数据能确定的下载，不解析 live latest。任何必需下载身份未确定都应报告缺项，不给它补零摘要。

`checks[] = {id, level, target, status, requirement, prerequisites, environment, command, evidence_level, cleanup, diagnostics, attachments, reason}`；字段均必需，含义如下：

- id/level/target 对应请求选中清单的检查与精确目标项（profile.targets[].id），环境/附件进一步固定其实际目标/ABI/SDK；requirement 取 `required|optional|not-applicable`，不适用必须 reason，不能因缺 SDK/失败而改成不适用。
- status 严格使用既有 `passed|failed|error|skipped|unsupported|blocked`。未执行在 reason 说明原因；请求必需项为 skipped/unsupported/blocked/error 时整体非零。
- environment 为实际观察的工具/SDK/宿主 `{id,version,sha256,host,target,source,arguments}`，复用已有环境条目。无法取得某项值时用诊断列出缺项，不补造通过环境；检查可失败且环境数组为空。
- command 为实际 argv 数组或 null；实际执行才有 argv，不把建议重试命令当已执行。日志按既有敏感信息处理要求脱敏。不能执行的建议命令放 recovery。
- evidence_level 使用既有层级或 null；未执行必须 null，实际结构检查可为 static。cleanup 使用 `complete|failed|not-run`；有进程/暂存清理责任且未完整结束时不得通过。
- diagnostics、attachments 使用既有结构；reason 为字符串或 null。附件必须存在、大小/摘要匹配且属于本次报告范围。

输出对象的本地绝对路径与包内 LogicalPath 是不同类型，不能把绝对路径塞进 common.path。脱敏导出建议 `output={path:"<project-root>",redacted:true}`，所有可恢复精确本地路径留在本地报告；导出不变更 request_digest 所绑定的原请求身份，但必须标注 redacted=true，避免它被解释为可直接重放的完整请求。

`recovery[] = {kind,message,arguments}`：kind 为 `retry|manual|inspect`；arguments 为 argv 数组或 null。retry 仅在 created 后引用项目 Wrapper 的具体受支持任务（如 build --frozen），不建议再次 init 同一目录。manual 用于 SDK/许可/签名等本轮不会自动修改的前提；不得把字符串拼成待自动执行 shell。若不能提供可执行的真实入口，arguments=null 并说明所缺能力，不能写不存在的命令。

### 4.2 请求身份与汇总规则

不再新建持久化 InitRequest wire 作为本批前置。定义请求摘要的规范投影：

`SHA256(UTF8("madb.init-request/1\n") || JCS({template_selector, template_profile_selector, parameter_values, targets, verify, open, dry_run, offline, output_path}))`。

selector 保存用户明确输入，未指定处为 null；parameter_values 保存默认展开后的参数；targets 按 Id 排序去重后参与投影，但重复请求值在校验阶段拒绝；output_path 是本次词法绝对输出路径，不能先解析一个尚不存在的最终目录。所有影响行为的默认展开必须进入投影。模板实际解析身份另由 result.template 固定。未知请求字段导致错误时仍可对收到的受限原始请求字节保存本地日志，但不能伪造一个正常成功请求摘要；若根本无法形成合法投影，request_digest/request 为 null 并返回 InvalidInput。

结果验证器还必须接收冻结的内部请求投影、选定模板和受检计划，重算请求身份，核对适用检查集合与真实观察；单凭 InitResult 自报不足以验证完整性。无需另加持久化 InitRequest wire；脱敏结果仅保留关联身份，不承诺单文件重放。以下关系须由结果验证器检查，不能依赖调用方自报：

1. 普通请求 exit_code=0 仅在 termination=completed、请求级别所有必需项 passed、所请求 open 成功、清理无缺口时成立。dry-run 是独立的只读计划操作：可在计划及所需本地前提解析成功时返回0，必须 creation=not-created、plan非null、checks为空且open为skipped；模板计划中的待执行检查仍在plan引用/模板中，不填入任何本次已执行的build/run/structure结果。dry-run与open同时给出时只规划open，绝不启动IDE，open.diagnostics说明未执行。
2. creation=created 表示完整源树已提交；后续构建/运行/open 失败保持 created 和非零退出，不撤销或删除用户工程。
3. 取消发生在提交前则 not-created，并留暂存清理观察；发生在提交后仍为 created，保留恢复信息。未停止的外部工作/清理缺口必须保留，不能将返回取消等同于全部资源已经消失。
4. 同一 `id+target` 检查不得重复；passed 必须满足本次要求的层级、实际命令/结构观察及必要附件。compile/link 不满足 run；open 不满足 import/debug。
5. 本机观察与既有模板 Lab 认证可用附件相互引用，但不可把某一份历史认证的 checks 复制成本次已执行结果。

doctor 复用这些小型检查/诊断的定义与纯汇总逻辑，但不返回 `InitResult`、不伪造 creation/template 或候选身份。doctor 的独立结果 envelope 在 I02 具体实现前另作最小字段审查；其职责、只读行为和拒绝条件已经批准，不受该格式拆分影响。

## 5 旧参与者、摘要与证据失效

- 旧 SchemaRegistry 不认识 project-template/init-result 时必须明确拒绝；不把新字段塞进 `extensions`，不自动降成 `madb.project/1` 或 `madb.report/1`。
- 模板身份、所选 profile、参数、全部资产源字节、renderer/配方实现及工具锁都进入实际生成任务/候选身份。仅模板版本号相同不允许复用旧结果。
- 使用原始模板文件 FileRef 摘要已能固定字节，本批无需再创造模板语义摘要域；新增 `madb.init-request/1` 仅用于请求，不改变任何已通过的 project/profile/candidate 摘要向量。
- 增加 renderer、role/recipe 语义、参数默认或所有权规则时按未知必需能力拒绝和版本规则处理，不能让旧工具“尽力生成”。公共选择/默认/关闭语义变化照既有 ADR 流程。
- 不变更 C04/C06/C07 历史验收；新模板只有自己的候选/环境/INIT 证据齐全才可推荐。默认/工具/模板变化使受影响初始化及消费证据失效，不能继承旧 SDK 路线通过直接认证工程。

## 6 应加入的共同格式与真实验收例

| 例 | 输入与准确结果 | 层次/责任 |
|---|---|---|
| 合法结构计划 | 根模块+库模块+应用模块，显式依赖，user/managed/provenance 各有唯一文件，选中闭包完整 | schema→adapter→ProjectPlan；确定性重排结果一致 |
| 默认与目标 | 省略 profile 取明确 default；传匹配目标集合成功；无匹配/多匹配/显式profile矛盾拒绝 | I01 选择正反例，不能按第一个匹配放行 |
| 错身份/版本 | 重复模板/模块/资产/检查 Id、越界版本区间、未知schema/字段/renderer/recipe | 明确格式或能力诊断，无文件写入 |
| 参数注入 | 名称包含引号、换行、`$()`、反斜线；JSON值安全编码；非法语言标识准确拒绝；字符串不能成为命令 | renderer真实输出解析+语言编译；无进程副作用 |
| 路径/owner | `../x`、绝对路径、大小写父目录别名、file/directory碰撞、缺owner、跨模块输出、重复改同一JSON节点 | schema/纯计划拒绝；现有9项计划测试可复用但不代替新schema |
| 真实文件边界 | source摘要错、源符号链接、输出祖先符号链接、并发创建最终目录 | host测试；不能靠路径字符串测试代替 |
| 任务引用 | 缺task root、未知配方、错目标、Gradle→MadB→同一应用根、占位输入摘要 | adapter/TaskGraph/冻结校验准确拒绝；不启动任务或不提交部分结果 |
| 结果分层 | structure通过且build/run未执行；build失败但created；open失败不推翻已建工程；run不满足必需证据 | 新结果规则测试，exit非零/恢复信息准确 |
| 失败和取消 | 提交前/后取消、验证失败、清理未完成、原锁仍可用 | I02真实无覆盖提交/取消测试，保持用户文件 |
| dry-run/offline | 离线有可信缓存/缺包；dry-run未下载、未建目录、未编译；missing运行环境不降级为成功 | I02进程/文件观察，不用mock调用次数充当全部证明 |
| doctor | 精确工具/SDK/锁观察；未知配方、错版本/摘要/宿主、缺必需工具明确失败 | 已获批I02入口验收，默认无安装/构建 |
| 真工程 | Rust/C S1模板随后按INIT完整验证；CLI/GUI Wrapper同锁；改导出后IDE正常重建 | I03及对应I任务/Lab；静态格式通过不替代 |

本草案未创建“通过”的 JSON 向量，因为 schema 尚未采用；实施时应把正反例加入现有 tests/fixtures/contracts 的共同向量和真实 schema decoder 测试，不另建任务/证据系统。

## 7 真正需格式/产品决定的影响与推荐

| 差异 | 影响 | 推荐与决定边界 |
|---|---|---|
| 两个新 schema 及新请求摘要域 | 模板作者、CLI JSON消费者和IDE向导需要认识新版本；旧工具明确拒绝 | 按现有格式流程集中审查本字段草案；不复用含义不同的旧格式；不要求重批I01/I02开工 |
| profile按精确目标集合选择，拒绝隐式裁剪 | 用户不能传任意子集期待自动拼装；需要显式有限组合 | 推荐采用有限组合，与既有架构一致；该具体用户可见规则和default字段在格式审查中确认，若改变现有预期则交项目所有者 |
| 首次可执行renderer仅copy/json-values | 不能因此宣称完整Kotlin/Swift/ArkTS参数模板已交付 | 推荐作为实现顺序，不删承诺、不改变I01整包出口；不需要重批已选语言能力。若要永久取消其他renderer/配方才属于产品范围决定 |
| 当前ASCII相对路径限制 | 部分资产名不能进入首片，但不等于所有中文输出目录不可用 | 推荐保留真实缺口与INIT-10测试，不能将实现限制升级为永久产品承诺；若拟降低既有路径承诺须项目所有者决定 |
| 本地完整路径和脱敏JSON结果 | JSON消费者需识别redacted；脱敏件不能直接重放本地操作 | 推荐显式redacted，保留本地恢复信息；不在普通共享报告泄露本机路径 |
| 取消的退出结果 | 已有CLI只有0/1/2/3/4分类，新增130将形成新脚本约定 | 推荐暂沿用1并用termination/诊断明确取消，现有退出分类不改；实际映射在格式实现前审查 |

doctor归I02、verify归C09、lock --update归C10、explain归L02，以及既定输出/拒绝条件均已批准；这里没有重新提出归属问题。L04/C08/L06责任决定也不受本草案影响。
