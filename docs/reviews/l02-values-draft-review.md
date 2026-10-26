# L02 报告草稿只读复核

结论：Mac 数字、候选身份、证据分级、历史结论和独审串联准确。审查期间协调人更新了草稿，以下两项主要修订已在新稿中落实；仍建议采用下列精确措辞和证据链接补充。本审核不批准收包，Linux 与持久归档占位不构成通过证据，也不新增相邻工作包前置。

被审稿为 `/private/tmp/madb-chief-round7/l02-native-values-report-draft.md`，SHA-256 `11cb1c6ef764d35a7c48f5a3aab3039e90e1a32d422850a892cae5a5a222468b`。以下行号绑定该版本。仅写本复核文件，未改原稿、两仓源码/catalog、正式候选或历史报告；未重跑 SDK、编译器、Linux 或 GitHub 查询。

追加读取的修订稿 SHA-256 为 `d00bda974bdc672c84867015b0ac6d6d45384ac90a4c8d324732ea44d5137c83`。第9行已限制作者至多一次、有效输出初始化及业务 domain/tag；第37行已补子进程开始时间/环境和mtime限制。下面第1–2项保留问题与依据，**不再列为该修订稿未解决事项**。

剩余建议均是既有边界的表达，不新增验收条件：

1. 第9行“旧7/14/9/16角色”改为“旧普通、测试伴随、诊断、诊断伴随配置分别保持7/14/9/16个 native 导出”，避免角色数与ABI导出数混称。
2. 第45行“旧实现相关源码没有漂移”收窄为“已复核的旧 SDK engine、explain CLI/app/adapter/说明目录及 first-route ownership 既有文件字节未变；本轮共享接线与新候选另行验证”。当前 driver/native_ast/shared gate 确有新值配方变更，原审计未声称这些共享文件全部未变。
3. 保留下文建议的 unwind/abort 前提、局部 `full_l02_acceptance=false` 和实际证据链接。现有草稿没有把Linux或整个L02提前记为通过。

## 具体修改

### 1. 第9行：收紧调用次数和失败输出的概括

原句“输入先验证，转换/作者调用各一次；失败输出清零”省略了实际前提。身份/输入/输入分配拒绝时作者调用为0；raw 的两个输出仅在有效内存、不重叠、无数据竞争的调用者前提下分别检查 NULL/alignment 并清零。业务 `Result::Err` 的 value 为零，但 error 必须保留 domain=1 和实际 enum tag，不能泛称所有失败输出全零。

建议替换为：

> 身份通过后才执行一次 raw 调用；raw 先验证全部输入，转换成功后仅调用作者一次，身份或输入转换拒绝时作者调用为零。失败时 value 保持零，合法业务错误保留明确的 domain/tag；raw 仅对满足有效内存前提并通过 NULL/alignment 检查的输出独立清零。

同时建议紧接原生异常说明补一句：

> 可恢复 unwind 依赖选定的 unwind 构建；abort、OOM abort、无效调用方内存和进程故障不属于可恢复保证。

这是对已批准边界的准确转述，不新增实现或验收条件。依据为已批准提案第3–6节，尤其原文112–125、133–144行，以及当前 `values_codegen.py` 102–151行。

### 2. 第37行：完整保留 C05 归档限制

当前准确保留了两份 Cargo build 日志被覆盖、三个临时消费目录清理，但遗漏同一独审明确列出的子进程时间戳/环境限制。第60行又写“所有完整度限制……保留”，建议补齐原限制并链接实际独审，避免最终报告比已有独审范围更强。

建议在现有段落末补：

> C05 子进程日志没有独立 started_at；SDKROOT 仅记录于外层执行文件，未归档每个子进程完整环境。临时消费目录已清理，因此现存安装副本和可执行文件不能再次检查。本次属于既有证据核对，不是独立重演；详细范围见 C05 独审。

依据：`macos-review/c05-audit.md` 的“证据限制”1–3项及对应 JSON。不得为补齐归档而改写原始运行记录；日志路径修复用新 Lab revision/新报告的草稿安排正确。

## 已核对且无需改数字的内容

