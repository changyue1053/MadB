# 工程构建修订稿：新增格式与原阻断复核

结论：原先架构方向与三个阻断的主要机制已经补齐，但**本轮读取的整稿仍不宜直接作为最终格式提交批准**。剩余问题可以通过局部修订闭合，无需增加框架。此报告覆盖第 1–8 节、两份 draft schema、12 项声明结构向量、已有结构观察及 Wrapper 原型；没有运行测试、validator、Wrapper 或真实工程。

## 已闭合与范围保留

- 原 B1：§6 明确 CAS 前不改变 current，CAS 后取消 AlreadyFinished，已发布但持久化不确定为 true/uncertain/exit3，事实无法确认才 null/uncertain。方向闭合；下面 R1 是结果表和 schema 尚未同步到该状态机的问题。
- 原 B2：静态 launcher 不嵌工具摘要，动态身份来自独立 descriptor，固定系统 plutil 读取字符串，系统 openssl 在清洁环境核对真实镜像，消除构建自引用；完整 schema/来源核对由随后启动的受检 MadB 执行。提案清楚说明系统卷基础信任与非沙箱边界，这一分工可采用。`launcher-probe-001/observation.json` 只记录假工具的正例、错字节、错命令 3 项，不是生产构建证明。
- 原 B3：§6 加入固定 project-state owner、来源标记、旧结果校验、generation/CAS 与禁止通用 recover/GC；未知/损坏 current 不作为首次构建。方向闭合。来源标记确切路径、必须保留的原字节和受管目录列表需在最终私有记录约定中定死，不得凭 `.madb` 名称认领已有文件；这是本次新增 Host 实现责任，无需另开框架。
- §5 已准确写成 app 只定义窄端口/领域约束，CLI 组合根复用 `execute_controlled`；不反向导入 CLI/Host。

## R1：最终发布状态、result 存在条件和 schema 仍有矛盾

提案 §7 当前行 103 写 `result` 仅在“publish 通过”时非 null，而 §6 行 84 及 §7 行 107/114 又要求已替换后 fsync/报告保存失败仍给 true、result、非零。需要明确定义 publish stage 的 status 对应“已替换事实”还是“持久化/回执全过程”：否则两个独立实现可分别发 passed 与 error，同时都自称符合提案。

同时结果 schema 的 `allOf` 对 published=true 只约束 build/result/artifact_root/durability，没有禁止 `exit_code=130, termination=cancelled`；从 structural-positive 改这两字段、保留 true+confirmed 即符合当前结构条件，却违反 §6 的取消封口。`published=null` 分支也未要求实际 artifact_root，而这只能发生在已准备本次输出并尝试 CAS 后，不能是解析失败时凭空给出的 unknown。

最小修订：result 的条件改成**已确认 published=true**（包括后续不确定持久化），并给出固定 publish stage 真值表；true 后只准完成或明确发布后错误，绝不准 cancelled/130；null 必须有实际尝试根、保留本次不可变输出记录的恢复定位，result 仍 null，不授予运行。将这些简单类型条件收进 schema；复杂清理、证据与原始错误优先级仍由语义层判断。补 true/uncertain 正例、null/uncertain 正例、true+130 反例，而非只有 uncertain+exit0 反例。

## R2：DisplayPath 脱敏兼容承诺与 schema 不一致

提案 §7 行 96/100/105/109 复用既有 DisplayPath 和导出脱敏规则。draft 结果 schema `$defs.root`（行 199–216）却只准 `{path:绝对路径,redacted:false}`，同一定义也用于 artifact/evidence/previous/installed_tool，所有脱敏导出都会被拒绝。

既有 `docs/schemas/init-result.schema.json:756–823` 分别支持 `<evidence-root>`/`<project-root>`，`crates/madb-app/src/use_cases/initialize/types.rs:99–103` 的 DisplayPath 本来也区分 Absolute/Redacted。不能把 draft 的限制称作“复用既有”。

最小修订：明确本格式各路径角色的固定脱敏 token，保持红acted 导出是展示、不可重放，不修改原 request_digest；schema 与正反向量同步。或者在这次提案中明确只交本机结果、不声称脱敏兼容，但那是本次决定中的具体范围选择，不能留矛盾到实现时。

## R3：16 MiB 输出上限尚不能经现行注册解码链读回

§7 行 116 承诺 result 上限 16 MiB；现有 `crates/madb-protocol/src/decode.rs:6–15` 的 strict_json 固定 1 MiB，`schema.rs:204–208` 即使输入已是 Value 仍按 1 MiB拒绝。直接按新增 schema 注册无法消费合法的 1–16 MiB 输出；改全局常量又会意外放宽所有旧格式。

最小修订：在本次格式决定中明确 result 专属解析/编码预算，输入 descriptor 保持现有 1 MiB、深度和整数子集；指定窄入口或按 DocumentKind 的限额而不修改旧格式预算。另一最小选择是把本结果也限制到 1 MiB并相应降低/提前检查库存预算。必须选择其一，再准备限额边界正反向量；不需要新解析框架。

