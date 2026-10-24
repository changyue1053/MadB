# MadB 0.9 实施契约与文档清理

日期：2026-10-24。范围：MadB 设计文档、实施计划、目录数据、文档检查工具和生成的 HTML。此次未实施 MadB 产品代码，未提升任务完成度或平台认证。

## 结果

修改 52 个已有文件，新增 5 个文件；没有删除历史审查或研究证据。16 条路线、41 个工作包、48 个公共问题、96 个基础案例组、9 份历史来源和 236 项映射保留。41 个工作包仍为 not_started，任务的阶段、依赖、责任人和证据字段不变。

十二项实施决定已经进入对应规范、工作包验收和进度 HTML。规范给出约束，catalog 维护准入映射，ADR 说明取舍；它们不构成第二套进度系统。

## 决定与计划

| 决定 | 实施结果 |
|---|---|
| DEC-01 符号身份与策略解析 | 首链集成前；关联 C02、C04；[规范](../architecture/03-contract-and-ir.md) |
| DEC-02 格式权威与语义摘要 | 首链集成前；关联 C02、E01、E02、E03；[规范](../architecture/03-contract-and-ir.md) |
| DEC-03 句柄、关闭与最终释放 | 首链集成前；关联 C02、C05、C09；[规范](../architecture/05-lifecycle-and-concurrency.md) |
| DEC-04 生成、构建与候选的任务图 | 首链集成前；关联 C04、C05、I03；[规范](../architecture/07-code-generation.md) |
| DEC-05 并发提交与崩溃恢复 | 首链集成前；关联 C04、I03、C09；[规范](../architecture/07-code-generation.md) |
| DEC-06 依赖追踪与缓存失效 | 首链集成前；关联 C04、I03、C09；[规范](../architecture/07-code-generation.md) |
| DEC-07 后端与便利层的唯一责任 | 对应配置集成前；关联 L01、L02、L03、L04、L05、L06、X01；[规范](../architecture/08-runtime-and-idiomatic-api.md) |
| DEC-08 执行器、容量与退出 | 涉及调度的配置集成前；关联 C05、C06、C07、C08、C09、L03、L04、L05；[规范](../architecture/08-runtime-and-idiomatic-api.md) |
| DEC-09 共享内核的物理装配 | 组合模板集成前；关联 I05、C09、C10；[规范](../architecture/09-build-packaging-release.md) |
| DEC-10 消费制品与装载匹配 | 对应 SDK 独立消费前；关联 C05、C06、C07、C08、C10；[规范](../architecture/09-build-packaging-release.md) |
| DEC-11 最低完整配置与阻断范围 | 各路线集成及发布前；关联 C03、C06、C07、C08、C11、I06；[规范](../architecture/16-technology-stack-and-versions.md) |
| DEC-12 开源安装、更新与责任 | 公开仓库及分发前；关联 C01、C10、C11；[规范](../delivery/02-open-source-and-contributing.md) |

完整取舍见 [ADR-0009](../adr/0009-implementation-contracts-and-gates.md)，精确验收见[实施目录](../catalog/implementation-plan.json)。修改了验收、文档关联或说明的工作包：C01、C02、C03、C04、C05、C06、C07、C08、C09、C10、C11、E01、E02、E03、I03、I05、I06、L01、L02、L03、L04、L05、L06、X01、X08、X09。

首链自有 C ABI 采用登记令牌，把关闭确认与最终移除分开；既有第三方 ABI 保持自身契约。生成源码、构建产物和候选包分别提交；按实际配置排列任务，避免循环依赖。内核共享必须由物理链接与组合根保证，不能用两个语言各自的 singleton 冒充同一个实例。

首链基准选择 macOS Arm64；其他布局探针不冒充真实目标运行。P03/P05 提前验证平台风险，但仅阻断受影响配置。版本候选数值保持原样，实际可复现锁仍由对应探针产出。Apache-2.0 是项目自有部分的设计默认，首次公开代码前仍要落实实际许可文件和第三方权利核对。

## 清理和保留

