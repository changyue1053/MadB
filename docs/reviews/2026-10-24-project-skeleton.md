# MadB 0.9 工程骨架落地

日期：2026-10-24。范围为设计文档、计划、图示数据、目录投影工具和生成 HTML。设计基线保持 0.9，本次不创建产品 Cargo workspace、SDK 实现或可安装制品。

## 设计结果

- MadB 工具骨架以 architecture/02-modules-and-ports.md 为唯一维护入口；MadB-Lab 以 validation/02-lab-architecture.md 为唯一维护入口。扩展与初始化说明改为引用及职责映射。
- 工具宿主采用六个核心 crate；应用拥有端口，CLI bootstrap 组装实现，host 控制受管 IO、进程及最终提交。逻辑目录不强制逐个拆 crate，实际成员随构建清单和工作包登记。
- 补全 model/artifacts 值模型归属，分开 ContractIR、ProjectPlan、TaskGraph，以及 wire DTO 与领域值。SDK 不依赖 CLI/app，语言运行时不复用工具进程的生命周期。
- 手写源码、模板、扩展索引、生成批次、构建产物、候选和证据分别明确维护方。首链双向接线、候选消费及初始化资产都有源码落点。
- Lab 按 cases、fixtures、consumers、scenarios、harness、expected、reports 分责；增加组合场景和执行隔离的明确归属，预期不从生产实现生成。
- 架构 HTML 直接抽取并转义规范中的具名目录块；目录缺失、重复或格式不合法时生成失败，避免形成第三份手写目录。

## 计划与质量门禁

C01/C02/C04/C05/C09/E01/E02/I01 的 acceptance/documents 已同步，共 8 个既有工作包。增加的门禁覆盖核心反向依赖、组合根、DTO 转换、适配器权限、SDK/CLI 隔离、Lab 独立性和手写/生成文件归属。

41 个任务的 ID、阶段、依赖、状态、owner、日期与证据不变；DEC-01 至 DEC-12 保持不变。16 条路线、48 个问题、96 个基础组、236 项历史来源映射和技术栈候选未变。文档与 HTML 通过不构成 C01 实现完成。

## 全部文件变化

修改 32 个已有文件，新增 2 个审查/验证文件，删除 0 个文件。历史审查正文与验证 JSON 保持原字节，仅更新导航索引。