## R4：两个新增摘要仍欠精确投影，当前向量只证明结构

§7 的 request_digest 当前仅写 `operation + root + frozen + descriptor原字节引用`，snapshot_digest 写“完整 inputs 原始字节身份及目录集合、描述原字节引用”；没有固定 JSON 对象键/形状。descriptor 是否已经作为 inputs.files 一员又单列、输入 file.path 是否包含 inputs/前缀、目录成员是递归平铺还是逐目录记录，以及早期失败时何时可生成摘要，均影响字节身份。`identity` 的 `{snapshot_digest,candidate,consumer,outputs,tool,recipe}` 已比较明确，但其输入摘要仍悬空。

最小修订：为每个新摘要写一个精确 JSON 投影对象（采用现有 `SHA256(UTF8(domain+"\n") || JCS(projection))`），固定 descriptor 引用位置、文件/目录排序和根路径规则；输入捕获规则明确来源标记是否在 bytes 集合以及其验证基准。给至少一个完整文件清单、目录不存在/空目录、已知 canonical bytes 与已知 hash 的设计向量，另列仅源码变化、仅目录存在变化、root重定位分别如何影响两个摘要。

现有 structural-positive 明确是假摘要结构样本，inputs 仅有作者一个文件，outputs 也未按约定路径排序（consumer 应先于 native）；可以保留为“结构通过/语义拒绝”样本，不能作为完整成功或摘要独立可重算证据。schema 不负责所有跨文件语义是合理的，但完整格式提案须有相应明确语义与向量。

## 其余需要准确登记的事项

- `vectors.json` 声明 12 项，读取到的 `structural-check.json` 仅有 11 条、缺 result-uncertain-success；且观察中 allOf 索引来自旧 schema。只能说“声明12，已有旧11项结构观察”，不能说新稿12项已全部检验。重跑由实施线负责，本审查未代跑。
- stages 的固定序列、唯一性、缺前置时 blocked 和成功时全部必需阶段通过，需在语义向量中逐一约束。当前 schema 的 minItems=1 允许仅一项 passed 的成功报告，是刻意留给语义层还是遗漏，应在格式表说明；不要仅因结构可通过就授予成功。
- install-tool 的实际文件已复制但 fsync/结果保存失败窗口，须说明 installed_tool 是否保留已验证镜像事实、准确恢复为“重试只复核已有匹配镜像”，不得让实现误作覆盖。此处 published 永远 false 仅表示工程 current，不能用来表示镜像未写入。
- Wrapper 原型以路径解析后的固定目录运行且 argv 带引号，当前三项观察支持其限定声明。未知宿主/系统镜像的准入检查必须在 init/install-tool/preflight 内落实，不把系统程序名称本身当摘要核验。根路径中的控制字符按 AbsolutePath/DisplayPath 规则拒绝即可，不额外承诺换行路径；原型本身不应被记作最终生产入口。

完成 R1–R4、来源记录的精确归属和同步向量后，可重新做定稿审查；这不要求重审已批准的旧 init/doctor 格式，也不阻止已授权独立工程工作继续。

## 读取快照

```json
[
  {
    "path": "engineering-rebuild-proposal.md",
    "sha256": "cf95d7d6420e3480b41a8db06c336ff3ff19ee9de867ed72d32f437f3eb3c8db",
    "bytes": 21691
  },
  {
    "path": "engineering/format/project-build.schema.json",
    "sha256": "a726bdf01292055fcaa82895e236d839c1e3ea510e0cfc9d333779dcc2ae155b",
    "bytes": 3941
  },
  {
    "path": "engineering/format/project-build-result.schema.json",
    "sha256": "975b3c703b163deb90405edf6002bf70362f4d7a63b447de73624f980732759e",
    "bytes": 18213
  },
  {
    "path": "engineering/format/common.schema.json",
    "sha256": "df10f44406bfaf6ba6592e8d667861adb5b194bdcbb1aa72f462fdac44f33314",
    "bytes": 13073
  },
  {
    "path": "engineering/format/vectors.json",
    "sha256": "fdd5ed5d8fd84e6a93b311126604d4e1b51f10e149d5f113de48a8963c541953",
    "bytes": 1492
  },
  {
    "path": "engineering/format/structural-check.json",
    "sha256": "b8a90755a19369d277ca9c1862a36541601ed4c3b26acde38958492c4203b0db",
    "bytes": 2878
  },
  {
    "path": "engineering/madbw-prototype.sh",
    "sha256": "7a4906b7041892ff82757a5ab63b833ce25ffbfa8b13c713163dd64f45bb4dac",
    "bytes": 2049
  },
  {
    "path": "engineering/launcher-probe-001/observation.json",
    "sha256": "740d23f3096983c77d9687b53353c67a73f36b97cba50a4406477af7eedd5e2b",
    "bytes": 1396
  }
]
```
