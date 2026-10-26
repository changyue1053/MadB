# I01 下一轮最小范围审查

推荐下一片先完成**完整模板/结果格式与首条 `rust-c-sdk` 配方的具体决定材料**，再接生产 decoder。四项增量已经批准，不重批；D 已有实现，A/B/C 可做窄内部规则，但不能用它们提前准入可执行模板。

这是只读范围建议，不是采用决定、任务状态或验收报告。实际产品根 `/Users/changyue/Desktop/MadB/madb`，读取前后均为 clean `e0f74a5fa5a9a6e73400ced70bc81ba06751227f`。未修改两仓源码/catalog/历史报告，未运行测试。子 agent 独立核读了源码；下面的既有测试数量来自源码与第六批报告，不声称本次重跑。

## 当前已有与尚缺

| 当前事实 | 精确边界 |
|---|---|
| catalog：I01 `in_progress / missing`，依赖 C02 | I02→C04/I01、I03→I02/C05 保持；子范围可提前做，不解除整包依赖。 |
| `madb-plan/src/project/mod.rs:1,68` 已有 ProjectPlan | 模块 DAG、明确 owner、四类所有权、路径/大小写/文件目录冲突；不是 wire 或文件提交许可。原9项测试，不改成“最深目录自动独占”。 |
| `project/selection.rs:56` 已落实 ADR-0019 D | 四分支、显式空/重复 targets 拒绝、完整 family 精确匹配、明确默认消歧、保留精确 targets；所选模块/文件交回原 ProjectPlan。第六批新增12项及原9项独立复验仅证明纯选择。 |
| A/B/C 已批准，但无生产初始化链 | 源中没有 InitResult、ProjectTemplate、模板 input_root 读取链或 CommitNew；CLI `commands.rs:136` 无 init，Protocol `schema.rs:17` 无这两种 DocumentKind。 |
| 有基础可复用，不可冒用 | `madb-app/src/ports.rs:4`、`inputs.rs:80` 可冻结/重验原字节，但不定义完整模板包。`transaction.rs:139,155` 的 CommitReceipt/BatchStore 只提交 Source/Build/Candidate 索引，不证明最终新工程根和同源码计划的无覆盖 CommitNew。 |

第六批报告第27行仍准确。第七批 SDK/候选证据也不证明初始化、Wrapper 或 IDE 完成。`project/1` 和 `report/1` 不能代替待定的模板/初始化结果格式。

## 现在可自主推进的最小片

1. **格式闭合材料，优先执行。** 新建一份合并 ADR-0019 的定稿提案，补下表缺口、完整正反设计实例、首条 `rust-c-sdk` 支持表并独立审查。保留旧草案/报告原结论。只把剩余新格式与兼容影响交决定，不重问四项，也不把候选 JSON 注册为生产格式。
2. **A/B 私有纯规则，可并行但不另建模型。** 复用既有 Digest、ErrorKind、检查语义，表达摘要保留与普通/dry-run 成功条件；消费冻结请求、受检计划和只读阶段事实，不序列化、不发创建许可。最低反例：null＋Host仍是Host；已有摘要不丢；普通未创建不得成功；structure仍须创建；dry-run有执行观察拒绝；提交后失败/取消保留created；清理缺口非零。纯事实单测不能冒充真实 CommitNew。
3. **C 的窄引用适配。** 只对已给定、已校验清单做两根适配：外层manifest/profile FileRef加input_root前缀，内部/1、/2、composition和锁原字节不改。核嵌套manifest、同名异内容、错根/越界/错hash与size；不计算未定义的整个package摘要，不做任意包解析器。

A/B/C应随实际调用方接入，避免为推进进度创建孤立框架。D不重写。首配方的工具锁、owner、SDK validator、IDE入口可只读盘点；现有C04脚本和 `madb.native-toolchain/1` 字符串不是现成I01配方资格。

## 不应越权实现的具体字段

ADR-0019第16行与提案第117行明确：完整envelope、包身份/获取锁和有限支持表定稿前，不能注册 `madb.project-template/1`、`madb.init-result/1` 生产schema/decoder。

