# MadB 设计基线 0.7：路线复核与全部修改

复核日期：2026-10-24。范围为 MadB/docs 全目录，以及本轮引用的历史材料和上游依据。本轮更新文档、目录数据、计划、HTML 模板及生成视图；未实施 MadB CLI/SDK，未执行产品发布。

结论：保留双工程及现有模块架构，修正后端能力、平台构建和正式分发的选型边界。16 条主线、48 个公共问题、96 个基础用例组保持稳定；计划由 38 项增至 39 项，新增 X07/P13 为可选实验。全部工作包仍未开始，已认证路线为 0。

## 1 主要修正

| 主题 | 本轮确定的修改 |
|---|---|
| 鸿蒙 Kotlin | CPF-KMP-CMP + akinterop 为主线；KNOI 是独立互通选择；工具链选择与桥接选择分开表达 |
| Rust 绑定 | Gobley 0.3.7/UniFFI 0.29.4 不再作为完整异步默认组合；必须修复生成模板，再核对对应 runtime 和真实接口能力 |
| Android 工程 | 新 KMP library 插件与独立 app；预编译库采用 jniLibs，需要 externalNativeBuild 时配置独立 Android 模块；测试显式启用 |
| Kotlin 发布 | 官方与 CPF 自首版采用独立 Maven GAV 与根 metadata；该隔离是 MadB 的架构决策，不能误写成上游强制规定 |
| Swift 工程 | SKIE 默认、NC 按声明归属可选；补齐可分发框架、cinterop 可调用性、Swift/C++ 构建方向及正式 SDK 消费约束 |
| 实验后端 | UniFFI-ArkTS 以 X07/P13 独立评估，不替代 napi 默认路线，不扩大 16 条主线或主线认证 |
| 运行正确性 | 分离业务执行、结果转换和完成通知；回调失败不允许触发第二次完成或重复释放 |
| 低门槛 API | 保留普通 API、惯用封装及编译期诊断；C++17 默认与可选 C++20 Concepts 区分 |
| 可追溯性 | 新审查证据单独登记，保留历史 236 条映射；生成器、模板、补丁、运行库和平台包分别记录身份 |

关键依据及候选版本集中在[技术栈选型](../architecture/16-technology-stack-and-versions.md)、[ADR0008](../adr/0008-route-selection-and-distribution.md)与[来源目录](../catalog/source-coverage.json)。候选配置必须经过计划中的探针和独立消费验证，文档复核不等于平台认证。

## 2 全部修改列表

本轮共修改 **51 个既有文件**、新增 **4 个文件**，合计 **55 个文件**；其余 **23 个文件**复核后保留。最终目录共 **78 个文件**。以下按文件逐项列出，HTML 同时列出模板源文件与生成物，避免只改页面后被下一次生成覆盖。

### 入口与架构决策

| 文件 | 处置 | 修改内容 |
|---|---|---|
| [README.md](../README.md) | 修改 | 升级设计基线 0.7；重排选型、ADR、四张图与完整修改记录入口，明确尚未实施和认证。 |
| [adr/0007-technology-stack-and-cpf.md](../adr/0007-technology-stack-and-cpf.md) | 修改 | 统一 CPF 主线、分离版本线和模板修复条件；将平台发布取舍指向 ADR0008。 |
| [adr/0008-route-selection-and-distribution.md](../adr/0008-route-selection-and-distribution.md) | 新增 | 新增六项工程决策：能力维度选型、异步模板、Android 模块、双 GAV、Swift 分发、可选实验。 |

### 架构规范

