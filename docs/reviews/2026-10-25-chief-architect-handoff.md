# 主总架构师接管交接

日期：2026-10-25（America/New_York）。用途：把本会话负责的架构、交付统筹、文档维护和后续开发协调交给另一位 agent。本文是指定输入的交接快照，不是第二份计划、状态目录或新的验收决定。接手后以实际文件和[工作包目录](../catalog/implementation-plan.json)复核，变动写回既有权威入口，不持续维护另一套交接进度。

可直接发送的[接管提示词](2026-10-25-chief-architect-prompt.md)与本文配套。

## 本次记录与输入

任务：既有 C07 后续执行交接及 L01/I/E/L 工作线的协调准备。依据：当前实施计划、相关 ADR、工程规范及已授权的 Swift/ArkTS 扩展范围。修改范围仅本文、提示词、输入快照和 reviews 索引；不开发功能、不改状态或依赖、不改历史证据、不提交、推送或发布。

| 工程 | 本机位置 | 本次捕获 |
|---|---|---|
| MadB 工具与权威文档 | `/Users/changyue/Desktop/MadB/madb` | HEAD `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`，分支 `codex/c07-arkts`；工作区非干净，包含此前文档、检查器和观察首片改动 |
| MadB-Lab 独立验收 | `/Users/changyue/Desktop/MadB/madb_lab` | HEAD `a466501275bee1786d171199390f228f55eb38a0`；捕获时干净 |
| 本聊天工作目录 | `/Users/changyue/Desktop/MadDemo` | 参考工程；不是本轮 MadB 产品代码或权威文档根 |

[输入记录](chief-architect-handoff-input.json)保存采集时间、两仓库分支/状态、受检源文件摘要及 C07 原报告摘要复核结果。捕获副本前后核对文件集合、字节摘要、HEAD 和工作区状态一致；它不证明捕获之后没有其他修改。当前目录仍使用设计基线 `0.9`，C07/ADR-0013 的局部实施契约标记为 `0.10`，不能只用一个版本标签认定全部输入相同。

**不得只基于 HEAD 建新工作树后开始开发。** 当前尚未提交的 ADR-0012/0014–0017、架构17、计划修订、Swift/ArkTS 观察源码与检查器是接管输入的一部分。普通 git worktree 不会带上它们。先辨认所有权和并行写入者，保留这些字节；需要隔离时显式选择、核对并带入所需改动，不能 reset/clean 或把整个脏工作区当作自己的成果提交。

本机路径只用于定位；产品、Lab 独立消费和公开文档取得方式仍遵循既有规范，不把本机相邻源码或缓存作为生产依赖。

## 接管职责与授权

新负责人承担主总架构师、工程交付负责人和文档审查者职责：选择可执行子范围、安排并行分工、统一公共契约、集成最终候选、安排独立验证、维护计划与真实状态，并向项目所有者报告结果。

以下事项已有授权，不必重新逐项确认：

