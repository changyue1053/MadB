# MadB 设计与工程文档

设计基线 0.9 · 更新于 2026-10-25。MadB 为 Kotlin、Swift、ArkTS、Rust、C/C++ 提供跨端 SDK 的契约检查、代码生成、工程接入及制品验证；MadB-Lab 独立验证公开候选和开发者使用流程。

C01–C05 已按各工作包范围验收，覆盖工程与契约基线、受控构建执行及 Rust ↔ C 首链；结果见[C05 记录](reviews/2026-10-24-c05-rust-c-first-route.md)和[审查索引](reviews/README.md)。C06 的固定 SampleEngine 四个 macOS Arm64 Release 配置已完成本机正式构建、54 个必需案例和 92 项 Lab harness 检查；实现与本机验收已交付；按本轮安排，远端 C06 整包验证并入下一轮，当前不计整包验收通过。具体身份与结果见[C06 记录](reviews/2026-10-24-c06-kotlin-swift-rust.md)，当前任务状态以[实施目录](catalog/implementation-plan.json)为准。

尚未发布正式版本；选定 profile 的验收不代表全部目标、工具组合和分发渠道已经覆盖，也不自动更新路线认证登记。范围仍为九组语言边界、十六条主线、四十一工作包；旧 mad、MadDemo 和 mad-boundary 仅提供研究案例，不设迁移任务。

## 阅读入口

| 目的 | 入口 |
|---|---|
| 理解整体架构和全部路线 | [架构全景](visuals/architecture-overview.html) · [路线专题导航](visuals/route-atlas.html) |
| 建立工程与确认源码归属 | [MadB 最终骨架](architecture/02-modules-and-ports.md) · [MadB-Lab 骨架](validation/02-lab-architecture.md) |
| 查阅九组历史研究 | [研究索引](references/history.md) · [归档规则](references/legacy-boundaries/README.md) |
| 使用 Agent 按计划开发 | [Agent 开发规范](engineering/07-agent-development.md) · [质量门禁](engineering/05-review-and-quality-gates.md) |
| 查看实施安排和任务状态 | [实施计划](delivery/01-implementation-plan.md) · [交互进度页](visuals/implementation-progress.html) |
| 确认开工前要交付什么 | [设计准入与首链资产](delivery/04-design-readiness-and-first-route.md) · [十二项实施决定](adr/0009-implementation-contracts-and-gates.md) |
| 查询后端与版本 | [后端配置](architecture/06-backend-profiles.md) · [技术栈、候选及冻结范围](architecture/16-technology-stack-and-versions.md) |
| 使用 SDK 或创建工程 | [开发者体验](product/03-developer-experience.md) · [一键初始化](product/04-project-initialization.md) |
| 查看交付与验收记录 | [C06 四路线](reviews/2026-10-24-c06-kotlin-swift-rust.md) · [C05 Rust/C 首链](reviews/2026-10-24-c05-rust-c-first-route.md) · [C04 整包](reviews/2026-10-24-c04-complete-package.md) · [全部审查记录](reviews/README.md) |

鸿蒙 Kotlin 主线为 CPF-KMP-CMP + akinterop，CMP 只在共享 UI 配置启用；KNOI 独立扩展。Swift 主线采用 Objective-C export + SKIE，NativeCoroutines 组合按 X01 验收。候选版本、上游修复与具体支持范围以技术栈规范和实际 profile 证据为准。

## 按责任阅读

