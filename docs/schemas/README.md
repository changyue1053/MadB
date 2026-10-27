# MadB 对外格式与 C02 契约实现

本目录是对外文件字段、版本和结构约束的唯一权威，采用 JSON Schema Draft 2020-12。宿主与帮助程序共享 schema 和格式向量；纯领域模型不导入 wire 类型。C02 提供离线校验库和受检转换，C04 通过执行配方装配这些真实端口。

## 格式清单

| schema | 输入或输出 | schema 之外的检查 |
|---|---|---|
| [common](common.schema.json) | 身份、路径、引用、选择器、来源、策略与精确常量的共用定义 | 不单独作为业务文档 |
| [project](project.schema.json) | 工程、结构入口、完整选择器、策略族与策略绑定 | 唯一身份、精确命中、策略冲突、冻结解析 |
| [profile](profile.schema.json) | v1 运行时、目标、ABI、工具、能力与默认值 | 支持范围、工具一致性、后端配置验证器 |
| [profile-v2](profile-v2.schema.json) | 同一组既有字段及显式 composition 引用 | 新版本输入/责任检查；旧执行入口不自动接纳 |
| [composition](composition.schema.json) | 包源、组件、源/生成资产、依赖、责任和有限配方选择 | 真实字节、清单、锁命名空间、能力与唯一责任；生成后资产与执行另验 |
| [lock](lock.schema.json) | project/profile/工具/组件/输入/schema 锁 | 实际文件字节、过期输入、完整依赖发现 |
| [contract-ir](contract-ir.schema.json) | 源身份、类型、符号、有效策略、来源与布局观察 | 类型引用、能力、所有权、摘要、编译器布局身份 |
| [case](case.schema.json) | 案例模板或实例、步骤、独立预期、清理与超时 | 实例化目标、精确错误、实际执行 |
| [coverage](coverage.schema.json) | 按 profile/target/direction 的适用性要求 | 48 项无遗漏、排除理由、案例集合；不代表通过率 |
| [diagnostic-context](diagnostic-context.schema.json) | explain 的显式诊断与源引用、位置编码及报告精度 | 实际源字节与位置核对，不认证外来诊断来源 |
| [explain-result](explain-result.schema.json) | 只读解释结果或明确失败 | 稳定说明目录、0/2/3/4退出及只读边界；ADR-0020 |
| [project-template](project-template.schema.json) | 完整模板、库存、模块/目标、有限参数与 renderer | 原包字节/完整库存、跨引用、计划/配方及工具准入；ADR-0023 |
| [init-result](init-result.schema.json) | 初始化结果、原包/证据根及初始内容计划 | 冻结请求、真实同根 CommitNew、独立必需检查与证据保存事实；不能信任外来 created |
| [report](report.schema.json) | 固定候选的执行结果、清理、附件和资源观察 | 独立预期、六项身份、最低证据层级、资源计数 |
| [read-request](read-request.schema.json) | reader 的已锁输入和目标 | 输入与返回 IR 的对应关系 |
| [generate-request](generate-request.schema.json) | 固定快照与输入、任务身份 | 返回批次、源输入和 profile 一致性 |
| [output-set](output-set.schema.json) | 生成文件、来源与批次摘要 | 文件实际字节、完整清单、禁止自引用 |
| [build-task](build-task.schema.json) | 工具、参数数组、依赖、输入/输出与执行阶段 | 任务图、文件权限、真实执行；C04 编排 |
| [build-evidence](build-evidence.schema.json) | 实际阶段、目标、状态、退出码、文件与日志 | 错目标、混批次、把 compile 当 run、缺输出 |
| [package-request](package-request.schema.json) | 固定输出、构建证据、依赖与公开能力 | 已完成输入，不重新生成或编译；C04/C05 接入 |
| [candidate](candidate.schema.json) | 候选文件/依赖/符号/验证要求及身份 | 字节完整性、混批次、清单自引用、真实独立消费 |
| [verify-request](verify-request.schema.json) | 固定候选、Lab revision、覆盖要求与实例化案例 | 与报告身份一致；C05/C09 调用独立 Lab |
| [extension-manifest](extension-manifest.schema.json) | 既有扩展、组件、路线与执行要求 | 依赖/权限/能力协商由 E01–E03 完成 |
| [extension-wire](extension-wire.schema.json) | 既有 hello/invoke/cancel/result 控制帧 | 帧状态机与进程协议；不能替代以上业务 payload |