| 草稿位置 | 复核结果与范围 |
| --- | --- |
| 第3、21、25–31行 | producer `e0f74a5`、Lab `2d06cbe` 与七个候选 digest 均匹配 Mac producer 和独审记录；目标及固定工具版本相符。 |
| 第11行 | 旧 gate 独审 → 最终 reader 追加独审 → 集成后8项 ownership/17项 reader 的三段串联准确。旧 names.rs pin 与最终文件不同，没有被改写成旧报告全字节覆盖最终提交。原生 payload 清理修复有独立重放与字节绑定。 |
| 第15行 | 产品完整 check 日志有最终成功结尾。Lab unittest 八组实际计数为507+21+18+6+28+20+12+9=621；这是仓库/harness 门禁，不是621个 SDK 认证案例。草稿已将正式 SDK 结果另列，可保留。 |
| 第17行 | 最终 clean live 的 build/live exit_code 均0，实际2项测试通过、五步变更及七类拒绝留证，源与工具镜像前后不变。固定 profile ID 与变化的 profile digest 已正确区分。 |
| 第33行 | 29个唯一案例=18真实SDK+5helper-only+5static+1compiled；report/1原层级23 target-run中含5个helper。19次SDK进程来自18个案例，其中混候选案例执行两次；5次stub、116向量、9次编译/链接拒绝均与审计结果一致。集成后复跑29不增加不同案例数。 |
| 第35行 | 新候选旧L02五案例包含56次真实SDK、24次stub、4次导出观察，合计84项观察；没有称84次SDK运行。C05最终19=17 target-run+1compiled+1static；首次4error/12blocked/3passed保留准确。 |
| 第37行 | 12原候选=7值+5旧配置；11保留安装副本=7值+4旧L02消费配置；Lab523源文件、七个生产工作区核对均有实际记录。不是12个都执行了同一套案例，也不是C05临时安装副本仍完整保留。 |
| 第45–54行 | 旧explain/诊断/checked constructor在其原冻结候选范围复用，Mac新候选回归另绑定；退出4没有冒充真实CLI文件系统竞态。普通资源/回调、静态/运行/helper与新值配方分列，没有扩大到Swift/ArkTS/组合认证。 |
| 第58行 | 1155/481既有文件核对、1186/523最终文件相等、fast-forward及原detached worktree保留，均与 `integration-source.json` 一致。 |
| 第62行 | 三个提交标题均带 `[skip ci]`，两个push退出0；已保存的GitHub快照核对两个远端ref和三个head_sha各0个Actions run。该快照观察时间为 `2026-10-26 20:01:45 UTC`，草稿“未观察到”措辞正确，不构成未来或持续零运行承诺；零运行也没有当作CI通过。 |

## 定稿前需要保留的分层与链接

- 建议在第33行补一句：“局部原始观察仍为 `full_l02_acceptance=false`；本报告在其各自范围内合并证据，整包状态只由 catalog 维护。”这是对已有原报告范围的说明，不能回写局部标记。
- Linux段继续等待本轮七个正式候选、独立 pins、29个分级案例、已计划原生专项和最终独审；归档段继续等待实际持久索引。占位阶段没有整包通过结论。
- 定稿归档索引应直接指向下列已存在的证据，尤其最终 clean live 结果、旧/最终 reader 与 gate 独审链、Mac C05限制及已保存的推送快照。无需为已有正确数字增加重复执行。

主要依据：

- `/private/tmp/round7/l02-closure-audit.md`
- `/private/tmp/madb-chief-round7/l02-prior-evidence-audit.md`
- `/private/tmp/madb-chief-round7/final-clean-live-001-evidence/result.json`、同目录 `live.log`、`final-clean-live-001/live-result.json`
- `/private/tmp/madb-chief-round7/product-clean-gates-004.log`、`lab-clean-gates-001.log`
- `/private/tmp/madb-chief-round7/macos-review/values-l02-audit.json`、`values-l02-checks.json`、`producer-input-audit.json`、`c05-audit.md`
- `/private/tmp/madb-chief-round7/integration-source.json`、`push-stage-source.json`、`remote-post-source-push.json`

本轮审查发现的是报告边界表达及归档限制遗漏，没有从这些已查证据发现新的实现阻断。后续 Linux实际结果或源码变化仍需按真实情况另行审查。
