# MadB 0.9 研究归档与对照修订

日期：2026-10-24。范围为九组历史研究的移动、现行设计对照、重复内容清理及文档校验。该工作属于既有 C01 的资料准备，不代表建仓或产品实施验收；全部工作包状态、路线、版本候选和认证口径保持不变。

## 归档与内容处理

原顶层九个目录的 18 份阅读文档归入 docs/references/legacy-boundaries，保留原目录名和文件名。文件标题与内容标明“对照 MadB 0.9 修订”；旧文件名只承担来源身份，不再定义另一套 mad-boundary 方案。

更新前的 18 份文件和旧 source-coverage.json 按原字节封存到 ZIP，包含逐文件摘要。修订阅读版更新路径、行号和摘要，236 个 source_id、问题标题、公共问题映射与证据类别不变；历史报告保持原字节。封存资料不进入默认开发阅读路径。

- 对齐 CPF+akinterop 主线、KNOI 扩展、官方/CPF 构建和发布隔离。
- 对齐 SKIE 主线、NativeCoroutines 分工、Swift export 独立配置及真实可分发消费。
- 对齐 Gobley 模板修复、UniFFI 后端独立配套、napi-ohos 主线与 UniFFI-ArkTS 实验配置。
- 对齐 Kotlin JVM/Native 分流、Android 两类 native 构建配方、Swift C++ 三类交付配置和 Rust/C 首链目标。
- 补接关闭/等待/最终释放、源端/队列/结果所有权、物理单一内核、生成事务和实际装载身份。
- 删除重复模块建议、独立 P0–P3 计划、旧项目迁移建议和本机阅读前提；共同规则链接现行规范、任务与 Agent 方法。

18 份阅读文档从 398,945 字节缩减为 198,944 字节，约减少 50.1%；236 条问题保留，其中 36 条处置/验收有针对性更新。封存文件不计入阅读篇幅。

## 各路线更新定位

