# MadB 全文质量复核与全部修改列表

日期：2026-10-24。设计基线：0.8。检查原有 103 份文件；修改 58 份，新增 8 份，保留 45 份；交付后共 111 份。

结论：仍存在与上一轮图形质量相似的问题，主要是机制说明套用模板、映射靠推断、计划缺责任，以及协议没有覆盖完整交接过程。本次已修正规范、计划、案例、描述数据、文档工具并重新生成全部 13 个 HTML。保留既有双工程和模块分层，没有新增产品核心或改变主线范围。

## 主要发现与处理

| 问题 | 修订 | 具体例子 |
|---|---|---|
| 同一模板掩盖机制差异 | 按真实链路补类型、回调、错误与示例；不支持能力直接显示拒绝 | UniFFI-ArkTS 经 Node-API 适配；ANI 不能复用 N-API 调用；Crubit/ObjC++ 不再混入无关语言 |
| 工程服务被画成语言桥 | 重写服务的数据、owner、时序和伪代码 | IDE 先提交源码再在最终路径构建；诊断从业务旁路采集，不接管业务调用；存储提交由宿主仲裁 |
| 工作包和问题关联靠猜 | 使用显式 problem_ids/task_ids 和已知规范引用；拒绝未知或不属于当前配置的归属 | 删除 JavaScript 中按风险文案关键词选择任务的逻辑 |
| 扩展有说明但无独立责任 | 增加 X08/P14 与 X09/P15，补主线/扩展统计和阶段依赖 | KNOI 不能沿用 akinterop 的认证；远程存储不阻塞本地 S5 发布 |
| 生命周期只覆盖部分区间 | 加入源端租约、队列票据、执行器准入、未消费结果清理 | native 回调已返回但 UI 队列仍存活；resume 后取消仍需释放尚未交付资源 |
| 数据与查询副作用说明不足 | 补编码、快照一致性、元数据前置和检查命令边界 | 标准 UTF-8 不能直接当 JNI Modified UTF-8；inspect 不隐式编译，generate --check 不改有效输出 |
| 上游归属/能力和探针错位 | 复核一手资料，纠正社区后端归属、Swift export 能力与独立配方，以及 Swift/C++ 探针 | Swift export 的新协程映射不意味着可复用 SKIE 分发；P06 不能承担 Swift/C++ 探针 |
| 用例占位与断言不对应 | 48 个正例具体化，补 6 组可控交错；示例断言必须有步骤支撑 | 一次完成不等于一次资源消费；杀进程不证明停止成功；两个入口必须证明同一内核 |
| 数量门槛冒充质量、验证层次混用 | 删除固定节点/卡片/代码行数门槛，增加文档规范和可重复工具 | 两节点的有效图可通过；10 项回归和 DOM 替身检查不能冒充浏览器或 SDK 测试 |

## 范围与状态

- 主线仍为九组语言边界、16 条路线；问题 48 个，基础用例组 96 个。
- 工作包由 39 项补齐为 41 项：30 个主线、11 个后续扩展。X08/X09 是已讨论方向的责任补齐。
- 全部产品工作包保持未开始、缺证据；认证路线为 0。文档测试通过不提高产品进度。
- 本次重新核对的是能力、机制、版本配方边界及文档一致性；没有重新安装所有工具或宣称跑完版本矩阵。既有候选版本数值不变。
- 9 份历史来源的 SHA-256、236 条历史映射和原有审查记录均保留，来源事实不改写成已发生的产品事故。

## 全部修改列表

以下每一行对应一个实际变化或新增文件。模板与生成 HTML 分开列出；生成页保留在文档包中便于直接打开。