`common` 统一共享字段，其他 schema 通过固定 URN 引用。Rust `SchemaRegistry` 只装载内置 schema，禁用 HTTP/文件引用解析；输入不能诱导下载 schema。扩展帧通过独立会话适配器接入宿主；格式通过不替代握手、执行身份、文件闭包及终态检查。

## 编解码与版本

- 文档声明明确版本；现有 `madb.<kind>/1` 与新增 `madb.profile/2` 分别登记，未知版本或端口格式拒绝。未知字段拒绝，不能靠宽松解码绕过新语义。
- JSON 解析前限制 1 MiB，嵌套深度最多 64；schema 中数组通常最多 4096 项。重复键、孤立 surrogate、尾随 JSON 值拒绝。
- JSON number 只接受 `[-9007199254740991, 9007199254740991]` 的**整数字面量**。`1.0`、`1e0`、`-0` 也拒绝，防止解析器先将 `1.0000000000000001` 舍入为 1 再通过检查。这是 MadB 的输入子集，不是对完整 JCS 数值范围的重新定义。
- 精确整数常量使用 `{kind: integer, type: u64, decimal: "18446744073709551615"}`；类型决定宽度和符号。浮点使用 `{kind: float, type: f64, bits: "8000000000000000"}`，保存负零、NaN 与无穷位模式。十进制范围在纯规则中检查。
- 源语言限定名允许 Unicode，保持原值；公开工具 ID 使用受限字符集。逻辑路径用 `/`，拒绝绝对路径、`.`/`..`、反斜线和控制字符；真实文件的符号链接、别名与目录隔离仍由 C04 host 检查。
- `extensions` 仅保存命名空间下的字符串元数据，不承载必需行为。非空 `backend_config` 必须由登记的后端验证器处理；C02 未安装这些验证器，明确返回 CAP005，不忽略配置。
- [TOML 样本](../examples/madb.example.toml) 经 TOML 解码后使用同一 project schema。JSON 中允许未解析的 profile/target 为 null，但 `check_frozen_inputs` 拒绝用于执行；样本是格式示例，零摘要不是真实工具锁。

## 三个实现边界

```text
外部 JSON / TOML
    ↓ madb-protocol：严格解析 + 同一份 schema
Document：不可变、带 DocumentKind 的受检 wire 值
    ↓ madb-contracts：显式字段转换及跨文档对应检查
madb-model：Symbol / Type / Policy / Capability / CaseResult
    ↓ madb-rules：纯身份、策略、类型、证据规则
Findings 或受检结果 → C04 应用请求与端口
```

不同时手写各套 DTO 字段镜像。`Document` 封装 schema 验证后的值，私有转换器检查具体字段并构造领域类型；不能把任意 JSON 传进核心。Rust/Python 运行同一组 22 类文档格式正反例（原 16 类、composition/profile-v2、diagnostic-context/explain-result 及 project-template/init-result），领域规则另测，不用 schema 通过替代语义通过。Python 参考校验验证结构向量，timestamp 的 RFC 3339 format 校验由 Rust 主解码器承担。

应用请求与 wire 文件不是一个类型：C02 固定端口 payload；C04 按用例构造纯应用请求，并通过适配器转换。C02 没有空执行器、成功占位后端或伪造的 `build/verify` 命令。

## 身份、策略和诊断

source ID 使用 `source:v1:<sha256>`。输入为 UTF-8 的 `madb.source-id/1\n`，随后依次写入 reader、package、module、kind、qualified_name、overload；每段是 **8 字节大端无符号字节长度 + UTF-8 字节**。源码行号不参与。读取器按语言定义真正的重载判别；Rust/C 等无函数重载的声明使用 `none`，不能把普通参数/返回类型拼入该身份。签名变化由 shape 捕获。默认 public ID 为 `<sdk_namespace>.<source-id 的 64 个十六进制字符的摘要>`；显式 ID 可跨重命名保留，形状变化仍单独检查。

