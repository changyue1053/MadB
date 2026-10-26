# L02 round6 既有正式证据收包对账

结论：在原冻结源码、候选、目标和案例范围内，explain、首链生产诊断/checked constructor、关闭诊断保护及有限首链 DEC-07 已有可追溯证据；本次没有发现这些旧子范围的新增阻断项。旧的两项报告缺口（五个 L02 case/1/report/1、Linux 对应诊断候选结果）已由后续正式记录补齐，不能继续照抄早期 exit-audit 的待办。第二业务的新生成与候选消费由 round7 单独审核，本文件不为其作通过判断。

本次只读。实际产品 HEAD 为 `e0f74a5fa5a9a6e73400ced70bc81ba06751227f`，读取时 `git status --short` 无输出。未运行 SDK、编译器、设备或容器，未改产品、Lab、catalog 或历史报告；唯一输出为本审查文件。

## 依据及精确范围

实际读取根 AGENTS.md、共同 Agent 规范、catalog L02 五项 acceptance、architecture/15 的诊断/普通入口边界、validation/07 的 DX-01/02/10/11、ADR-0020/0021，以及第六批记录、正式冻结候选报告和它们引用的独审/执行索引。

- 旧 producer：clean `fbb0a493fa0380797918a5a83c210b1b5f621ae1`。
- 最终旧 Lab：clean `9286861babe9b0f3d65a8ac7cc8f87469dc63475`。
- 案例目录：`sha256:5c74de3764114dc4b82a3a63a506a9de50b2bd8c5c88cd76aeb29dc143557b98`。
- macOS arm64 和 Linux x86_64 分别固定五配置：普通、伴随、反向、诊断、诊断伴随；候选身份不互借。
- Linux 是 QEMU 7 用户态目标，C05 最终成功使用固定 Clang 私有 cc 路径；原 GNU cc 失败仍是失败，不得写成已恢复。此范围不等于物理 x86 硬件或性能认证。

## 对 L02 五条验收的贡献

| 当前 acceptance | 已有证据与本次核对 | 本范围仍不得宣称 |
| --- | --- | --- |
| 声明复用、来源映射、普通入口独立消费 | 真实 cbindgen/Clang、受检 IR 与来源；真实 ArkTS reader 两类拒绝→10 次解释查询；新生成 `madb_rust_client.h` 的自动身份构造；两目标真实 ordinary 消费、四组实际错库拒绝。 | 旧固定七/九角色不等于第二独立业务成功生成；不外推 Swift/ArkTS 正式包或任意业务。 |
| 关闭诊断仍保留资源/线程/状态机保护 | 两目标的 `disabled-protection` 标准案例通过；本次读取两份原始 local report，各 84 项中 56 个 SDK 进程均以 passed 结束。两项 off 均显示 events=0/capture_enabled=0；八项 self-wait 均为 WOULD_DEADLOCK、observe=CLOSING、close=confirmed；两项慢 exporter 均在 exporter 释放前完成普通业务。 | 不称任意调度无缺陷、全线程安全证明或全系统无遗留资源；数值资源数组为空处不填造计数。 |
| 首链普通资源/回调与生成诊断，分别声明覆盖 | 64 槽生产 pull 诊断、configure/drain 的四输出与拒绝、普通资源/同步回调、off/on/full、FIFO、容量/耗尽、独立 image、重入/争用/故障隔离均有源码独审和候选消费。旧 C05 两端各 19 案例另记。 | 静态 surface 校验不是静态内存安全；有限并发/计时样本不是 sanitizer 全覆盖或实时上界。L06 的 RAII/move-only/静态属性仍归 L06，不新增为 L02 前置。 |
| DEC-07 每符号责任、普通/便利共用机制 | 七/九 role 的 required/provided、27/33 行责任表、核心比较、Source 实际字节和 Package 精确保留已实现；B1/B2（省略责任表/换 reader 标签）漏洞已修并独审。checked helper 仅委派既有 bootstrap/create，没有重复资源机制。 | 不以固定表替代第二业务实际符号和策略派生。新纯值配方须有其自身责任资产；不要求为纯值捏造订阅/排水。 |
| explain 稳定码、触发/来源/修复、未知码、机器结果/退出、只读 | 51 个有限词条，21 个真实 CLI 场景、14 项受控读取分类/零读取检查、26 个共享格式向量、两类真实 reader→10 次文本/JSON 查询；NATIVE006 最后说明改动另独审。 | context 只绑定给定字节/位置，不认证诊断生产者或整个候选。退出4的受控重检证据不冒充真实 CLI 文件系统竞态复现。 |

## 正式报告和实际字节