| 文件 | 操作 | 改了什么 |
|---|---|---|
| [README.md](../README.md) | 修改 | 升级设计基线 0.8；补上最新完整修改清单、质量规范和关键场景入口，历史审查保留为独立快照。 |
| [adr/0007-technology-stack-and-cpf.md](../adr/0007-technology-stack-and-cpf.md) | 修改 | 修正 Swift/C++ 探针关联：由不相关的 P06 改为 P07/P12，并明确 C08 工作包；保留原决策日期与历史基线。 |
| [architecture/03-contract-and-ir.md](../architecture/03-contract-and-ir.md) | 修改 | 规定 ContractSnapshot 新鲜度、MetadataRequired 和显式 metadata 前置图；防止查询隐式编译及生成依赖循环。 |
| [architecture/04-data-and-errors.md](../architecture/04-data-and-errors.md) | 修改 | 补齐 JNI Modified UTF-8、长度溢出和编码边界；快照须同一 revision，并区分不可变值复制与对象身份。 |
| [architecture/05-lifecycle-and-concurrency.md](../architecture/05-lifecycle-and-concurrency.md) | 修改 | 补齐源端租约、排队票据、执行器准入、停止与资源释放两个确认点；拆开终态认领和结果实际消费。 |
| [architecture/06-backend-profiles.md](../architecture/06-backend-profiles.md) | 修改 | 更新 Swift export 当前协程映射、Alpha/集成限制和反向能力拒绝；明确 KNOI 独立 X08/P14。 |
| [architecture/07-code-generation.md](../architecture/07-code-generation.md) | 修改 | 区分只读查询、metadata 准备和 generate --check 的临时复算；保留有效源码与锁。 |
| [architecture/08-runtime-and-idiomatic-api.md](../architecture/08-runtime-and-idiomatic-api.md) | 修改 | 将队列门控、载荷回收和未交付资源清理纳入语言适配职责；宏或便利 API 不改变协议。 |
| [architecture/09-build-packaging-release.md](../architecture/09-build-packaging-release.md) | 修改 | 补齐远程存储的宿主校验/提交权及取消边界；组合消费验证唯一内核，不能只比较结果值。 |
| [architecture/12-extension-protocol.md](../architecture/12-extension-protocol.md) | 修改 | 明确现有 wire 协议范围；存储与 IDE 新 payload 必须另行演进，禁止借 package 请求偷渡能力。 |
| [architecture/16-technology-stack-and-versions.md](../architecture/16-technology-stack-and-versions.md) | 修改 | 同步 0.8 和新的能力说明、KNOI/存储任务；纠正 Apple 探针归属，保留既有候选版本数值。 |
| [catalog/architecture-map.json](../catalog/architecture-map.json) | 修改 | 同步当前基线 0.8；模块与依赖结构保持原设计。 |
| [catalog/case-groups.json](../catalog/case-groups.json) | 修改 | 48 个正例由重复占位改为问题专属场景；增加 CRIT 场景引用，96 组 ID 和执行状态保留。 |
| [catalog/implementation-plan.json](../catalog/implementation-plan.json) | 修改 | 补齐 X08/KNOI、X09/远程制品及 C02/C04/C09 验收；共 41 个任务，保持未开始、缺证据。 |
| [catalog/problems.json](../catalog/problems.json) | 修改 | 同步 0.8；修正 CAP-002 到契约与 IR 规范的关联，48 个问题及状态保留。 |
| [catalog/route-views/arkts-c-cpp-architecture.json](../catalog/route-views/arkts-c-cpp-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；清除 Rust 机制混入，分别呈现 C/C++ 与 Node-API 的注册、线程和队列载荷责任。 |
| [catalog/route-views/arkts-rust-architecture.json](../catalog/route-views/arkts-rust-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；画出社区 UniFFI-ArkTS 的序列化→Node-API→UniFFI 链，和 napi-ohos 主线分开；不支持的同步反向调用直接拒绝。 |
| [catalog/route-views/extensions-architecture.json](../catalog/route-views/extensions-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；重写存储、IDE、诊断各自的数据/时序/owner/示例；修正 IDE 先提交源码再构建，诊断为旁路观察；补 KNOI/存储任务和 Swift export/ANI 能力限制。 |
| [catalog/route-views/kotlin-c-cpp-architecture.json](../catalog/route-views/kotlin-c-cpp-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；区分 JNI/cinterop 与 C++ 包装，补齐 Modified UTF-8、描述符、目标布局和释放器案例。 |
| [catalog/route-views/kotlin-rust-architecture.json](../catalog/route-views/kotlin-rust-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；区分 JVM JNI 与 Native C ABI 的类型/编码、模板交接和构建条件，补充有针对性的失败场景。 |
| [catalog/route-views/rust-c-cpp-architecture.json](../catalog/route-views/rust-c-cpp-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；区分 bindgen、cbindgen、CXX/Crubit 的真实边界，突出符号、布局、panic 和同步回调责任。 |
| [catalog/route-views/swift-c-cpp-architecture.json](../catalog/route-views/swift-c-cpp-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；区分 C/ObjC++ 包装、direct C++ 与反向导出；修正 NSError/Swift 错误归属和发行场景。 |
| [catalog/route-views/swift-rust-architecture.json](../catalog/route-views/swift-rust-architecture.json) | 修改 | 升级描述视图 v3，显式问题/任务/规范关联、构建步骤与伪代码性质；区分 UniFFI 与显式 C ABI，明确资源交付和停止确认；避免把终态一次当作消费一次。 |
| [catalog/routes.json](../catalog/routes.json) | 修改 | 以 extension_work_packages 显式关联 11 个扩展方向及责任；主线仍为 16 条。 |
| [catalog/source-coverage.json](../catalog/source-coverage.json) | 修改 | 仅同步当前目录基线；9 份原始资料、236 条历史映射和既有专项映射内容保持原样。 |
| [delivery/01-implementation-plan.md](../delivery/01-implementation-plan.md) | 修改 | 同步 41 项计划、30 主线/11 扩展分母、P14/P15 和关键验收；S0 探针与 S6 扩展验收分开。 |
| [delivery/03-decisions-and-risks.md](../delivery/03-decisions-and-risks.md) | 修改 | 加入 P14/P15 的未知项、交付和阻断条件，以及本次全文复核结论和报告入口。 |
| [engineering/02-clean-code.md](../engineering/02-clean-code.md) | 修改 | 补充边界示例与生成逻辑的整洁约束：明确责任，禁止通用布尔检查掩盖资源交接和并发协议。 |
| [engineering/05-review-and-quality-gates.md](../engineering/05-review-and-quality-gates.md) | 修改 | 把文档投影、机制差异、所有权示例及真实证据纳入评审；静态通过不代替产品和视觉验证。 |
| [engineering/06-documentation-quality.md](../engineering/06-documentation-quality.md) | 新增 | 新增文档权威层级、路线必答问题和三层检查标准；取消凑节点/卡片/代码行数的门槛。 |
| [examples/case.example.json](../examples/case.example.json) | 修改 | 移除没有对应测试步骤的重入断言；watchdog 超时记 error-with-trace，避免虚报已验证。 |
| [product/02-capabilities-and-roadmap.md](../product/02-capabilities-and-roadmap.md) | 修改 | 明确 KNOI、存储、IDE 和插桩各自工作包，扩展机制通过不等于具体扩展已完成。 |
| [product/03-developer-experience.md](../product/03-developer-experience.md) | 修改 | 明确查询命令只读、元数据缺失/过期的修复路径，以及生成检查的副作用范围。 |
| [references/README.md](../references/README.md) | 修改 | 登记本次复核的 Swift export、JNI、协程取消和社区 ArkTS 后端一手资料；区分上游事实和 MadB 设计。 |
| [references/glossary.md](../references/glossary.md) | 修改 | 增加交付票据、终态认领、描述性配置三个术语，避免把 HTML 配置和已认证 profile 混用。 |
| [reviews/2026-10-24-document-quality-audit.md](../reviews/2026-10-24-document-quality-audit.md) | 新增 | 新增本次发现、完整逐文件变化、保留理由、验证范围和剩余事项。 |
| [reviews/document-quality-validation.json](../reviews/document-quality-validation.json) | 新增 | 保存本次静态、文档工具回归、隔离脚本执行与进度保留检查的真实结果，浏览器及产品测试明确未执行。 |
| [reviews/inventory-0.8.json](../reviews/inventory-0.8.json) | 新增 | 保存全部文件的原摘要/新摘要、状态和逐项说明；自身摘要留空避免循环。 |
| [tools/check_docs.py](../tools/check_docs.py) | 新增 | 新增可重复的全文静态入口：Markdown/JSON/TOML、引用、案例映射、来源摘要、schema、HTML 脚本与生成漂移。 |
| [tools/render_visuals.py](../tools/render_visuals.py) | 修改 | 校验显式问题/任务映射与规范来源；支持可变节点和生命线数量，检查重叠及步骤一致性；删除人为数量门槛。 |
| [tools/templates/architecture-overview.html](../tools/templates/architecture-overview.html) | 修改 | 把社区 UniFFI-ArkTS 后端的归属标注正确，继续从共享目录投影全局架构。 |
| [tools/templates/implementation-progress.html](../tools/templates/implementation-progress.html) | 修改 | 纠正社区后端归属；从共享目录展现 41 项任务与独立扩展进度。 |
| [tools/templates/kotlin-arkts-architecture.html](../tools/templates/kotlin-arkts-architecture.html) | 修改 | 加入 X08/P14 归属以及队列投递、env 退出的生命周期约束。 |
| [tools/templates/kotlin-swift-architecture.html](../tools/templates/kotlin-swift-architecture.html) | 修改 | 补充 resume 后取消和排队投递两个容易遗漏的交接点，链接具体 CRIT 验收场景。 |
| [tools/templates/route-panorama.js](../tools/templates/route-panorama.js) | 修改 | 删除关键词猜任务逻辑；读取显式风险映射、构建步骤和子分区，按生命线数量计算时序布局。 |
| [tools/tests/panorama_smoke.cjs](../tools/tests/panorama_smoke.cjs) | 新增 | 新增隔离 DOM 替身测试：配置、风险/步骤、图筛选、节点键盘、时序、深链接、分类和主题；不是浏览器测试。 |
| [tools/tests/test_document_contracts.py](../tools/tests/test_document_contracts.py) | 新增 | 新增 10 项文档工具回归：错误归属/问题、编译证据、步骤漂移、重叠、重复键及精简图可接受。 |
| [validation/01-testing-strategy.md](../validation/01-testing-strategy.md) | 修改 | 同步 0.8；强调原生工作、队列、消费者和结果分别观察，关联 6 组确定性关键场景。 |
| [validation/02-lab-architecture.md](../validation/02-lab-architecture.md) | 修改 | 补齐 barrier、独立计数、唯一内核及结果交接的 Lab 观测要求，避免用生产状态机当预期。 |
| [validation/03-case-specification.md](../validation/03-case-specification.md) | 修改 | 规定每项断言需要实际步骤支撑；超时与清理失败分类，禁止杀进程就算停止协议通过。 |
| [validation/04-problem-catalog.md](../validation/04-problem-catalog.md) | 修改 | 为全部 48 问题补充具体正例，同步 CAP-002 规范链接，并关联 CRIT 场景；保留原历史映射章节。 |
| [validation/08-critical-scenarios.md](../validation/08-critical-scenarios.md) | 新增 | 新增 6 组前提、可控交错、独立预期与清理规范：队列停止、结果取消、源端 context、唯一内核、查询副作用、取消提交。 |
| [visuals/README.md](../visuals/README.md) | 修改 | 说明 route-detail-view/3、显式映射和可重复检查；更新任务数与维护流程，区分静态、替身和真实浏览器结果。 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 修改 | 重新生成：基线 0.8、共享计划和社区后端正确归属；保留全局模块结构。 |
| [visuals/arkts-c-cpp-architecture.html](../visuals/arkts-c-cpp-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；清除 Rust 机制混入，分别呈现 C/C++ 与 Node-API 的注册、线程和队列载荷责任。 |
| [visuals/arkts-rust-architecture.html](../visuals/arkts-rust-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；画出社区 UniFFI-ArkTS 的序列化→Node-API→UniFFI 链，和 napi-ohos 主线分开；不支持的同步反向调用直接拒绝。 |
| [visuals/extensions-architecture.html](../visuals/extensions-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；重写存储、IDE、诊断各自的数据/时序/owner/示例；修正 IDE 先提交源码再构建，诊断为旁路观察；补 KNOI/存储任务和 Swift export/ANI 能力限制。 |
| [visuals/implementation-progress.html](../visuals/implementation-progress.html) | 修改 | 重新生成：41 项工作包、新扩展归属和验收要求；实施与认证状态保持不变。 |
| [visuals/kotlin-arkts-architecture.html](../visuals/kotlin-arkts-architecture.html) | 修改 | 重新生成：补 KNOI 独立任务、P14 及队列/env 清理边界，保留 CPF+akinterop 主线。 |
| [visuals/kotlin-c-cpp-architecture.html](../visuals/kotlin-c-cpp-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；区分 JNI/cinterop 与 C++ 包装，补齐 Modified UTF-8、描述符、目标布局和释放器案例。 |
| [visuals/kotlin-rust-architecture.html](../visuals/kotlin-rust-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；区分 JVM JNI 与 Native C ABI 的类型/编码、模板交接和构建条件，补充有针对性的失败场景。 |
| [visuals/kotlin-swift-architecture.html](../visuals/kotlin-swift-architecture.html) | 修改 | 重新生成：保留参考页组织，补齐队列交付、及时取消和关键场景入口。 |
| [visuals/route-atlas.html](../visuals/route-atlas.html) | 修改 | 重新生成：同步 0.8 与新的描述视图目录；九组语言边界、十六条主线数量不变。 |
| [visuals/rust-c-cpp-architecture.html](../visuals/rust-c-cpp-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；区分 bindgen、cbindgen、CXX/Crubit 的真实边界，突出符号、布局、panic 和同步回调责任。 |
| [visuals/swift-c-cpp-architecture.html](../visuals/swift-c-cpp-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；区分 C/ObjC++ 包装、direct C++ 与反向导出；修正 NSError/Swift 错误归属和发行场景。 |
| [visuals/swift-rust-architecture.html](../visuals/swift-rust-architecture.html) | 修改 | 由更新后的描述视图和模板重新生成；区分 UniFFI 与显式 C ABI，明确资源交付和停止确认；避免把终态一次当作消费一次。 |

## 检查后保留的文件

保留不是漏检。结构和引用按全文检查，语义按职责复核；历史报告按摘要保留，不重新解释其当次结论。

| 文件 | 保留理由 |
|---|---|
| [adr/0001-product-and-authority.md](../adr/0001-product-and-authority.md) | 原有 ADR 的决策与本次修订一致；保留决定和历史上下文。 |
| [adr/0002-core-and-adapters.md](../adr/0002-core-and-adapters.md) | 原有 ADR 的决策与本次修订一致；保留决定和历史上下文。 |
| [adr/0003-lab-and-release.md](../adr/0003-lab-and-release.md) | 原有 ADR 的决策与本次修订一致；保留决定和历史上下文。 |
| [adr/0004-language-extension.md](../adr/0004-language-extension.md) | 原有 ADR 的决策与本次修订一致；保留决定和历史上下文。 |
| [adr/0005-project-initialization.md](../adr/0005-project-initialization.md) | 原有 ADR 的决策与本次修订一致；保留决定和历史上下文。 |
| [adr/0006-developer-ergonomics.md](../adr/0006-developer-ergonomics.md) | 原有 ADR 的决策与本次修订一致；保留决定和历史上下文。 |
| [adr/0008-route-selection-and-distribution.md](../adr/0008-route-selection-and-distribution.md) | 原有 ADR 的决策与本次修订一致；保留决定和历史上下文。 |
| [architecture/01-system-architecture.md](../architecture/01-system-architecture.md) | 双工程、核心/适配器与构建/运行分层继续适用，无需改动总体架构。 |
| [architecture/02-modules-and-ports.md](../architecture/02-modules-and-ports.md) | 现有小端口和依赖方向可容纳本次补充；没有另造平行核心。 |
| [architecture/10-diagnostics-and-performance.md](../architecture/10-diagnostics-and-performance.md) | 可选诊断与必需保护分离、预算和 Release 对照已有规定，修正展示图来遵循该规范。 |
| [architecture/11-language-extension.md](../architecture/11-language-extension.md) | 新增语言与后端的扩展流程继续适用；具体缺项在 X08/X09 和协议规范补齐。 |
| [architecture/13-contract-evolution.md](../architecture/13-contract-evolution.md) | 未知能力拒绝、公共语义演进和证据失效流程继续适用。 |
| [architecture/14-project-initialization.md](../architecture/14-project-initialization.md) | 已有最终路径提交后构建、取消保留用户工程的准确规则，本次修正图来对齐它。 |
| [architecture/15-developer-ergonomics.md](../architecture/15-developer-ergonomics.md) | 普通 API、宏/注解边界和可选便利层职责保持一致。 |
| [delivery/02-open-source-and-contributing.md](../delivery/02-open-source-and-contributing.md) | 发布、许可、贡献与证据要求继续适用，不以文档完成替代发布准备。 |
| [engineering/01-architecture-principles.md](../engineering/01-architecture-principles.md) | 依赖倒置、单一职责、明确语义和组合原则继续适用。 |
| [engineering/03-design-patterns.md](../engineering/03-design-patterns.md) | 既有模式按职责选用，不因图形质量问题增加抽象层。 |
| [engineering/04-language-guidelines.md](../engineering/04-language-guidelines.md) | 各语言静态检查、unsafe 与惯用资源管理要求仍一致。 |
| [examples/extensions/README.md](../examples/extensions/README.md) | 现有扩展协议范围和正反例仍一致；结构验证通过，本次未扩展 wire schema。 |
| [examples/extensions/dart-ffi.extension.example.json](../examples/extensions/dart-ffi.extension.example.json) | 现有扩展协议范围和正反例仍一致；结构验证通过，本次未扩展 wire schema。 |
| [examples/extensions/protocol-session.example.json](../examples/extensions/protocol-session.example.json) | 现有扩展协议范围和正反例仍一致；结构验证通过，本次未扩展 wire schema。 |
| [examples/extensions/schema-vectors.json](../examples/extensions/schema-vectors.json) | 现有扩展协议范围和正反例仍一致；结构验证通过，本次未扩展 wire schema。 |
| [examples/madb.example.toml](../examples/madb.example.toml) | 项目配置示意与当前范围一致；保持设计示例性质。 |
| [product/01-positioning-and-requirements.md](../product/01-positioning-and-requirements.md) | 产品范围、双工程和角色责任没有改变。 |
| [product/04-project-initialization.md](../product/04-project-initialization.md) | 一键初始化和原生 IDE 可用性仍为既有产品承诺。 |
| [reviews/2026-10-24-atlas-depth-revision.md](../reviews/2026-10-24-atlas-depth-revision.md) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/2026-10-24-final-audit.md](../reviews/2026-10-24-final-audit.md) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/2026-10-24-route-atlas.md](../reviews/2026-10-24-route-atlas.md) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/2026-10-24-route-reconciliation.md](../reviews/2026-10-24-route-reconciliation.md) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/inventory-0.4.json](../reviews/inventory-0.4.json) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/inventory-0.7.json](../reviews/inventory-0.7.json) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/route-atlas-validation.json](../reviews/route-atlas-validation.json) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/route-depth-validation.json](../reviews/route-depth-validation.json) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/validation-result-0.5.json](../reviews/validation-result-0.5.json) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [reviews/validation-result.json](../reviews/validation-result.json) | 历史快照按原字节保留，不把本次审查结果回填到旧记录。 |
| [schemas/README.md](../schemas/README.md) | 现有扩展协议范围和正反例仍一致；结构验证通过，本次未扩展 wire schema。 |
| [schemas/extension-manifest.schema.json](../schemas/extension-manifest.schema.json) | 现有扩展协议范围和正反例仍一致；结构验证通过，本次未扩展 wire schema。 |
| [schemas/extension-wire.schema.json](../schemas/extension-wire.schema.json) | 现有扩展协议范围和正反例仍一致；结构验证通过，本次未扩展 wire schema。 |
| [tools/templates/route-atlas.html](../tools/templates/route-atlas.html) | 布局/CSS/导航模板仍可复用；机制与映射改在描述数据或脚本层。 |
| [tools/templates/route-panorama.css](../tools/templates/route-panorama.css) | 布局/CSS/导航模板仍可复用；机制与映射改在描述数据或脚本层。 |
| [tools/templates/route-panorama.html](../tools/templates/route-panorama.html) | 布局/CSS/导航模板仍可复用；机制与映射改在描述数据或脚本层。 |
| [tools/templates/shared.css](../tools/templates/shared.css) | 布局/CSS/导航模板仍可复用；机制与映射改在描述数据或脚本层。 |
| [validation/05-language-onboarding.md](../validation/05-language-onboarding.md) | EXT 验收继续适用于新补工作包，不将声明能力当认证。 |
| [validation/06-project-initialization.md](../validation/06-project-initialization.md) | INIT 门禁继续适用；最终路径构建和 IDE 检查不降级。 |
| [validation/07-developer-ergonomics.md](../validation/07-developer-ergonomics.md) | DX 的普通入口、便利层和生命周期验收仍适用。 |

## 验证结果与边界

| 检查 | 结果 | 范围 |
|---|---|---|
| 全文静态检查 | 通过 | Markdown 表格/代码围栏/本地引用，JSON/TOML，目录引用和场景编号，9 份来源摘要，扩展 schema/示例，HTML ID/链接和脚本语法 |
| 扩展结构 | 通过 | 2 个 schema、20 个正反向量、4 个会话帧；只验证结构与身份一致性 |
| 生成一致性 | 通过 | 13 个 HTML 与目录、模板、脚本的投影一致；基础图形边界及节点重叠检查 |
| 文档工具回归 | 10/10 通过 | 错误映射、无证据编译声明、步骤漂移、重叠和重复键拒绝；精简图仍可接受 |
| 隔离 DOM 替身执行 | 通过 | 29 配置、173 次图筛选、87 个时序切换、3044 次节点键盘选择，以及主题、分类、深链接和显式映射 |
| 进度与历史保留 | 通过 | 旧任务/路线/问题/案例的状态与证据不变；原始来源、历史映射、历史报告未改写 |
| 真实浏览器复核 | 未执行 | 本会话本地页面访问策略限制；没有运行浏览器布局、真实交互、窄屏和控制台复验 |
| 产品与平台执行 | 未执行 | 没有构建 SDK、运行设备、验证后端、认证工具链或发布制品 |

静态几何和 DOM 替身不能证明字体、连线标签、滚动和窄屏实际表现。真实浏览器视觉验收仍待完成；不得引用此前某一页的检查结果替代本次全部页面验收。

可重复入口，在 docs 目录执行：

```sh
python3 tools/render_visuals.py --check
python3 tools/check_docs.py --node /path/to/node
python3 -m unittest discover -s tools/tests -v
node tools/tests/panorama_smoke.cjs
```

静态检查依赖支持 TOML 的 Python 与 jsonschema，以及 Node.js。旧资料位于工程外；公开发布前须按照许可提供资料快照或公开可定位来源，不能要求读者拥有作者机器上的路径。诊断详情见 [验证结果](document-quality-validation.json)，每份文件的前后摘要见 [完整清单](inventory-0.8.json)。

## 仍须在实施阶段完成

1. 真正执行 P01–P15 的适用探针并冻结具体目标组合；本文和网页不提供这些运行证据。
2. 将 96 个基础组按 profile/平台实例化，尤其执行 CRIT-01 至 CRIT-06；独立预期不能由生产代码计算。
3. 完成 13 页桌面/窄屏浏览器复核与文档包公开可移植性检查，再将视觉和公开分发标为通过。