策略只按完整 public ID 或 source ID 绑定。展开选定策略族后绑定显式值，再补允许的 profile 默认；两个显式来源即使值相同也拒绝，两个族同时提供同一属性也拒绝。显式值替换默认时保存被替代来源。profile 只可默认复制值类型输入，不自动生成线程、停止、释放或业务成功语义。

项目策略只能声明 project-policy 来源；reader 注解走独立 source-annotation 输入通道。最终 IR 的每项属性都有且只有一项 provenance，`check_policy_resolution` 对照实际权威输入重算，拒绝“来源记录看似完整但值不一致”的快照。

| 诊断族 | 例子 |
|---|---|
| FMT000–006 | schema 装配、格式/版本、预算、重复键、数值精度 |
| DTO001–002 / HASH001–004 | 字段转换、领域值不合法、摘要及集合身份错误 |
| ID001–007 / POL001–011 | 身份冲突、选择器零命中、策略权威重复、错误默认与来源不一致 |
| TYPE001–008 / NUM001 | 未知/递归类型、复合资源不受支持、变体标签类型、整数越界 |
| CAP001–005 / SEM001–009 | profile/能力不匹配、启发式结构、缺少语义、错释放器、异步借用 |
| ABI001–005 / LOCK001–006 | 错布局目标/生产工具、缺布局、旧输入、未冻结目标和重复锁项 |
| FILE001–006 / PORT001–004 | 文件缺失/损坏、自引用、混批次、执行阶段冒充 |
| CASE001–002 / COV001–004 / EVD001–007 | 案例未实例化、覆盖缺项、旧证据、错预期、清理计数矛盾 |

Finding 带稳定 code、subject、message 和来源位置；格式错误带路径。语义错误不能仅输出“生成失败”。具体来源断言见[首链设计](../examples/first-route/README.md)。

## 摘要投影 v1

语义摘要统一为 `sha256:` 加 `SHA256(UTF8(domain + "\n") || JCS(projection))`。JCS 对象键按 UTF-16 排序，字符串不做 Unicode 规范化，数组按下表确定集合/顺序含义；编码遵循 [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785)。文件引用的 sha256 始终计算**实际文件字节**，不加域前缀，不用 JSON 重排后字节替代。

| 域 | 投影 | 排序与排除 |
|---|---|---|
| madb.shape/1 | 单个符号 shape、可达类型、对应 C 布局结果 | 类型/布局按 ID；参数、字段保持顺序；不含 source ID、位置与 provenance |
| madb.contract/1 | schema=madb.structure/1、namespace、boundary、公开 ID+shape、类型、必需功能、布局结果 | 公开 ID/类型/布局/功能按 ID；不含来源行、输入摘要及策略 |
| madb.policy/1 | 每个 symbol+slot 的有效 values | 按 symbol、slot kind、parameter 排序；不含 provenance |
| madb.profile/1 | 完整已解析 profile | 工具/宏/限制/功能按 ID；默认仅 values；排除 extensions 和默认来源 |
| madb.profile/2 | 既有字段按 v1 规则投影，composition 只保留 id 与 semantic_digest | 使用独立域；FileRef 原字节另验，不继承旧 profile 身份 |
| madb.composition/1 | 完整组合输入 | 包/组件/资产按 id、文件按 path、依赖与责任按规定键；重复/别名拒绝，args 保序 |
| madb.project/1 | 工程与选择、策略、策略族、制品声明 | boundary/族按 ID；策略按规范 JSON 字节排序、族引用排序；排除 origin/extensions |
| madb.source/1 | path、文件原始 sha256、size | 按逻辑路径；schema 标记不改变源字节身份 |
| madb.task/1 | BuildTask 或带 operation 的阶段投影 | 去掉自身 task_digest；输入/环境/依赖/输出排序；参数顺序保留 |
| madb.generation/1 | task/input/profile 摘要与文件清单 | 文件按路径、sources 按 ID；清单不能包含自身 |
| madb.candidate/1 | 完整候选、锁/契约/策略/生成身份及公开要求 | 去掉 candidate_digest；文件/依赖/证据按路径、符号/案例按 ID；禁止清单自引用 |
| madb.case/1 | 独立案例完整定义 | 问题/来源/功能/环境按 ID；setup/steps/assertions/cleanup 保持顺序 |