| 角色 | 建议顺序 |
|---|---|
| 产品与架构 | [定位与要求](product/01-positioning-and-requirements.md) → [能力范围](product/02-capabilities-and-roadmap.md) → [系统架构](architecture/01-system-architecture.md) → [阶段准入](delivery/04-design-readiness-and-first-route.md) |
| 核心与生成器 | [模块与端口](architecture/02-modules-and-ports.md) → [身份、策略与 IR](architecture/03-contract-and-ir.md) → [格式实现](schemas/README.md) → [生成事务与缓存](architecture/07-code-generation.md) |
| SDK 作者与平台实现 | [类型与错误](architecture/04-data-and-errors.md) → [生命周期](architecture/05-lifecycle-and-concurrency.md) → [运行时与机制责任](architecture/08-runtime-and-idiomatic-api.md) → [构建与发布](architecture/09-build-packaging-release.md) |
| 语言与 UI 集成 | [语言特性、宏与属性封装](architecture/15-developer-ergonomics.md) → [开发体验验收](validation/07-developer-ergonomics.md) |
| 工程模板 | [初始化架构](architecture/14-project-initialization.md) → [IDE 与模板验收](validation/06-project-initialization.md) |
| 扩展贡献 | [扩展规范](architecture/11-language-extension.md) → [进程协议](architecture/12-extension-protocol.md) → [模型演进](architecture/13-contract-evolution.md) → [接入验收](validation/05-language-onboarding.md) |
| 验证与交付 | [测试策略](validation/01-testing-strategy.md) → [Lab 架构](validation/02-lab-architecture.md) → [案例与报告](validation/03-case-specification.md) → [关键交错](validation/08-critical-scenarios.md) → [开源交付](delivery/02-open-source-and-contributing.md) |

工程规范：[设计原则](engineering/01-architecture-principles.md)、[整洁代码](engineering/02-clean-code.md)、[设计模式](engineering/03-design-patterns.md)、[语言规范](engineering/04-language-guidelines.md)、[评审门禁](engineering/05-review-and-quality-gates.md)、[文档质量](engineering/06-documentation-quality.md)、[Agent 开发规范](engineering/07-agent-development.md)。诊断与预算见[性能规范](architecture/10-diagnostics-and-performance.md)。

## 权威来源

| 内容 | 维护位置 |
|---|---|
| 工具 / Lab 目录与源码依赖 | [模块规范](architecture/02-modules-and-ports.md) / [Lab 规范](validation/02-lab-architecture.md)；HTML 从规范抽取目录 |
| Agent 开发方法 | [Agent 开发规范](engineering/07-agent-development.md)；根 AGENTS.md 只作执行入口，C01 落地实际检查 |
| 行为规则及技术取舍 | 对应 architecture 规范；[ADR](adr/0009-implementation-contracts-and-gates.md)解释决定及替代方案 |
| 问题与基础验收组 | [problems.json](catalog/problems.json)、[case-groups.json](catalog/case-groups.json)；[问题阅读视图](validation/04-problem-catalog.md) |
| 路线、范围与认证 | [routes.json](catalog/routes.json)；语言对不等于完整 profile |
| 工作包、DEC 准入映射和状态 | [implementation-plan.json](catalog/implementation-plan.json)；HTML 从目录生成 |
| 系统和路线图投影 | [architecture-map.json](catalog/architecture-map.json)、`catalog/route-views/`；不另立运行语义 |
| 对外文件结构 | [格式与摘要 v1](schemas/README.md)；共同向量与受检转换维护同一结构权威 |
| 历史问题追溯 | [source-coverage.json](catalog/source-coverage.json)；[研究索引](references/history.md)连接对照修订的阅读版与封存原文，仅供追溯 |
| 真实实现和认证证据 | 固定候选、profile、工具、Lab revision 与案例报告；正文不手抄通过率 |

`examples/` 是设计样本，不是运行证据。Dart E04 验证同步扩展子集，不增加主线路线。未验证事项由[探针与阻断范围](delivery/03-decisions-and-risks.md)跟踪；官方机制资料见[参考资料](references/README.md)，术语见[术语表](references/glossary.md)。

## 完整性与维护

每个承诺问题需有处置规则、使用示例、正反用例和适用条件。生成、运行时协议、检查拒绝及明确约束都可以是处置方式；任意原生指针、算法及业务撤销的正确性不能由通用工具自动证明。

普通消费者只需理解业务 API 的效果；作者负责必要语义，工具封装底层转换、引用与线程接线。普通 API 始终可用，便利语法复用同一协议。

更新规范后同步任务与案例，运行文档检查并重新生成 HTML。具体命令及浏览器验证边界见[视图维护](visuals/README.md)。历史报告保留当时内容，不作为后续修订或平台认证的依据。