本次直接读取并核对下面四份 `madb.report/1` 的 SHA-256、大小、实际 schema 和结果。每目标均为三个普通诊断 case 和两个诊断伴随 case，全部 `passed / target-run / cleanup complete`：

- Mac：`/private/tmp/madb-chief-round6/l02-macos-standard-004/c-to-rust-diagnostics.report.json`、`c-to-rust-diagnostics-test.report.json`。
- Linux：`/private/tmp/madb-chief-round6/l02-linux-consumer-002/l02/workspace/c-to-rust-diagnostics.report.json`、`c-to-rust-diagnostics-test.report.json`。
- 五个稳定 ID：`checked-constructor`、`diagnostics`、`disabled-protection`、`diagnostics-isolation`、`diagnostics-capacity`，均带 `l02.c-to-rust-diagnostics[‑test].` 对应前缀。

各目标的 84 项观察准确分成 56 次真实 SDK 进程、24 次 helper stub、4 个导出检查，并另有 12 次编译；不能说成 84 次真实 SDK 运行。两个 local report 的字节均与最终独审索引吻合，且仍明确 `full_l02_acceptance=false`。

已实际核对两份 producer freeze：Mac `e00b3267fb2d49d684d4a5fbadca075c29d84c1f0abc38c4fd8027cd19dca574`；Linux `cbdd19a385816a3d880c92a5d994f74ef8bb4f25229d7754646ed8c263a80675`。这里复核索引字节与已记录独审，不重新执行 90 个公开候选文件或 408 个 Linux 导出文件的全量复审。

持续保留的权威入口：

- `docs/reviews/2026-10-26-l02-frozen-candidate-validation.md`
- `docs/reviews/l02-final-macos-review.json`、`l02-final-linux-review.json`
- `docs/reviews/l02-final-macos-execution.json`、`l02-final-linux-execution.json`
- `docs/reviews/sixth-batch-native-diagnostics-review.json`、`sixth-batch-responsibility-review.json`

持久归档 `/Users/changyue/Desktop/MadB/.local/evidence/round6-l02-fbb0a49-reports/archive-manifest.json` 的当前实际 SHA-256 为 `efab2b39715d725fd0f16ebc5ba252313eb52f35281d39ff93d8c12ec4ef5df3`，与仓库索引一致；本次没有重新遍历全部 1977 个归档文件。

## explain 精度补记

`/private/tmp/l02-explain-review-probes/cli-results.json` 等六份已引用结果的实际摘要均吻合。21 个真实 CLI 场景涵盖出口0/2/3、未知码先于缺失 context/root 拒绝、缺参数/多参数、BOM/UTF16/字节位置、失效 UTF-8、已删除 cwd 下纯码查询、stdout 失败。出口4由受控 ReadWorkspace 的“捕获后变化”分类、既有机器格式向量和 CLI `Conflict -> 4` 映射共同支持；报告应保持这个层级，不声称已有真实 CLI 竞态重现。

读取的独审明确 NoWorkspace、未知码先查表、只用 bundle 说明、无网络/工具启动；真实 reader→CLI 复验记录 `source_unchanged_before_after=true`、`query_workspace_unchanged=true`。本次 `git diff --name-only fbb0a49 HEAD` 在旧 SDK engine、explain app/adapter/CLI、说明目录、相关测试、first_route ownership 范围没有既有文件差异；唯一列出的是新增 `crates/madb-cli/src/bin/madb-native-values-read.rs`。因此旧实现独审仍可按相同字节复用，新的共享接线/新候选则由 round7 另行绑定。

## 收口建议与未执行项

本旧范围没有需要重新审批的契约，也没有证据要求把 L04、L05、L06、I 链、C09/C10 整包提前设为 L02 前置。早期 `l02-exit-audit.md` 的五例正式报告和 Linux 诊断候选缺口已有后续正式证据解决；保留其历史原文即可，不把历史结论改写为当时通过。

剩余收包判断应集中在 round7：第二业务的公开声明四种变更、失败保留与实际 Source/Build/Candidate、普通 C 消费、各自责任表、已声明两目标的新有限值候选及必要独审。待这些按当前范围实际完成后，旧诊断/explain 子范围本身没有已知剩余阻断。Linux 新七候选仍在构建的事实不能由 round6 旧 Linux 五配置替代。

本次未执行任何 SDK/编译/设备/容器重跑、全部附件遍历、Actions、发布或新平台认证。C05 各19项应继续按17 target-run、1 compiled、1 static报告；Mac 初次 SDKROOT 失败、Linux GNU cc 失败及旧 local observation 均保留原结论。