| 文件 | 修改内容 |
|---|---|
| [README.md](../README.md) | 增加工具/Lab 骨架入口及权威来源说明；当前变化指向本报告。 |
| [architecture/01-system-architecture.md](../architecture/01-system-architecture.md) | 明确源码组合根及图中调用关系与源码依赖的区别，链接两个目录权威。 |
| [architecture/02-modules-and-ports.md](../architecture/02-modules-and-ports.md) | 作为 MadB 骨架权威：细化目录、六个核心 crate、状态归属、依赖矩阵、端口/DTO、宿主权限、文件归属与首链落点。 |
| [architecture/11-language-extension.md](../architecture/11-language-extension.md) | 删除重复整树，改为引用权威骨架和扩展内容落点；保留权限、独立分发及核心不变约束。 |
| [architecture/14-project-initialization.md](../architecture/14-project-initialization.md) | 修正旧初始化与 Lab IDE 路径，改为用例责任到源码位置的映射，复用 host 和现有工具链。 |
| [catalog/architecture-map.json](../catalog/architecture-map.json) | 同步 9 个节点的职责、组合根与 host/SDK/Lab 边界；保留节点身份、几何、链路与数量。 |
| [catalog/implementation-plan.json](../catalog/implementation-plan.json) | 将骨架建设和验收要求落实到 C01/C02/C04/C05/C09/E01/E02/I01；仅修改 acceptance/documents。 |
| [delivery/01-implementation-plan.md](../delivery/01-implementation-plan.md) | 落实 S0 建仓与依赖检查、S1 核心和首链交付顺序，不维护第二份任务目录。 |
| [delivery/04-design-readiness-and-first-route.md](../delivery/04-design-readiness-and-first-route.md) | 把首链六类资产连接到源码落点与 Lab 分工，区分可编译结构和真实功能验收。 |
| [engineering/01-architecture-principles.md](../engineering/01-architecture-principles.md) | 将职责、依赖倒置与组合根原则定位到实际源码边界。 |
| [engineering/05-review-and-quality-gates.md](../engineering/05-review-and-quality-gates.md) | 补核心依赖、装配、DTO、宿主权限、SDK 隔离、Lab 独立性和文件归属的正反门禁；指定任务与检查落点。 |
| [engineering/06-documentation-quality.md](../engineering/06-documentation-quality.md) | 明确两个目录各自唯一维护入口、HTML 抽取规则和目录规划不代表代码已实现。 |
| [reviews/README.md](../reviews/README.md) | 登记当前骨架报告，将上一份 0.9 清理报告列为历史快照，原记录不改写。 |
| [tools/render_visuals.py](../tools/render_visuals.py) | 从两份规范抽取唯一 text 树并安全转义；缺失、重复、逆序或格式错误拒绝生成。 |
| [tools/templates/architecture-overview.html](../tools/templates/architecture-overview.html) | 增加两工程目录区及源码边界说明，可滚动/折叠、键盘访问；目录由规范生成。 |
| [tools/templates/implementation-progress.html](../tools/templates/implementation-progress.html) | 增加骨架建设阶段入口，链接已有 C01/C04/C05 及规范，不另设进度指标。 |
| [tools/tests/test_document_contracts.py](../tools/tests/test_document_contracts.py) | 新增两个回归测试，覆盖目录结构/转义及缺失、重复、逆序、错误围栏拒绝。 |
| [validation/02-lab-architecture.md](../validation/02-lab-architecture.md) | 作为 Lab 骨架权威：完整目录、七类责任、组合场景、隔离执行、独立预期和报告归档。 |
| [visuals/README.md](../visuals/README.md) | 说明目录区、生成来源、交互入口及计划关联。 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 重新生成骨架目录区与系统职责，两个目录直接来自规范。 |
| [visuals/arkts-c-cpp-architecture.html](../visuals/arkts-c-cpp-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/arkts-rust-architecture.html](../visuals/arkts-rust-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/extensions-architecture.html](../visuals/extensions-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/implementation-progress.html](../visuals/implementation-progress.html) | 重新生成骨架建设入口和相关任务验收，实施状态不提升。 |
| [visuals/kotlin-arkts-architecture.html](../visuals/kotlin-arkts-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/kotlin-c-cpp-architecture.html](../visuals/kotlin-c-cpp-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/kotlin-rust-architecture.html](../visuals/kotlin-rust-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/kotlin-swift-architecture.html](../visuals/kotlin-swift-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/route-atlas.html](../visuals/route-atlas.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/rust-c-cpp-architecture.html](../visuals/rust-c-cpp-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/swift-c-cpp-architecture.html](../visuals/swift-c-cpp-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [visuals/swift-rust-architecture.html](../visuals/swift-rust-architecture.html) | 从更新后的计划和系统节点数据重新生成；保留本路线的配置、机制、交互和认证状态。 |
| [reviews/2026-10-24-project-skeleton.md](../reviews/2026-10-24-project-skeleton.md) | 记录最终设计、逐文件变化、验收结果与未执行范围。 |
| [reviews/project-skeleton-validation.json](../reviews/project-skeleton-validation.json) | 记录本次静态、回归、DOM 和保留性检查；不覆盖上次报告。 |

## 实际检查

- 修改前：静态检查、14 项文档工具回归及 DOM harness 通过。
- 修改后：文档静态检查和 13 个 HTML 生成一致性通过；16 项文档工具回归通过，其中新增 2 项目录抽取检查。
- DOM harness：29 个配置、173 个图筛选、87 个时序场景、3,044 次键盘节点选择，以及深链接、分类、主题检查通过。它不验证浏览器字体或真实布局。
- 保留性检查：任务仅改验收和文档链接；原始历史报告逐字节保留；路线、问题、基础组与候选版本未改。
- 机器记录：[project-skeleton-validation.json](project-skeleton-validation.json)。

浏览器布局、目录滚动/折叠的真实浏览器交互本次未执行；产品源码尚未建立，未执行 Rust workspace 编译、FFI 或设备测试。新增工程依赖门禁是 C01/C04 等任务的实施要求，不是文档检查已证明的结果。

## 实施入口

[MadB 最终骨架](../architecture/02-modules-and-ports.md) · [Lab 骨架](../validation/02-lab-architecture.md) · [架构全景](../visuals/architecture-overview.html#project-skeleton) · [实施计划](../visuals/implementation-progress.html#task-C01)。