| 剩余决定 | 必须闭合的内容及建议 |
|---|---|
| `template.package_sha256`、模板id/version、获取锁/重验凭据 | 定义完整原包字节范围、入口与包根、来源/cache pin和重验时点。优先固定整个发行包原始字节身份与完整解包闭包；不能擅用composition的source摘要或所选引用子集充当整个包。复用现有FileRef/锁输入优先；新获取字段须由具体方案证明必要。 |
| 模板顶层其余字段 | `id/version/madb_compatibility/default_profile/parameters/profiles/modules/boundary_refs/assets/ide_entries/task_roots/validation` 的必需性、空值、唯一性、未知字段及交叉引用。当前第三批草案不是采用决定，不能塞进extensions绕过。 |
| 参数与版本 | `parameters` 的类型/default/required/字符串pattern与choices/整数上下限；MadB/template/host/IDE区间与预发布规则。固定有限语法、默认展开及反例，不造通用版本solver；通用Id不等于语言标识符校验。 |
| profile目标与SDK | `profiles[].targets[].{id,family,boundary_refs}`、各引用集合、`requirements.{structure,build,run,open}` 的精确解释。**已批准删除sdks**，只保留 `{hosts,tools,ide_entries}`；SDK来自profile.sdk和点名有限配方的实际validator。/2可读不等于组合可执行。 |
| 资产/renderer | `assets[].{source,output,ownership,renderer}`；`renderer.id/bindings[].{parameter,pointer}`。copy逐字节；json-values必须明确现有严格JSON安全整数子集、已存在节点替换、重复/非法pointer拒绝。源码专用renderer未定前不能全局字符串替换。 |
| 首条recipe及检查 | `task_roots[].{id,module,recipe,boundary_refs,targets}`；配方版本、目标/工具/SDK/renderer允许表、真实输入/输出owner与独立必需检查。先定产品已列rust-c-sdk，复用原Generator/Toolchain/TaskGraph/C04；无任意argv/shell、新E02操作或未来产物假摘要。 |
| IDE/验证定义 | `ide_entries`的ide/version/path/run_target/extensions；`validation`的level/target/task_root/required/minimum_evidence/prerequisites。必需集合由受检配方与INIT承诺派生，模板不能自降optional；launch不等于import/debug。 |
| InitResult剩余形状 | `tool/template/template_profile/boundary_profiles/request/output/plan/creation/termination/exit_code/checks/ide_entries/open/recovery/diagnostics/logs`，尤其plan.downloads、检查环境/命令/附件/cleanup。保持已批input_root、真实created、dry-run与open相等约束；不让结果自报required列表或created来证明成功。 |
| 请求投影、脱敏及诊断 | 单一候选 `madb.init-request/1` 的实际投影类型/默认展开/词法绝对路径；`output.{path,redacted}`、recovery argv、日志/附件根。不要追加第二摘要/请求wire或130退出约定；新INIT code和doctor独立结果字段仍须具体审查，不重问I02归属。 |

首条可执行模板还有一个具体连接点：产品的 `rust-c-sdk` 包含Rust库、C消费程序、CMake及运行任务，`--name/--namespace` 可能进入Cargo/CMake/C文本。**copy/json-values并不自动提供这些语言的安全渲染能力。** 支持表必须说明哪些参数只改JSON、哪些由既有生成器产生、哪些需新的精确renderer；不能复制后偷做字符串替换，也不能永久取消既定参数/平台承诺。

## 推荐交付与验收边界

- **本次可直接交付**：一份完整格式定稿提案＋首配方支持表＋正反设计实例＋独立审查；四项列为“已批准”，剩余字段列为“本次新决定”。可以收这个文档/规则子片，不能收I01整包。
- **剩余格式采用后**：共享SchemaRegistry/decoder/共同向量单写；adapter把受检选择闭包交回原ProjectPlan，核完整包/manifest/profile/锁字节；未知语义/recipe/renderer/SDK、错根与引用冲突拒绝。这仍不等于真实init。
- **再接I02/I03真实竖切**：Resolve→Preflight→Render→结构检查→真实CommitNew→最终路径Build/Verify→可选Open，CLI/IDE共用Wrapper。按精确公开宿主/目标做INIT-01/06/07/08/09/11及承诺的02/03/04/10适用项；Lab独立消费，模板自己的断言不作唯一oracle。真实链接/无覆盖并发/恢复不能用纯路径单测替代。

保持单一协调入口；共享契约/catalog单写，纯规则、配方证据及独审按接口并行。已有工程接入/升级/禁用、Swift两类配方、ArkTS、完整IDE矩阵和成本基线均留在原队列，不由首片外推完成。

## 读取依据

catalog I01–I07；ADR-0019；第六批报告第27行及sixth-batch-i01-{validation,review}.json；project-init-decision第20–117行及其独审；第三批format-draft第27–170行（候选字段）；architecture/14、product/04第63–93行、validation/06第7–26与64–82行；AGENTS和共同方法、模块、质量规范。源码定位由独立只读复核确认；本次没有新增执行证据。

关键输入SHA-256：

| 文件 | SHA-256 |
|---|---|
| `docs/catalog/implementation-plan.json` | `2891c68a456b093ff47a32440ba23b82ad30bb085702bc87852d1f7f8b7da2c1` |
| `docs/adr/0019-project-init-incremental-contracts.md` | `b810339d917442faf32ad6ec3890d41fb4a88dee3cf693ed5b47910057ea8d70` |
| `docs/reviews/2026-10-26-chief-architect-sixth-batch.md` | `eca87b32e751a180dd8faaaf7ea57ee94972615a51b09779b391dd439b064b01` |
| `docs/reviews/project-init-decision.md` | `fb4a1741949fe1d1bfb878457819136de8a3782346900a02eb9b3afa10dce935` |
| `docs/reviews/2026-10-26-project-init-format-draft.md` | `431acd6d4869693b97300b36acf090271f1a02d500c3e859b4838711d265ff66` |
| `crates/madb-plan/src/project/selection.rs` | `dffe987b9287c832059774e2b0103661aafe36b8661e5e952d7e3a8b4abd7632` |
