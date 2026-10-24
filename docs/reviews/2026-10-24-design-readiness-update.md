# MadB 设计定稿与首链修订记录

日期：2026-10-24。沿用设计基线 0.8。本次把语言特性评估落实为设计准入和既有任务验收；修改 25 份文件，新增 2 份。实际工具与 SDK 实施尚未开始。

## 本次决定

- 主线普通 API、按配置启用的便利层、S6 扩展和 E04 Dart 探针分别描述；保留全部已规划能力。
- S0 可做协议实现、原型与探针；首链正式集成前收敛所需契约；语言便利层以原型和普通用法对照收敛；公开发布建立 API 与兼容承诺的版本基线。
- Rust/C 首链具备 C 消费 Rust、Rust 消费 C 两个直接方向。作者声明、生成接口、端口样本、工具锁、消费者与预期、候选包六类资产形成闭环。
- C09/C10 从 S1 提供首链能力，S5 收口全矩阵；不新建工作包或依赖环，不削弱既有扩展协议、模板、IDE 与运行验收。

## 工作包与状态

细化 C02、C03、C04、C05、C09、C10、E04、I03、L01–L08 共 16 个既有工作包。仍为 41 项任务、16 条主线、48 个问题和 96 个基础组。任务状态与验证证据、依赖和日期保持原样；技术栈候选版本及历史资料不变。

## 全部修改列表

| 文件 | 操作 | 变化 |
|---|---|---|
| [README.md](../README.md) | 修改 | 增加设计定稿与首链资产入口；将此前全文复核明确标为历史快照。 |
| [architecture/10-diagnostics-and-performance.md](../architecture/10-diagnostics-and-performance.md) | 修改 | 区分契约检查、静态属性与动态插桩的覆盖；S1 基础诊断和 S6 深度工具分开。 |
| [architecture/11-language-extension.md](../architecture/11-language-extension.md) | 修改 | 明确 Dart 探针不必启用全部语言技术，build_runner/注解按配方需要选择。 |
| [architecture/15-developer-ergonomics.md](../architecture/15-developer-ergonomics.md) | 修改 | 区分主线基础、按配置便利层、S6 扩展和 E04 探针；明确 KSP、Swift 状态入口、静态属性边界与定稿深度。 |
| [catalog/implementation-plan.json](../catalog/implementation-plan.json) | 修改 | 细化 16 个既有工作包及 S0/S1 展示条件；任务 ID、依赖、阶段、范围、状态、日期与证据不变。 |
| [delivery/01-implementation-plan.md](../delivery/01-implementation-plan.md) | 修改 | 增加分阶段定稿与首链资产安排；明确 C09/C10 在 S1 提供首链能力，S5 再收口。 |
| [delivery/04-design-readiness-and-first-route.md](../delivery/04-design-readiness-and-first-route.md) | 新增 | 新增四个定稿节点、设计成熟度区分、Rust/C 两个直接方向、六类资产及最小正反验收。 |
| [engineering/05-review-and-quality-gates.md](../engineering/05-review-and-quality-gates.md) | 修改 | 将原型、正式集成和公开发布的材料要求分别纳入评审。 |
| [product/02-capabilities-and-roadmap.md](../product/02-capabilities-and-roadmap.md) | 修改 | 说明设计范围不等于默认启用、API 定稿或已认证。 |
| [reviews/2026-10-24-design-readiness-update.md](../reviews/2026-10-24-design-readiness-update.md) | 新增 | 记录本次范围、逐文件修改及真实验证边界。 |
| [tools/templates/implementation-progress.html](../tools/templates/implementation-progress.html) | 修改 | 增加定稿节点和首链资产卡片，复用既有任务状态与深链接机制。 |
| [validation/02-lab-architecture.md](../validation/02-lab-architecture.md) | 修改 | 补 C 消费端及两个直接方向的独立验证，关联首链资产和分阶段 Lab/打包职责。 |
| [validation/07-developer-ergonomics.md](../validation/07-developer-ergonomics.md) | 修改 | 增加集成前的输入/展开、普通/便利用法、依赖、诊断及 DX 材料；未启用语法不能免除公开语义验收。 |
| [visuals/README.md](../visuals/README.md) | 修改 | 指向本次修订；保留前次审查为历史快照。 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/arkts-c-cpp-architecture.html](../visuals/arkts-c-cpp-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/arkts-rust-architecture.html](../visuals/arkts-rust-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/extensions-architecture.html](../visuals/extensions-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/implementation-progress.html](../visuals/implementation-progress.html) | 修改 | 重新生成：增加设计定稿节点、首链资产入口及细化后的任务验收。 |
| [visuals/kotlin-arkts-architecture.html](../visuals/kotlin-arkts-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/kotlin-c-cpp-architecture.html](../visuals/kotlin-c-cpp-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/kotlin-rust-architecture.html](../visuals/kotlin-rust-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/kotlin-swift-architecture.html](../visuals/kotlin-swift-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/route-atlas.html](../visuals/route-atlas.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/rust-c-cpp-architecture.html](../visuals/rust-c-cpp-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/swift-c-cpp-architecture.html](../visuals/swift-c-cpp-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |
| [visuals/swift-rust-architecture.html](../visuals/swift-rust-architecture.html) | 修改 | 同步共享计划中的验收与文档引用；路线拓扑与产品状态不变。 |

## 检查结果

- 全文静态检查、目录/规范引用、计划卡片任务关联和 13 个 HTML 的生成一致性通过。
- 文档工具 10 项回归通过；隔离 DOM 替身覆盖 29 个配置、87 个时序及既有交互路径。替身不是浏览器。
- 原任务的状态、证据、依赖、阶段、范围和日期保持不变；历史报告按原字节保留。
- 未执行产品构建、设备验收或 API 原型；本次形成的是实施要求。
- 浏览器视觉复验受本会话本地页面访问策略限制，未执行；计划页新卡片的实际排版仍需浏览器复核。

静态检查使用 `tools/check_docs.py`，生成漂移使用 `tools/render_visuals.py --check`；两项检查从当前 docs 文件读取，历史审查报告只对各自当次快照负责。