| 文件 | 处置 | 修改内容 |
|---|---|---|
| [architecture/01-system-architecture.md](../architecture/01-system-architecture.md) | 修改 | 补充运行时、方向、接口能力、交付形态、构建系统五个选型维度及平台适配职责。 |
| [architecture/03-contract-and-ir.md](../architecture/03-contract-and-ir.md) | 修改 | 扩展概念级 Capability 与 profile 约束；区分语言互通、后端能力与具体工程组合。 |
| [architecture/05-lifecycle-and-concurrency.md](../architecture/05-lifecycle-and-concurrency.md) | 修改 | 分开计算、转换、完成通知；禁止成功回调异常触发第二次完成及重复回收。 |
| [architecture/06-backend-profiles.md](../architecture/06-backend-profiles.md) | 修改 | 逐条复核 16 条路线；明确默认、可选、实验方案、适用条件及真实消费者验收。 |
| [architecture/07-code-generation.md](../architecture/07-code-generation.md) | 修改 | 补全 JNI 注册及转换产物、C ABI/CXX 分工、可选 SWIG、UniFFI-ArkTS 配方及模板修复。 |
| [architecture/08-runtime-and-idiomatic-api.md](../architecture/08-runtime-and-idiomatic-api.md) | 修改 | 明确一次完成、结果交接和迟到结果清理，区分业务失败与完成通知失败。 |
| [architecture/09-build-packaging-release.md](../architecture/09-build-packaging-release.md) | 修改 | 补充 Android 原生打包、官方/CPF 独立坐标、SKIE 可分发框架及 Swift/C++ 构建约束。 |
| [architecture/14-project-initialization.md](../architecture/14-project-initialization.md) | 修改 | 补齐 AGP 新 KMP 模块、独立 app、native 接入、显式测试、IDE/宿主和跨平台交付配方。 |
| [architecture/15-developer-ergonomics.md](../architecture/15-developer-ergonomics.md) | 修改 | 补充 SKIE cinterop/lambda/组合诊断、实验后端入口；将 Concepts 限定为可选 C++20。 |
| [architecture/16-technology-stack-and-versions.md](../architecture/16-technology-stack-and-versions.md) | 修改 | 集中修订精确版本候选及证据；撤回旧 Gobley 完整默认组合，拆分 napi/ohrs，补齐平台和宿主条件。 |

### 产品与工程规范

| 文件 | 处置 | 修改内容 |
|---|---|---|
| [engineering/01-architecture-principles.md](../engineering/01-architecture-principles.md) | 修改 | 增加多维选型与生成器/模板/运行库分离原则，避免语言名称直接推导能力。 |
| [engineering/02-clean-code.md](../engineering/02-clean-code.md) | 修改 | 规范完成回调异常边界、生成模板补丁身份和再生成流程，禁止只手改生成物。 |
| [engineering/03-design-patterns.md](../engineering/03-design-patterns.md) | 修改 | 明确策略与适配器承载平台 profile；避免引入无边界的通用工厂。 |
| [engineering/04-language-guidelines.md](../engineering/04-language-guidelines.md) | 修改 | 补充官方/CPF 工具链隔离、JVM/Native 差异及 Swift 公开接口消费检查。 |
| [engineering/05-review-and-quality-gates.md](../engineering/05-review-and-quality-gates.md) | 修改 | 加入模板版本、公开 API 可调用性、双根 metadata、正式分发及 build-only 非运行证据门禁。 |
| [product/01-positioning-and-requirements.md](../product/01-positioning-and-requirements.md) | 修改 | 将能力承诺绑定到具体 profile，明确主线和实验后端边界。 |
| [product/02-capabilities-and-roadmap.md](../product/02-capabilities-and-roadmap.md) | 修改 | 同步默认路线、可选增强、正式交付要求及独立实验范围。 |
| [product/03-developer-experience.md](../product/03-developer-experience.md) | 修改 | 补充真实消费者、发布依赖与诊断体验，避免下载成功被当作可用。 |
| [product/04-project-initialization.md](../product/04-project-initialization.md) | 修改 | 把平台选择规则转化为用户配置体验，补齐工程模块、坐标和 SDK 消费验收。 |

### 计划、验证与目录数据

