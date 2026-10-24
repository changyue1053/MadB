# C01 公开仓库与合入保护验收

日期：2026-10-24；设计基线 0.9。任务范围为 C01 的仓库公开、合入保护验证和对应文档状态。此前[工程基线报告](2026-10-24-c01-baseline.md)保留当时的私有仓库阻塞记录；本报告补齐该项证据，C01 更新为 implemented / passed。后续工作包未启动，PR 未合并，没有产品发布或平台认证。

## 决定与公开范围

项目所有者明确授权先将 [MadB](https://github.com/changyue1053/MadB) 和 [MadB-Lab](https://github.com/changyue1053/MadB-Lab) 公开，Pro 后续另行处理。两个仓库的 API 读回均为 PUBLIC；原先保护配置的 HTTP 403 已解除，无需改变既定 CI 验收要求。

公开内容为 C01 工程基线、项目自有设计资料和有来源归属的历史研究。实际 LICENSE/NOTICE 已提供；依赖、历史资料与后续嵌入片段的处理以 [第三方范围说明](../../THIRD_PARTY.md)为准，第三方权利保持原有归属。公开仓库不代表已经提供可安装的跨语言 SDK。

公开前检查了 MadB 的 202 个、Lab 的 26 个可达 Git blob，并展开历史 ZIP 的 20 个 Markdown/JSON 成员。所用私钥、GitHub token、AWS key ID 等高置信模式未命中；该结果仅覆盖上述模式和扫描版本。原归档未包含原生库或上游生成器实现，来源标注与原文快照继续保留。

## 实际生效的 main 保护

| 设置 | MadB | MadB-Lab |
|---|---|---|
| 必需检查 | MadB C01 baseline | MadB-Lab C01 baseline |
| 检查来源 | GitHub Actions，app_id 15368 | GitHub Actions，app_id 15368 |
| 分支须与 main 保持最新 | strict=true | strict=true |
| 管理员同样受限 | enforce_admins=true | enforce_admins=true |
| 强推 / 删除 main | 均禁止 | 均禁止 |
| PR 批准数量 | 0；当前单维护者配置 | 0；当前单维护者配置 |

两仓库均通过 API 应用既有 .github/branch-protection.json，并 GET 回读实际规则。批准数为 0 不代表必需 CI 可以跳过。将来修改可见性、账户能力或合入控制时，贡献指南要求重新核验有效规则和失败阻断。

## 失败阻断与恢复验证

为排除草稿状态的影响，验证时两个 PR 临时转为非草稿；代码无合并冲突。各加入一条预期违规，核对独立 CI 的具体诊断及 GitHub 的合入状态，再用独立 revert 提交撤销负例。

| 仓库 | 负例提交 | 预期拒绝与实际状态 | 恢复提交与实际状态 |
|---|---|---|---|
| MadB | 817749f4bc7f0ec0ef679122ee17fe426ad57c3b | ARCH003 / ARCH005：model 反向依赖 protocol；[CI 失败](https://github.com/changyue1053/MadB/actions/runs/40072075565)，非草稿 PR 为 BLOCKED | 8087ed0c0b2cd4f893fd5b15d1402525a9ab0ba4；[CI 通过](https://github.com/changyue1053/MadB/actions/runs/40072212785)，非草稿 PR 恢复 CLEAN |
| MadB-Lab | 33058c43e8c7c84c8f6e3ae110cecd12c8b06b0c | LAB006 / LAB007：Lab 引入私有 crate 和外部源码路径；[CI 失败](https://github.com/changyue1053/MadB-Lab/actions/runs/40072083199)，非草稿 PR 为 BLOCKED | 412ffc6877a3cf51d70361452b149ce9e3415e8f；[CI 通过](https://github.com/changyue1053/MadB-Lab/actions/runs/40072220264)，非草稿 PR 恢复 CLEAN |

恢复提交的 Git tree 与各自负例前完全相同：MadB 为 5f5e97246d96c524469f8cee9ae187b22bf5b1bd，Lab 为 a57bbf41ba3114d732caea1def7cde5524f26843。没有保留违规依赖、Lab 临时 Cargo.toml 或测试错误，也没有向 main 合入负例；历史保留准确的失败和恢复过程。

上述证据绑定表中提交。此后的文档与计划提交由 [MadB PR](https://github.com/changyue1053/MadB/pull/1)和 [Lab PR](https://github.com/changyue1053/MadB-Lab/pull/1)的最新必需 CI 验证；不将表中历史运行冒充新提交结果。验证完成后恢复原草稿状态，合并仍作为独立操作。

## 状态与修改范围

- C01 本地/远程构建、CLI 小任务、依赖负例、Lab 独立性证据沿用既有基线；本次补齐真实 main 保护与失败阻断、恢复通过的证据。
- 更新两仓库贡献说明、公开范围、Lab 规范入口、文档首页和审查索引；清除计划中的过时环境失败说明。Lab 固定规范、REVISION 和摘要不变。
- 更新权威任务目录并重新生成 HTML。41 个工作包中仅 C01 的验收状态变化；九组边界、16 条主线及后续任务范围保持原有定义。
- 历史审查报告、研究封存资料和第三方来源归属不改写。SDK、FFI、设备及真实浏览器验收仍由后续对应任务提供。

本次 Git HTTPS 域名连接超时，通过 GitHub Git API 同步原有本地提交；远程 tree 与 commit SHA 均逐个核对完全相同，仅快进工作分支，未强推或改写历史。