- Swift 必要能力与九项扩展按 [ADR-0014](../adr/0014-swift-production-baseline.md)、[ADR-0015](../adr/0015-swift-extension-delivery.md)实施；ArkTS 十二项必要能力与十二项扩展按 [ADR-0016](../adr/0016-arkts-production-baseline.md)、[ADR-0017](../adr/0017-arkts-extension-delivery.md)实施，技术执行方决定具体顺序。
- 一个主协调任务统筹，负责人按接口、文件和设备资源决定并发数量；优先委派子 agent，参见[并行执行与交接](../delivery/01-implementation-plan.md#c07-后的并行执行与交接)。工作线不等于必须另建聊天；本次只制作交接材料，没有替用户创建或发送新聊天。
- E05 保留 S1 开工、整包 S5 收口，依赖 E04 与 L03–L06；L03 保留 S2 开工、整包 S3 收口，增加 C07 前置。此调整已经批准并写入目录，见[修订记录](2026-10-25-e05-l03-scheduling.md)，不再作为待批准方案。
- C06/C07 已按完整本地验证和用户确认验收。GitHub Actions 改为项目完成后的独立补跑专项；待跑与 cancelled 不能写成通过。当前不启动专项，也避免 push/PR 触发现有工作流；发布、分支保护和最终验证要求仍适用。
- 既定范围内的内部实现、针对性修复、分工和实施顺序可自主推进。新增产品范围、改变既定公共 API/ABI、所有权、线程、取消语义、支持承诺或整包验收条件，按既有流程提交具体差异、影响和推荐方案，不能借“主总架构师”角色自行裁定。

消费者可选与项目交付承诺是两回事：已经选定并纳入工作包的配置仍须完成。允许提前做隔离原型不等于整包验收前置已满足；实验后端不继承主线认证。不要为保持41包而隐藏真实缺口，也不要为管理并行新增调度框架或任务系统。

## 权威入口与阅读顺序

1. 两仓库 `AGENTS.md`，MadB [共同开发方法](../engineering/07-agent-development.md)；Lab 先读自己固定版本的只读 reference 和元数据。
2. [任务目录](../catalog/implementation-plan.json)、[实施计划](../delivery/01-implementation-plan.md)、[决策与风险](../delivery/03-decisions-and-risks.md)。目录决定任务状态、依赖和验收；计划解释阶段及联合交付。
3. [模块与端口](../architecture/02-modules-and-ports.md)、[生命周期与并发](../architecture/05-lifecycle-and-concurrency.md)、[生成](../architecture/07-code-generation.md)、[构建分发](../architecture/09-build-packaging-release.md)、[开发体验](../architecture/15-developer-ergonomics.md)、[跨路线扩展组合](../architecture/17-extension-composition.md)，再按本批任务 documents 追踪详细规范和 ADR。
4. [质量门禁](../engineering/05-review-and-quality-gates.md)、[Lab 架构](../validation/02-lab-architecture.md)、[初始化验收](../validation/06-project-initialization.md)、[语言体验验收](../validation/07-developer-ergonomics.md)。
5. 本文引用的 C06/C07、Swift/ArkTS 首片、全量审查与阶段修订记录用于追溯指定快照。历史结论不覆盖较新的决定。HTML 仅为生成阅读视图，维护方法见[视图说明](../visuals/README.md)。

无需重新复制完整的 Swift/ArkTS 清单；ADR 已保存选型、取舍与单项出口。文档、实现和证据冲突时记录具体差异，不能仅挑对开发方便的一份作为依据。

## 已交付的架构与治理结果

现有方案继续采用六个核心 crate 与外部适配器分层。ContractIR、ProjectPlan、TaskGraph 各有职责；工具构建调度与应用 SDK 运行时生命周期分开。SDK 普通接口优先，业务 Facade 按受支持声明生成；通用资源/线程适配可手写维护，不把固定 SampleEngine 复制当作通用生成。

跨路线扩展通过既有注册、进程协议、语义协商及生成/构建入口接入；未启用的 UI/响应式配置不传播运行依赖。同一接口的转换、协程桥接和订阅各有明确负责人，不增加平行运行时或让两个桥重复接管。

本会话已完成跨路线复杂度与扩展设计整理、Swift/ArkTS 选型和完整实施映射、观察首片、现行文档/展示审查，以及 E05/L03 阶段修订和并行交接安排。具体落点：

| 工作 | 入口与边界 |
|---|---|
| 架构复杂度、角色使用面与扩展组合 | [复杂度记录](2026-10-25-architecture-simplification.md)、[扩展组合记录](2026-10-25-extension-composition-design.md)；不另建万能模型或扩展框架 |
| Swift 观察首片 | [记录](2026-10-25-swift-extension-delivery.md)、[源码入口](../../sdk/swift/observation/README.md)、[验证入口](../../tests/integration/l04_swift_observation/README.md)；macOS 普通观察/Observation 与固定 SKIE 开发组合，不代表 iOS/完整业务生成/公开发行 |
| ArkTS 观察首片 | [记录](2026-10-25-arkts-extension-delivery.md)、[源码入口](../../sdk/arkts/observation/README.md)、[正式 SDK 编译入口](../../tests/integration/l05_arkts_observation/SDK_README.md)；宿主生命周期检查和正式 HAR/HAP 编译，不含该首片的 Native DSO/页面目标运行证明 |
| 全量文档审查 | [报告](2026-10-25-current-document-audit.md)及所链 JSON；修过需求映射、文字依赖环、门禁样本、状态展示和过期说明。其旧 C06/C07 状态及五项待裁定不能原样当作当前状态 |
| 阶段与依赖、并行安排 | [E05/L03 修订](2026-10-25-e05-l03-scheduling.md)、[实施计划](../delivery/01-implementation-plan.md)；精确依赖以目录为准 |

## 接管时的真实开发状态

下表是本次目录快照，后续只更新目录，不更新本文来维护实时状态。

| 工作包 | 本次状态 | 不能由此推导的结果 |
|---|---|---|
| C01–C05 | implemented / passed | 不代表全部路线、平台或公开 SDK 完成 |
| C06 | implemented / passed | 证据属于固定 SampleEngine 的四个 macOS Arm64 Release 配置；54项必需案例包括51项运行和3项编译/链接拒绝。其他 Apple 目标、便利层和新源码回归另验 |
| C07 | implemented / passed | 8个普通/伴随候选、120必需组、195实际HAP、16生成正反例；不代表 L05、物理设备、生产签名、任意路径重放或公开渠道发布 |
| C09、C10、L01、L04、L05 | in_progress / missing | 已开工或局部成果不能计整包验收 |
| 其余工作包 | not_started / missing | 设计采用、提前探针和目录存在不代表已经交付 |

C07 最终生产者 `18256fcc4b26cf3cd1e2e79c39b86c2512e012d9`，Lab `a466501275bee1786d171199390f228f55eb38a0`；与当前工作区 HEAD 不同。证据以[收口记录](2026-10-25-c07-arkts.md)和[收口 JSON](c07-local-closeout.json)为准，本次仅复核其8份原 report.json 字节摘要匹配，没有重跑195个HAP或独立重判120组。

C07 编译使用正式 SDK 26.0.0.105，运行目标为 API23 Release Arm64 模拟器，社区调试签名。Kotlin 主 env 显式关闭不等于主 VM 物理销毁；原 Rust 外层监测会话退出码未观察到，独立重验成功不能补造该原始退出码。归档位于 `/Users/changyue/Desktop/MadB/.local/evidence/c07-18256fc-a466501`；本次确认文件存在，未重新计算整个归档摘要。完整历史重放仍有原路径依赖，不能把本地归档称为已公开发布。

同 env 顺序重开现已由 [ADR-0013](../adr/0013-arkts-runtime-and-completion.md)采用，受 physical ACK、最终 receipt 消费及不复用 epoch 约束；不再作为未决定的架构方向。当前风险表仍残留“目标验收待完成”描述，与最新收口材料需要按具体场景对账。独立交接复核已定位四路线最终普通报告中的 `ordinary-runtime-reopened` 观察，后续重点核对伴随 ACK/receipt 前拒绝及旧对象隔离的逐项映射。接手时核对普通/伴随报告中的重开、旧对象隔离及 ACK/receipt 前拒绝，不仅凭总体 passed 删除该条，也不因旧文字把整个 C07 回退为 missing。

## 接手后的第一批工作

先做增量接管核对，不重开一轮没有边界的全面设计。读取实际 HEAD/脏文件/活动工作树及正在写入的任务，确认本文输入之后的差异；保护 C07 冻结候选和历史记录。完成下面的任务选择后，在已授权范围内继续实施，不只输出“可以继续”。

| 工作线 | 第一批可执行子范围 | 出口与协调 |
|---|---|---|
| 公共契约主线 | L01 剩余契约、受支持声明子集、生成/手写分工、普通 API 与依赖边界 | 形成可供语言与工程消费的具体接口、正反样例、验收映射；以目录 acceptance 判断是否整包完成 |
| 工程接入 | I01 模板契约和 IDE 探针，之后 I02 → I03 | 只基于已确定接口并行；作者源码协作与发行消费者分开。I04/I05 等待各自实际前置，不跳过 Wrapper 与正常 IDE 构建 |
| 扩展底座 | E01–E03 的注册、进程协议、语义协商 | 公共字段由一位负责人收敛；优先复用已有 DTO、协议样本及端口，不能只有 JSON 示例而无真实实现/拒绝证明 |
| Swift / ArkTS | L04/L05 既有观察首片与 C06/C07 固定公开候选的接入核对；只推进契约已确定部分 | Swift 后续 UI/工程/便利层按 ADR；ArkTS 优先补真实 Native 与页面运行。第二业务模块的声明增删改与生成是主线验收重点 |
| 独立原生与验证 | 资源足够时安排 C08 前置分析/实现，C09/C10 随子范围供给消费者和候选 | 不抢占同一编译输出/设备；独立 Lab 不引用生产者私有源码或共享生产预期 |

L02 是公共声明复用与诊断线；L03/L04/L05 是各语言实现线。所需 L01 契约明确后可按实际接口并行，不把 L02 整包人为加成 L03 的新前置。L03 的 CPF CMP 需要 L05/I05 联合材料；不能只做 Kotlin 基础 API 就整包验收。

后续仍按既有队列推进：I01 → I02 → I03 → I04 → I05 → I06；E01–E03 → E04 → E05 → E06；C08 → L06（同时依赖 L01）；C09/C10 持续跟进，L07 在语言与 I06 就绪后收口，C11 满足全部正式前置。精确 depends_on 比队列箭头更细，以目录为准。X 系列正式验收依赖 C11，I07 依赖 I06，L08 依赖 L07；不能额外把 I07/L08 都改成依赖 C11，也不能把 X 原型放到主线完成的隐式等待链上。

并行数量由负责人决定，不要求同时开完所有工作线。优先将公共契约的未定输入收敛，再扩大独立模块并发。每批明确一个集成负责人、各任务允许修改路径和检查条件；schema/IR、公共端口、共享 runtime、工具锁、导出入口和 catalog 各设单一写入负责人。设备、模拟器、共享缓存及构建输出要隔离或串行使用。

## 仍需裁定与实证补齐

当前[风险入口](../delivery/03-decisions-and-risks.md#本轮文档审查待裁定项)保留两项实质决定，不因角色移交自动授权改约：

1. L04 与 C08/L06 的 Swift 验收责任。推荐列明 C06 通用层通过项、后续原生路线接线与必须复跑的 Swift 用例，L07 汇总组合；尚不能机械给 L04 加整个 C08 前置，也不能假定 L06 包办所有 Swift API/UI 验收。
2. 目标 CLI 的 doctor、verify、lock --update、explain 交付归属。现有建议依次评估 I02、C09、C10、L02；需先提出用户可见输出、拒绝条件及 acceptance 差异，再按决策流程落实。不要误称现有 CLI 已实现，也不要撤销 C04 原范围的历史验收。

其余为实施或证据任务：L01 完整子集、真实业务生成、正常 IDE 构建、Swift 完整目标/包装、ArkTS 首片 Native/UI 运行、干净环境公开包消费、C08 其余路线、完整兼容/性能/供应链矩阵，以及项目完成后的 Actions 专项。它们应归既有包逐项交付，不能用“全部扩展已写入计划”替代工程结果。

## 真实检查与交付纪律

在仓库根按版本文件配置环境。本次文档使用 Python 3.14.4、Node 26.8.2；ArkTS 工具套装自己的 Node/Java/Hvigor 与仓库 Node 用途不同，不能全局替换。开始目标工作时重新检查实际工具锁和目标，不凭本文假设工具或设备仍可用。

文档检查入口：

```sh
.venv/bin/python -B docs/tools/check_docs.py
.venv/bin/python -B docs/tools/render_visuals.py --check
.venv/bin/python -B -m unittest discover -s docs/tools/tests
node docs/tools/tests/panorama_smoke.cjs
```

产品实现按受影响风险运行针对性门禁及适用的 `scripts/check.py`；Lab 使用自己根 AGENTS 及固定 Python 的检查入口。生成器要重生成并独立消费；原生所有权、关闭/取消、并发要有目标证据和独立复核。子任务通过之后，集成候选仍需验证；源码改变不能继续冒用旧候选证据。

本次交接的检查结果记录于输入 JSON 的 validation 字段。静态/生成检查与文档工具测试证明本次材料的相应一致性，不是重做全产品验收；未改 HTML 模板/数据，已有 HTML 只检查生成一致性。没有执行新的目标运行、远程 CI、发布或真实浏览器检查。此前浏览器工具受环境禁用限制，不能把 DOM 替身写成浏览器通过；接手后根据自己的实际环境判断，不绕过限制。

新增状态只写 catalog，展示改源数据/模板后生成。历史报告保留当时结论，新结论另记。不得通过删测试、放宽断言、降低验收或改写历史制造全绿。每次交付报告实际修改、命令与结果、输入/候选身份、未覆盖范围及下一步；未获新增事实时不继续无边界重跑。

用户沟通偏好：中文，结论清楚，给出可执行结果；已有授权内自主完成，避免反复询问“是否继续”；重大决定说明依据和影响；不给无依据的日期或工期。禁止使用 archify。