| 文件 | 处置 | 修改内容 |
|---|---|---|
| [catalog/architecture-map.json](../catalog/architecture-map.json) | 修改 | 同步基线、架构说明与路线工程约束，保留原模块身份。 |
| [catalog/case-groups.json](../catalog/case-groups.json) | 修改 | 保留 96 个基础用例组；追加平台特定场景及审查条目关联。 |
| [catalog/implementation-plan.json](../catalog/implementation-plan.json) | 修改 | 补齐既有工作包验收；新增 X07 可选 UniFFI-ArkTS 实验，共 39 项，实施状态仍全部未开始。 |
| [catalog/problems.json](../catalog/problems.json) | 修改 | 保留 48 个问题；追加本轮具体案例及证据关联。 |
| [catalog/routes.json](../catalog/routes.json) | 修改 | 保留 16 个路线 ID；补充主线、构建、分发与实验配置约束，认证仍为零。 |
| [catalog/source-coverage.json](../catalog/source-coverage.json) | 修改 | 完整保留 9 份历史资料和 236 条映射；独立新增 12 个审查来源、10 个审查条目。 |
| [delivery/01-implementation-plan.md](../delivery/01-implementation-plan.md) | 修改 | 同步 39 项计划及各阶段验收，明确 X07 独立于主线 S5 完成度。 |
| [delivery/02-open-source-and-contributing.md](../delivery/02-open-source-and-contributing.md) | 修改 | 补充上游补丁来源、可再生成要求及独立发布坐标的使用说明。 |
| [delivery/03-decisions-and-risks.md](../delivery/03-decisions-and-risks.md) | 修改 | 具体化模板、Android、发布坐标、Swift 等风险及阻断范围；新增可选 P13 探针。 |
| [validation/01-testing-strategy.md](../validation/01-testing-strategy.md) | 修改 | 加入生成模板、平台装配、正式分发与独立消费验证；区分构建与真机运行证据。 |
| [validation/02-lab-architecture.md](../validation/02-lab-architecture.md) | 修改 | 补充双发布线、独立消费者、平台场景和实验隔离组织。 |
| [validation/03-case-specification.md](../validation/03-case-specification.md) | 修改 | 补齐模板/组件/补丁身份及平台场景证据，避免不可追溯的通过记录。 |
| [validation/04-problem-catalog.md](../validation/04-problem-catalog.md) | 修改 | 将新增选型问题关联到现有公共问题及基础用例组，避免重复扩张分类。 |
| [validation/05-language-onboarding.md](../validation/05-language-onboarding.md) | 修改 | 加入 UniFFI-ArkTS 的独立实验接入、线程/取消/真机和缺环境处理要求。 |
| [validation/06-project-initialization.md](../validation/06-project-initialization.md) | 修改 | 补充 Android 模块/native 打包/测试、双 GAV、Swift 分发及 IDE/宿主失败样例。 |
| [validation/07-developer-ergonomics.md](../validation/07-developer-ergonomics.md) | 修改 | 增加 SKIE 可调用性、声明归属、单次完成及跨机器预生成 SDK 消费检查。 |

### HTML、模板与样式

| 文件 | 处置 | 修改内容 |
|---|---|---|
| [tools/templates/architecture-overview.html](../tools/templates/architecture-overview.html) | 修改 | 更新全景模板及 catalog 驱动的决策卡和任务链接。 |
| [tools/templates/implementation-progress.html](../tools/templates/implementation-progress.html) | 修改 | 更新进度模板；同时显示 route/probe、独立扩展验收、搜索和重复 hash 导航。 |
| [tools/templates/kotlin-arkts-architecture.html](../tools/templates/kotlin-arkts-architecture.html) | 修改 | 更新 ArkTS 专题模板，默认 akinterop，切换 KNOI 时同步节点详情。 |
| [tools/templates/kotlin-swift-architecture.html](../tools/templates/kotlin-swift-architecture.html) | 修改 | 更新 Swift 专题模板，状态和基线由 catalog 提供，明确增强与分发职责。 |
| [tools/templates/shared.css](../tools/templates/shared.css) | 修改 | 调整四图导航、长文本换行及局部滚动规则，降低窄屏溢出风险。 |
| [visuals/README.md](../visuals/README.md) | 修改 | 统一四张图的阅读、维护和生成方式，注明当前基线与尚待浏览器复验。 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 修改 | 重新生成全景；增加六项选型决策及任务入口，同步实际目录数据和状态。 |
| [visuals/implementation-progress.html](../visuals/implementation-progress.html) | 修改 | 重新生成 39 项计划；显示路线与探针、独立扩展验收，修复重复深链接及搜索字段。 |
| [visuals/kotlin-arkts-architecture.html](../visuals/kotlin-arkts-architecture.html) | 修改 | 重绘 CPF+akinterop 主线，保留独立 KNOI 扩展；同步切换详情、双发布线及验证门禁。 |
| [visuals/kotlin-swift-architecture.html](../visuals/kotlin-swift-architecture.html) | 修改 | 重新生成 SKIE 默认与 NC 可选组合；补齐分发、可调用性、反向调用和 C++ 装配边界。 |