| 目录 | 保留问题数 | 针对性修订条目 | 入口 |
|---|---|---|---|
| ArkTS_C_CPP | 30 | 1、19、25 | [阅读目录](../references/legacy-boundaries/ArkTS_C_CPP/ArkTS_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| ArkTS_Rust | 30 | 1、10、22、25、26、27 | [阅读目录](../references/legacy-boundaries/ArkTS_Rust/ArkTS_Rust_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_ArkTS | 28 | 1、14、22、23、25 | [阅读目录](../references/legacy-boundaries/Kotlin_ArkTS/Kotlin_ArkTS_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_C_CPP | 25 | 1、17、23 | [阅读目录](../references/legacy-boundaries/Kotlin_C_CPP/Kotlin_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_Rust | 26 | 1、11、20、22 | [阅读目录](../references/legacy-boundaries/Kotlin_Rust/Kotlin_Rust_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_Swift | 24 | 1、17、21 | [阅读目录](../references/legacy-boundaries/Kotlin_Swift/Kotlin_Native_Swift_架构总结与新版_mad-boundary_设计输入.md) |
| Rust_C_CPP | 23 | 1、3、11、13、21 | [阅读目录](../references/legacy-boundaries/Rust_C_CPP/Rust_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| Swift_C_CPP | 24 | 1、19、21 | [阅读目录](../references/legacy-boundaries/Swift_C_CPP/Swift_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| Swift_Rust | 26 | 1、9、20、23 | [阅读目录](../references/legacy-boundaries/Swift_Rust/Swift_Rust_架构总结与新版_mad-boundary_设计输入.md) |

## 全部文件变化

修改 docs 中 11 个已有文件；新增 22 个文件，其中 18 个为迁移后的修订文档。顶层九个来源目录移出工作区；原内容同时保存在封存 ZIP 与安装备份中，不遗留第二套日常阅读目录。

| 文件 | 修改内容 |
|---|---|
| [README.md](../README.md) | 增加归档入口，当前修订指向本报告，区分阅读版与封存原文。 |
| [architecture/02-modules-and-ports.md](../architecture/02-modules-and-ports.md) | 在目标目录说明 references 包含研究阅读版和快照。 |
| [catalog/source-coverage.json](../catalog/source-coverage.json) | 保持 236 个来源身份、标题、公共问题与证据类别；更新路径、行号和当前摘要，并登记原始快照。 |
| [engineering/06-documentation-quality.md](../engineering/06-documentation-quality.md) | 明确阅读版可修订、原始快照和历史报告不可覆盖，校验范围包含映射位置和快照。 |
| [references/README.md](../references/README.md) | 将历史材料导航统一到 docs 内归档。 |
| [references/history.md](../references/history.md) | 更新九组 18 个入口，说明修订来源与原始来源的定位方式。 |
| [references/legacy-boundaries/ArkTS_C_CPP/ArkTS_C_CPP_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/ArkTS_C_CPP/ArkTS_C_CPP_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/ArkTS_C_CPP/ArkTS_C_CPP_跨语言边界工程清单.md](../references/legacy-boundaries/ArkTS_C_CPP/ArkTS_C_CPP_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/ArkTS_Rust/ArkTS_Rust_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/ArkTS_Rust/ArkTS_Rust_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/ArkTS_Rust/ArkTS_Rust_跨语言边界工程清单.md](../references/legacy-boundaries/ArkTS_Rust/ArkTS_Rust_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/Kotlin_ArkTS/Kotlin_ArkTS_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/Kotlin_ArkTS/Kotlin_ArkTS_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/Kotlin_ArkTS/Kotlin_ArkTS_跨语言边界工程清单.md](../references/legacy-boundaries/Kotlin_ArkTS/Kotlin_ArkTS_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/Kotlin_C_CPP/Kotlin_C_CPP_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/Kotlin_C_CPP/Kotlin_C_CPP_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/Kotlin_C_CPP/Kotlin_C_CPP_跨语言边界工程清单.md](../references/legacy-boundaries/Kotlin_C_CPP/Kotlin_C_CPP_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/Kotlin_Rust/Kotlin_Rust_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/Kotlin_Rust/Kotlin_Rust_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/Kotlin_Rust/Kotlin_Rust_跨语言边界工程清单.md](../references/legacy-boundaries/Kotlin_Rust/Kotlin_Rust_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/Kotlin_Swift/Kotlin_Native_Swift_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/Kotlin_Swift/Kotlin_Native_Swift_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/Kotlin_Swift/Kotlin_Swift_跨语言边界工程清单.md](../references/legacy-boundaries/Kotlin_Swift/Kotlin_Swift_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/README.md](../references/legacy-boundaries/README.md) | 新增归档入口、权威优先级、共同规则和后续维护要求。 |
| [references/legacy-boundaries/Rust_C_CPP/Rust_C_CPP_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/Rust_C_CPP/Rust_C_CPP_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/Rust_C_CPP/Rust_C_CPP_跨语言边界工程清单.md](../references/legacy-boundaries/Rust_C_CPP/Rust_C_CPP_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/Swift_C_CPP/Swift_C_CPP_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/Swift_C_CPP/Swift_C_CPP_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/Swift_C_CPP/Swift_C_CPP_跨语言边界工程清单.md](../references/legacy-boundaries/Swift_C_CPP/Swift_C_CPP_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/Swift_Rust/Swift_Rust_架构总结与新版_mad-boundary_设计输入.md](../references/legacy-boundaries/Swift_Rust/Swift_Rust_架构总结与新版_mad-boundary_设计输入.md) | 迁移并重整：当前路线与决定、关键历史经验、具体协议流程及现有任务入口；删除旧 P0–P3 计划和重复模块方案。 |
| [references/legacy-boundaries/Swift_Rust/Swift_Rust_跨语言边界工程清单.md](../references/legacy-boundaries/Swift_Rust/Swift_Rust_跨语言边界工程清单.md) | 迁移并修订：保留每条问题、手段、历史样本和验收；对齐主线/扩展、工程配方与生命周期，移除本机链接和重复尾部。 |
| [references/legacy-boundaries/snapshots/2026-10-24-before-refresh.zip](../references/legacy-boundaries/snapshots/2026-10-24-before-refresh.zip) | 封存原始 18 份文档、旧来源映射及逐文件摘要清单；本机路径只保留在历史快照。 |
| [reviews/2026-10-24-boundary-archive.md](../reviews/2026-10-24-boundary-archive.md) | 记录全部迁移、修订、降噪和实际验证范围。 |
| [reviews/README.md](../reviews/README.md) | 登记当前归档报告，原 Agent 规范报告转为历史导航。 |
| [reviews/boundary-archive-validation.json](../reviews/boundary-archive-validation.json) | 记录来源保留、篇幅变化和检查结果；不提升产品状态。 |
| [tools/check_docs.py](../tools/check_docs.py) | 补充来源编号/标题/行号/完整覆盖及封存 ZIP 清单、摘要和原问题映射检查；不提取执行快照。 |
| [tools/tests/test_document_contracts.py](../tools/tests/test_document_contracts.py) | 新增五项检查：有效归档、陈旧行号、缺失/重复映射、已知问题替换、损坏快照。 |
| [visuals/README.md](../visuals/README.md) | 当前文档检查不再依赖 --reference-root 或旧工程目录。 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 重新生成目录说明；其他十二页重生成后内容未变。 |

## 实际验证

- 修改前静态检查通过；修改后在不包含顶层九组资料的独立暂存 docs 中检查通过，未使用 --reference-root。
- 85 份 Markdown、32 份 JSON、215 个表格、1,432 个本地链接与 13 个 HTML 静态检查通过；生成一致性与脚本语法通过。
- 21 项文档工具回归通过，其中新增 5 项来源与快照保护检查。没有把静态通过当作真实浏览器或产品运行结果。
- 236 个问题的来源编号、标题、行号与文件摘要一致；原问题映射与封存映射一致；原始 18 份文件及旧映射逐字节保留。
- 工作包、路线、公共问题、基础案例组、技术栈候选和既有历史报告保持原字节。十三页重新生成，仅架构总览的目录文字发生变化。
- 当前阅读文档没有作者机器绝对路径或旧顶层目录链接；封存原文保留原有定位信息，仅用于历史追溯。

机器记录：[boundary-archive-validation.json](boundary-archive-validation.json)。本次按现行设计对照整理，没有重新执行旧项目或重新查询所有上游版本；真实浏览器布局、产品编译、FFI 与设备测试均未执行。文档通过不构成任何路线认证。
