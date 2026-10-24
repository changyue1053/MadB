# MadB 0.9 Agent 开发方法落地

日期：2026-10-24。范围为开发规范、计划和生成阅读视图；设计基线保持 0.9。本次不创建产品源码、实际根 AGENTS.md、CI 配置或新的 Agent 管理系统。

## 决策与计划

共同方法由 engineering/07-agent-development.md 维护：绑定已有工作包及本次子范围，按相关权威规范实施，按风险验证，对照验收交付。范围内实现自主推进；既定架构和契约变化走原有决策流程，已有授权不重复确认。不得以扩大范围、改写规范、削弱检查或虚报结果完成任务。

两个工程各采用简短根 AGENTS.md，实际文件、命令入口和合入约束由 C01 落地；平台门禁随对应工作包补齐。复用现有任务记录、计划、测试和评审，不新增任务 schema、policy/checks JSON、调度服务、独立交接系统或每项任务的强制全量测试。写入方法和计划不表示这些执行约束已部署。

C01 仅增加两项验收和共同规范引用；其余 40 个工作包保持原样。全部任务的编号、阶段、依赖、owner、日期、状态和证据，以及 DEC 准入映射均未改变。16 条路线、48 个问题、96 个基础组、236 项历史映射、技术栈候选与工具生成逻辑未变。

## 全部修改列表

修改 21 个已有文件，新增 3 个文件，删除 0 个文件。13 个 HTML 为已有数据变化后的自动重生成，不新增展示页。

| 文件 | 类型 | 内容 |
|---|---|---|
| [README.md](../README.md) | 修改 | 增加 Agent 阅读与权威入口，当前修订指向本报告。 |
| [architecture/02-modules-and-ports.md](../architecture/02-modules-and-ports.md) | 修改 | 工具目标目录增加根 AGENTS.md，说明仅为规范入口及 C01 交付目标。 |
| [catalog/implementation-plan.json](../catalog/implementation-plan.json) | 修改 | 仅给 C01 增加两项 acceptance 和一份 documents 引用；实际任务状态不变。 |
| [delivery/01-implementation-plan.md](../delivery/01-implementation-plan.md) | 修改 | 明确规范适用于全部工作包，将根入口和执行约束纳入 C01，不另设任务体系。 |
| [delivery/02-open-source-and-contributing.md](../delivery/02-open-source-and-contributing.md) | 修改 | 补 Agent 贡献入口与独立检出要求，沿用现有贡献评审。 |
| [engineering/05-review-and-quality-gates.md](../engineering/05-review-and-quality-gates.md) | 修改 | 将 Agent 任务范围、设计一致性和结果真实性接入已有风险门禁。 |
| [engineering/07-agent-development.md](../engineering/07-agent-development.md) | 新增 | 新增轻量执行规范：任务与设计依据、四步流程、自主范围、变更决策、真实验收及禁止绕过事项。 |
| [reviews/2026-10-24-agent-development.md](../reviews/2026-10-24-agent-development.md) | 新增 | 记录全部变化、实际检查与未实施范围。 |
| [reviews/README.md](../reviews/README.md) | 修改 | 登记本报告，将原骨架报告移入历史导航；原报告内容保留。 |
| [reviews/agent-development-validation.json](../reviews/agent-development-validation.json) | 新增 | 保存本次检查和范围保留证据，沿用文档审查记录，不新增产品或 Agent 协议。 |
| [validation/02-lab-architecture.md](../validation/02-lab-architecture.md) | 修改 | Lab 目标目录增加根 AGENTS.md，强调独立预期、公开候选及真实验证。 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 修改 | 同步两工程的根 AGENTS.md 目标目录及 C01 数据；保持原有布局和交互。 |
| [visuals/arkts-c-cpp-architecture.html](../visuals/arkts-c-cpp-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/arkts-rust-architecture.html](../visuals/arkts-rust-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/extensions-architecture.html](../visuals/extensions-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/implementation-progress.html](../visuals/implementation-progress.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/kotlin-arkts-architecture.html](../visuals/kotlin-arkts-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/kotlin-c-cpp-architecture.html](../visuals/kotlin-c-cpp-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/kotlin-rust-architecture.html](../visuals/kotlin-rust-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/kotlin-swift-architecture.html](../visuals/kotlin-swift-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/route-atlas.html](../visuals/route-atlas.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/rust-c-cpp-architecture.html](../visuals/rust-c-cpp-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/swift-c-cpp-architecture.html](../visuals/swift-c-cpp-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |
| [visuals/swift-rust-architecture.html](../visuals/swift-rust-architecture.html) | 修改 | 从既有生成器同步 C01 数据及计划摘要；保留页面布局、交互和认证状态。 |

## 实际验证

- 修改前的文档静态检查通过。
- 修改后的文档静态检查通过：65 份 Markdown、31 份 JSON、146 个表格、1,117 个本地链接；13 个 HTML 与生成输入一致，脚本语法通过。
- 16 项既有文档工具回归通过；未新增规则引擎或专门的 Agent 测试代码。
- DOM 替身检查通过：29 个配置、173 个图筛选、87 个时序场景、3,044 次键盘节点选择，以及深链接、分类和主题。该检查不等于真实浏览器验证。
- 保留性检查通过：计划仅修改 C01 的 acceptance/documents；任务状态、依赖及 DEC 不变；路线、问题、案例、版本及 18 个既有历史文件逐字节保留；文档工具、模板和 schema 未改动。

机器记录：[agent-development-validation.json](agent-development-validation.json)。真实浏览器布局和交互、产品编译/FFI/设备测试、Agent 规则加载及 CI 合入阻断未在本次执行；C01 验收未因此完成。