### 参考与审查记录

| 文件 | 处置 | 修改内容 |
|---|---|---|
| [references/README.md](../references/README.md) | 修改 | 区分当前决策与历史记录；补充关键上游依据和本地案例入口。 |
| [references/glossary.md](../references/glossary.md) | 修改 | 新增 profile 五维、CPF、akinterop、GAV、根发布和模板补丁身份术语。 |
| [reviews/2026-10-24-final-audit.md](../reviews/2026-10-24-final-audit.md) | 修改 | 明确标注 0.5 历史记录，浏览器检查指向归档结果，避免与当前验证混用。 |
| [reviews/2026-10-24-route-reconciliation.md](../reviews/2026-10-24-route-reconciliation.md) | 新增 | 新增本轮结论、完整逐文件修改清单、保留项及实际校验边界。 |
| [reviews/inventory-0.7.json](../reviews/inventory-0.7.json) | 新增 | 新增全目录逐文件处置及修改前后摘要；自身摘要排除以避免循环。 |
| [reviews/validation-result-0.5.json](../reviews/validation-result-0.5.json) | 新增 | 原样归档旧验证结果，保留历史证据。 |
| [reviews/validation-result.json](../reviews/validation-result.json) | 修改 | 替换为本轮实际结构、schema、引用、生成漂移和静态 HTML 检查结果；明确浏览器复验受阻。 |

## 3 复核后保留的文件

未发现需要改变下列规范边界的依据，因此不为增加修改量而重写。逐文件状态和摘要见[完整目录清单](inventory-0.7.json)。

| 文件组 | 保留原因 |
|---|---|
| ADR0001–0006 | 产品边界、核心/适配器、Lab、语言扩展、初始化、开发体验决策继续成立；新增选型由 ADR0007/0008 承接 |
| architecture/02、04、10–13 | 模块端口、数据错误、诊断、扩展协议和模型演进规则继续适用；新增平台配方落实在后端/生成/发布层 |
| schemas/ 下 3 个文件 | 现有两个 schema 是扩展清单和控制帧结构；本轮没有声称已实现新的 project/profile/lock/report schema |
| examples/ 下 6 个文件 | 示例继续用于结构验证，20 个正反向量及 4 帧协议样本重新校验；不升级为真实运行证据 |
| tools/render_visuals.py | 现有渲染与校验入口能读取新增数据，无需修改核心脚本 |
| reviews/inventory-0.4.json | 历史输入摘要原样保留，当前清单另建 0.7 文件 |

## 4 验证与未决事项

本轮文档与静态检查通过：51 份 Markdown、408 个相对文件链接、112 个表格、16 份 JSON、1 份 TOML；2 个 schema、9 个正例、11 个反例、4 帧协议样本以及四张 HTML 的静态检查均通过，生成物无漂移。当前机器结果见[validation-result.json](validation-result.json)。本轮检查 Markdown 代码围栏、表格与相对文件链接，JSON/TOML 解析，目录 ID/关联/任务 DAG，历史资料摘要，扩展 schema 与正反样本，四张 HTML 的结构、内联脚本语法、静态引用及生成漂移。

HTML 静态检查与浏览器复验分别记录。浏览器工具的 URL 安全策略拒绝本地 file:// 页面，未使用替代协议绕过；本轮未完成布局、交互、窄屏和主题的浏览器复验。模板中的交互修复经过静态代码审查，仍需允许的浏览器环境实际操作确认。0.5 历史浏览器结果仅保存在[归档验证记录](validation-result-0.5.json)，不覆盖本轮。

未运行跨语言 SDK、设备、编译器矩阵或性能测试。修复后 Gobley 的精确发布版/commit、CPF/OHOS 完整能力、Swift 正式分发、双 GAV 消费者及 UniFFI-ArkTS 实验结果仍由相应工作包验证。所有版本锁与认证最终以可复现的构建和 Lab 证据为准。

本轮未删除或重命名文档，未改写历史源材料；原始进度和证据状态保持不变。schema 实现、平台探针与产品工作量在计划中继续明确为待实施。