C04 增加 `execution` 配方，将真实请求引用、工具安装位置、显式参数/环境、输入、响应位置及受管目录绑定在一次操作中；metadata 不要求已有 IR。`dependency-request`、`dependency-manifest`、`cache-entry` 是单独校验的宿主辅助格式，不改变十五种业务 payload。

生成任务使用 `madb.task/1` 域下的 `{operation: "generate", request, execution}` 投影：request 去掉自身 task_digest，inputs 按 path 排序；execution 取 id、tool、arguments、environment、working_directory、response_path、inputs，环境按 name、输入按 path 排序。请求文件位置和安装路径不入投影，实际文件摘要和工具 image 仍逐项验证。metadata/package 使用对应 operation 与完整 request/相同执行投影；build-set 使用排序后的真实 BuildTask 集合。原 BuildTask 投影和旧向量保持不变。

依赖清单采用规范 JSON 字节身份；准备上下文、存储内容和物理批次各有内部版本域。物理目录身份可以变化，不能代替 wire generation_id 或 candidate_digest；内部格式不授予路线认证。

源码字节变化必须使 source/input 身份变化，即使仅移动声明行不改变 shape。改变参数顺序改变 shape/structure；改变策略改变 policy；工具版本和参数改变 profile/task，不能复用旧候选证据。布局观察的 selector/input/producer 必须对应快照与 profile；形状摘要只投影 subject/result，生产来源仍在完整 IR 文件及冻结输入中验证。

`DigestDomain::Catalog` 预留给后续固定案例目录的语义输入；C02 不使用它计算一种未定义的目录投影。目录文件本身可以使用实际字节 SHA-256。生成任务输入的传递闭包、任务 DAG 与缓存准入属于 C04，不由单个 hash 函数证明。

## 支持边界与证据

C02 支持标量、精确常量、字符串、标量 buffer、可空值、值 record/variant、直接资源与回调的声明。嵌套资源/回调和非标量 buffer 缺少逐字段所有权映射，v1 拒绝并要求显式适配；不会把它们默认复制。C record 必须提供匹配输入、工具和 profile 的 compiler-derived 布局观察；这类观察要由 C03 的真实探针取得，测试构造的观察只验证检查器。

`check_frozen_inputs` 检查项目/锁/profile/源清单之间的关系；`verify_reference` 检查宿主提供的真实字节，`verify_files` 再检查输出清单无遗漏/额外文件。它们不自行打开路径，不替代宿主权限、完整依赖发现、上游工具执行和实际 ABI 验证。

要求范围 required/optional/not-applicable、工作进度、六种执行状态、证据 missing/current/stale 分开。排除需要理由；required 案例必须匹配独立定义、身份、精确结果、最低证据层级、清理和附件。资源观察必须守恒并回到基线；不能靠 report 自报 passed、任意崩溃或编译日志给路线认证。

[测试样本](../../tests/fixtures/contracts/README.md)使用明确标识的合成工具和占位文件引用。其测试通过只验证格式和规则；C03 提供真实工具锁，C04 提供应用流程，C05/C09 提供候选及独立 Lab 执行。

## C04 执行辅助格式

- [execution](execution.schema.json)：单阶段执行配置；真正的请求仍由各业务 schema 校验。
- [dependency-request](dependency-request.schema.json)：明确 Clang image、target、源/搜索域、参数、环境与缺失条件。
- [dependency-manifest](dependency-manifest.schema.json)：真实发现结果、完整保守输入上集与实际读取子集；保存该文档不能单独授予缓存资格。
- [cache-entry](cache-entry.schema.json)：固定原生准备输出的字节清单；恢复后逐项校验，不代表提交或认证。

所有辅助格式离线验证，未知字段和格式拒绝。实际支持子集、准备与重新绑定协议以[生成规范](../architecture/07-code-generation.md)为准。