- 删除当前入口中重复维护的全量任务表、叠加的旧审查链接、历史更新流水账及重复复核附录。
- 将历史研究导航移到 references/history.md，历史审查集中从 reviews/README.md 进入；原有审查文件逐字节保留。
- 修正 Dart 同步探针被描述为 Future/Stream 通用适配、平台探针全局阻断、源码提交前统一要求完整消费编译三处范围或顺序问题。
- 不删已有路线、扩展、语言便利层或诊断能力。宏/属性封装器的具体名称继续在对应模块集成前确定，公开语义和验收前置。
- 不用文档优化替代实现，不把设计示例标成已编译代码，也不把格式校验结果标成已认证 SDK。

四个入口/计划说明文件从 26,331 字符整理为 16,319 字符，减少约 38%；实施规范增加必要约束。该数字仅反映入口重复内容的压缩，不以全目录字数下降为目标。

## 全部文件变化

### 修改的文件

| 文件 | 修改内容 |
|---|---|
| [README.md](../README.md) | 重排当前规范、角色入口与验收入口；移除重复任务清单、历史修订流水账，直接链接当前审查与归档。 |
| [architecture/02-modules-and-ports.md](../architecture/02-modules-and-ports.md) | 清理历史性的模块补充措辞，保留模块职责、依赖方向、端口与扩展边界。 |
| [architecture/03-contract-and-ir.md](../architecture/03-contract-and-ir.md) | 固定 DEC-01/02：符号身份、精确选择器、策略来源、schema 权威、首链 payload、语义摘要及确定性批次身份。 |
| [architecture/05-lifecycle-and-concurrency.md](../architecture/05-lifecycle-and-concurrency.md) | 固定 DEC-03：首链受管令牌、关闭/等待/释放分工、错误类别和收尾责任；限定为自有 ABI，不改第三方 ABI。 |
| [architecture/06-backend-profiles.md](../architecture/06-backend-profiles.md) | 设计基线更新为 0.9；保留原后端能力与限制。 |
| [architecture/07-code-generation.md](../architecture/07-code-generation.md) | 固定 DEC-04/05/06：按配置安排无环任务；源码/产物/候选分层提交；不可变批次、并发冲突、恢复及传递依赖缓存。 |
| [architecture/08-runtime-and-idiomatic-api.md](../architecture/08-runtime-and-idiomatic-api.md) | 固定 DEC-07/08：每符号、每阶段唯一责任；执行器归属、队列预算、溢出与清理；收窄 Dart 探针描述为既定同步子集。 |
| [architecture/09-build-packaging-release.md](../architecture/09-build-packaging-release.md) | 固定 DEC-09/10：共享内核物理链接与注入、制品匹配和实际装载检查，明确外部库可验证能力的限制。 |
| [architecture/10-diagnostics-and-performance.md](../architecture/10-diagnostics-and-performance.md) | 清理历史补充措辞，保留诊断、插桩与正确性保护的职责分离。 |
| [architecture/14-project-initialization.md](../architecture/14-project-initialization.md) | IDE 普通构建使用对应配置任务图；接入固定生成批次、并发与输入追踪规则；保留新建工程和手写文件边界。 |
| [architecture/15-developer-ergonomics.md](../architecture/15-developer-ergonomics.md) | 将“本次增加的职责”改为稳定的语言体验职责，不改变注解、宏或属性封装器范围。 |
| [architecture/16-technology-stack-and-versions.md](../architecture/16-technology-stack-and-versions.md) | 固定 DEC-11：最低完整配置、首链 macOS Arm64、第二布局探针和局部阻断；去掉重复复核附录，保留候选版本数字。 |
| [catalog/architecture-map.json](../catalog/architecture-map.json) | 设计基线更新为 0.9，保留模块和关联身份。 |
| [catalog/case-groups.json](../catalog/case-groups.json) | 保留 96 个基础组；为相关正反例组关联 CRIT-07 至 CRIT-12。 |
| [catalog/implementation-plan.json](../catalog/implementation-plan.json) | 增加十二项 decision_gates，细化关联工作包验收及 S0/S1 交付；保留任务数量、阶段、依赖、状态和证据。 |
| [catalog/problems.json](../catalog/problems.json) | 设计基线更新为 0.9；48 个问题定义不变。 |
| [catalog/routes.json](../catalog/routes.json) | 设计基线更新为 0.9；16 条路线及原映射不变。 |
| [catalog/source-coverage.json](../catalog/source-coverage.json) | 设计基线更新为 0.9；历史来源、摘要及 236 项映射不变。 |
| [delivery/01-implementation-plan.md](../delivery/01-implementation-plan.md) | 以阶段出口和交付顺序说明计划，移除与 catalog 重复的全量任务表；明确首链局部交付与 S5 全矩阵收口。 |
| [delivery/02-open-source-and-contributing.md](../delivery/02-open-source-and-contributing.md) | 固定 DEC-12：Apache-2.0 设计默认、真实命名空间、GitHub Releases、安装信任、显式升级及坏版本处置责任。 |
| [delivery/03-decisions-and-risks.md](../delivery/03-decisions-and-risks.md) | 区分已采用决定与实施探针；保留 P01 至 P15，明确阻断范围，清除历史审查流水账。 |
| [delivery/04-design-readiness-and-first-route.md](../delivery/04-design-readiness-and-first-route.md) | 将首链资产、准入时点、目标和成功/失败证据连接到十二项决定，明确 C09/C10 从 S1 提供局部交付。 |
| [engineering/05-review-and-quality-gates.md](../engineering/05-review-and-quality-gates.md) | 评审要求核对规范、具体接口或格式、成功用例与失败用例；关联关键场景，避免仅凭文案放行。 |
| [engineering/06-documentation-quality.md](../engineering/06-documentation-quality.md) | 明确规范、计划、示例、HTML 与历史记录各自权威；当前正文不累计修订历史。 |
| [examples/madb.example.toml](../examples/madb.example.toml) | 用显式 symbol_id 演示精确策略绑定；说明 ID 必须对应声明，执行器和容量由配置明确。 |
| [product/01-positioning-and-requirements.md](../product/01-positioning-and-requirements.md) | 清理迭代措辞，保持工具/SDK 作者/消费者责任与产品范围。 |
| [product/02-capabilities-and-roadmap.md](../product/02-capabilities-and-roadmap.md) | 清理补充式措辞，保持主线、扩展和初始化能力边界。 |
| [references/README.md](../references/README.md) | 保留当前一手依据导航，补摘要/依赖追踪依据；历史研究索引迁到独立 history.md。 |
| [schemas/README.md](../schemas/README.md) | 分清现有两个格式 schema 与待实施 payload；外部结构共享格式向量，语义摘要不替代文件字节摘要。 |
| [tools/render_visuals.py](../tools/render_visuals.py) | 校验 decision_gates 的身份、任务和规范链接；安全转义并生成进度页的十二项准入表。 |
| [tools/templates/architecture-overview.html](../tools/templates/architecture-overview.html) | 平台准入改为按受影响配置阻断，避免将 CPF 探针作为所有路线统一前置。 |
| [tools/templates/implementation-progress.html](../tools/templates/implementation-progress.html) | 加入从计划数据生成的决定/时点/规范/任务表，清理历史更新措辞和全局阻断表述。 |
| [tools/tests/test_document_contracts.py](../tools/tests/test_document_contracts.py) | 增加四项回归：未知任务、缺失规范、重复决定 ID、HTML 文本转义。 |
| [validation/01-testing-strategy.md](../validation/01-testing-strategy.md) | 整理测试策略表述，保持风险驱动、独立消费者及真实目标证据要求。 |
| [validation/02-lab-architecture.md](../validation/02-lab-architecture.md) | 清理历史新增说明，保持 Lab 与 SDK 工程分离及候选输入边界。 |
| [validation/03-case-specification.md](../validation/03-case-specification.md) | 补适用性先解析、独立评审和候选/案例摘要要求；禁止后端自行跳过失败。 |
| [validation/04-problem-catalog.md](../validation/04-problem-catalog.md) | 清理重复总结与历史性措辞；保留全部公共问题和基础用例。 |
| [validation/08-critical-scenarios.md](../validation/08-critical-scenarios.md) | 保留 CRIT-01 至 CRIT-06，增加策略身份、关闭释放、并发提交、缓存、满队列退出和错库装载六组可控场景。 |
| [visuals/README.md](../visuals/README.md) | 统一可视化入口与生成检查方式；移除堆叠的旧审查入口，补决定索引与范围说明。 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 重新生成 0.9 全景页；同步平台准入范围和权威计划数据。 |
| [visuals/arkts-c-cpp-architecture.html](../visuals/arkts-c-cpp-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/arkts-rust-architecture.html](../visuals/arkts-rust-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/extensions-architecture.html](../visuals/extensions-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/implementation-progress.html](../visuals/implementation-progress.html) | 重新生成 0.9 进度页与十二项准入表；工作包状态不提升，修正平台阻断范围。 |
| [visuals/kotlin-arkts-architecture.html](../visuals/kotlin-arkts-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/kotlin-c-cpp-architecture.html](../visuals/kotlin-c-cpp-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/kotlin-rust-architecture.html](../visuals/kotlin-rust-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/kotlin-swift-architecture.html](../visuals/kotlin-swift-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/route-atlas.html](../visuals/route-atlas.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/rust-c-cpp-architecture.html](../visuals/rust-c-cpp-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/swift-c-cpp-architecture.html](../visuals/swift-c-cpp-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |
| [visuals/swift-rust-architecture.html](../visuals/swift-rust-architecture.html) | 由权威目录重新生成，嵌入 0.9 计划与映射；保留该页既有路线配置、图示与交互。 |

### 新增的文件

| 文件 | 用途 |
|---|---|
| [adr/0009-implementation-contracts-and-gates.md](../adr/0009-implementation-contracts-and-gates.md) | 登记十二项采用决定及取舍，并链接规则权威、工作包与验收；不增加进度或认证状态。 |
| [references/history.md](../references/history.md) | 承接九份历史研究索引，保留来源定位；不将旧研究结论当作当前产品保证。 |
| [reviews/2026-10-24-implementation-contracts-cleanup.md](../reviews/2026-10-24-implementation-contracts-cleanup.md) | 记录完整文件变化、删除/保留边界、验证范围和未完成的实施资产。 |
| [reviews/README.md](../reviews/README.md) | 建立当前审查与不可变历史记录的分层入口。 |
| [reviews/implementation-contracts-validation.json](../reviews/implementation-contracts-validation.json) | 记录本次实际静态检查、回归检查、DOM 检查和未执行范围，不覆盖历史验证 JSON。 |

## 检查结果

- 文档静态检查通过：62 份 Markdown、29 份 JSON、1 份 TOML、13 个 HTML 及其脚本；本地链接、目录身份、扩展格式与来源映射一致。
- 文档生成器回归检查 14 项通过；原基线为 10 项，本次增加 4 项准入与转义回归。
- DOM harness 通过：29 个配置、173 个图筛选、87 个时序场景、3,044 次键盘节点选择，以及深链接、分类和主题检查。
- 13 个 HTML 与生成源一致；未提升任何工作包状态，既有历史审查逐字节保持，路线/问题/来源映射仅更新设计基线。
- 可复核结果：[本次机器检查记录](implementation-contracts-validation.json)。

检查从 0.8 原始快照开始，使用相同静态检查、单元检查和 DOM harness 对照。新增回归仅针对准入数据和 HTML 投影的失败条件；CRIT-07 至 CRIT-12 是未来产品用例定义，不是本次执行过的测试。

浏览器中的真实布局、实际交互和控制台检查本次未执行；DOM harness 不替代浏览器。未执行跨语言产品测试、设备运行、发布或认证。

本次针对摘要、依赖追踪和发布来源核对一手依据，包括 [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785)、[Cargo 构建脚本](https://doc.rust-lang.org/cargo/reference/build-scripts.html)、[Gradle 增量构建](https://docs.gradle.org/current/userguide/incremental_build.html)、[GitHub 来源验证](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)。这是机制核对，不表示所有候选版本已重新完成兼容测试。

## 实施时仍需交付

按[首链实施设计](../delivery/04-design-readiness-and-first-route.md)取得真实工具锁、schema/端口样本、可编译生成 API、消费者与故障 fixture、候选包及正反结果。各平台随后交付责任表、执行器/容量预算、装载匹配与适用用例；实际 LICENSE、仓库身份及发布资产在公开前落实。上述内容已挂到现有任务，不靠继续添加概念性文档放行。