C06 使用可选 `shape.throws` / callback `throws` 与 resource `backend-handle`，分别受 `core.errors@1`、`core.backend-resources@1` 能力门禁保护。错误类型参与相关符号的可达类型摘要，旧文档省略字段时保持原摘要。定义和版本拒绝规则见[契约规范](../architecture/03-contract-and-ir.md#c06-的错误通道与后端句柄)。

C07 的 `core.external-runtime-verification@1` 复用 profile.required_features 的现有结构，属于必需语义协商，不新增 wire 字段。默认三阶段保持；新模式的 Build 图仅允许 compile/link，并把目标运行留给独立 Lab。未知功能仍明确拒绝，选择进入 profile 摘要；只声明 supported_features 不切换策略。规则和回归边界见 [ADR-0010](../adr/0010-external-runtime-verification.md)。

## 已采用组合格式的准入边界

[ADR-0018](../adr/0018-extension-composition-formats.md)采用 `composition/1`、`profile/2` 和 `core.extension-composition@1`。`DocumentKind::ProfileV2` 与旧 `Profile` 分开；能够解码新格式不赋予旧 read/generate/package/verify 入口执行资格。旧扩展文件闭包遇到新格式明确拒绝，不能把它当作无引用的普通文件绕过组合检查。

包源摘要复用 source 域，包内文件到 lock.inputs 使用包限定路径；执行镜像仍单独按实际字节绑定。生成资产只有 producer/output_slot/paths，真实 OutputSet 的字节、来源及事务要在生成后检查。新 feature 不加入旧运行语义的无条件能力集合，仅在明确的 profile/2 组合检查中识别。必需集合来自已固定配方和真实声明；输入自身不能授予完整性证明。

新格式的 [共同向量](../../tests/fixtures/contracts/composition-format-vectors.json)与原向量一并运行。结构样例含专用夹具身份，不是已安装包或正式候选。实际执行接线、生成后绑定及独立普通/增强/禁用消费仍按 E01–E03 和相关工作包交付。

当前纯适配接口由宿主传入固定配方事实与独立必需责任集合，返回只读绑定观察或比较结果；它尚不从实际业务声明和 SDK 导出自动建立整套配方事实，也不提供执行、提交或认证令牌。调用方不能把同一份申请数组同时当作必需集合和已提供集合来宣称完整性。

## I01 完整模板与结果

[ADR-0023](../adr/0023-project-template-and-init-result.md)采用完整 `project-template/1`、`init-result/1` 及有限首分片。模板库存覆盖全部原包文件（仅排除 manifest 自身），原包摘要计算受限 USTAR 的完整原字节；解包根不随嵌套 manifest 的父目录移动。结果原包、输入根、边界原 profile/SDK、计划文件字节和证据根分别绑定。库存、跨引用、准入、真实创建和检查事实在 schema 之外验证。

`DigestDomain::InitRequest` 使用 `madb.init-request/1` 域。内部投影精确为 `{template_selector,template_profile_selector,parameter_values,targets,verify,open,dry_run,offline,output_path}`；合法默认先展开，targets 为完整排序唯一 family，output_path 纯词法绝对规范化。它不是新增 InitRequest wire。摘要为空不改变真实阶段错误类别；原包/工具身份也不能由请求摘要代替。

普通成功必须由同根同计划 Host 创建事实和独立配方必需检查支撑；dry-run 只保存独立证据和计划，不创建工程或运行工具。`InitResult` 的结构与语义校验不对第三方自报创建背书。首配方仅按已批准的 profile/1、macOS/aarch64 与精确工具集合准入，当前开发版本不能伪装成正式兼容版本。格式与纯规则结果不等同于工程/IDE运行。

## I02 doctor结果

[ADR-0025](../adr/0025-doctor-preflight-contract.md)采用[doctor-result/1](doctor-result.schema.json)。三档仅表示静态前提检查范围，scope固定为`preflight-only`；不存在DoctorRequest wire。工具身份、请求、局部配置引用和检查事实分别允许适用的空值，不据空值重分类错误。输出预算DOCTOR001的缩减结果保留实际更高优先级错误。

[共用格式向量](../../tests/fixtures/contracts/doctor-format-vectors.json)由Rust和独立Python校验器消费；模型夹具不构成实际预检证据。schema检查严格形状、字段及局部状态关系；app/contracts仍须验证必需行全集、唯一ID、向前的blocked依赖、完整输入关系、实际观察与返回前重验，不能由schema有效推导预检成功。
