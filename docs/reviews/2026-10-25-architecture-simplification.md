# 架构复杂度、角色体验与文档一致性复核

日期：2026-10-25。设计基线：0.9。源码观察基于 MadB `b9f1e1bce5fdde3b45232b06ac59b7dd6ff90030` 及开始复核时的工作区；进行中的 C07 内容另行保留。本记录是设计与文档审查，不是 SDK、扩展或平台认证。

## 任务范围与处理原则

项目所有者要求总结跨路线扩展讨论，评估总体复杂度、各角色使用成本与合理性，更新现有文档和计划并进行一致性、质量与降噪检查。具体 Swift、ArkTS 必要项和增强项随后另行讨论；扩展何时实施仍由项目所有者决定。

修改限于现行产品、架构、验收、计划及其派生视图。保留九组边界、十六条主线、四十一工作包，保留状态、依赖、阶段、负责人、日期与已有证据。不安装依赖、不实现功能、不修改 C07 代码或验收、不升级格式与工具版本、不改历史报告和 Lab 固定规范快照、不迁移旧 mad/MadDemo。检查覆盖全体文档；只有发现明确缺口的当前规范需要修改。

## 结论与取舍

| 结论 | 依据与处理 |
|---|---|
| 核心划分合理，当前没有足够依据合并核心或增加框架层 | 六个核心 crate 与实际外层适配器承担不同变化原因；ContractIR、ProjectPlan、TaskGraph 及工具/SDK 生命周期继续分离。目录不是独立包或进程的强制清单 |
| 必要复杂度集中在边界正确性，使用复杂度应由工具吸收 | 所有权、线程、停止、排水、批次身份和独立验证保留；普通作者与消费者不手填内部请求、摘要或执行图 |
| 固定业务首链与可扩展业务生成必须分开证明 | 已有真实绑定和生命周期实现不等于受支持的新业务声明可自动生成完整 SDK；第二个独立业务模块成为明确验收实例 |
| 首版扩展组合应有明确边界 | 先交付有限推荐配置、明确依赖和精确锁；复用五个小端口及公共协议工具，不把任意组合求解器或插件市场作为前置 |
| 现有工程需要持续使用流程 | 新建仍由 init 负责；接入、升级、禁用通过受检变更计划，复用文件所有权与事务，保护手写内容和旧有效批次 |
| 构建开销有调查价值，尚无性能结论 | 当前准备实际输出后再冻结重放的路径可见；先量化编译次数、耗时、输入与缓存原因，再判断是否需要执行模型调整 |
| 验证成本通过配置边界和影响分析管理 | 日常按可证明影响选择回归；推荐及发布仍覆盖全部承诺项。两两兼容、缓存命中、结构检查不生成组合认证 |

## 实现证据与限制

| 观察点 | 当前证据 | 不外推的结论 |
|---|---|---|
| 业务声明范围 | [Kotlin/Rust lowering](../../adapters/generators/kotlin_rust/lowering.py) 的 policies 对照固定 semantic surface；[适配器说明](../../adapters/generators/kotlin_rust/README.md)注明受限范围 | 不声称任意业务项目可直接生成 |
| 绑定与 Facade | [Kotlin/Rust driver](../../adapters/generators/kotlin_rust/driver.py)调用后端并复制选定 Facade；[SKIE 构建](../../adapters/generators/skie/build.py)复制通用 Swift 适配源码 | 手写通用原语合理，但固定接口复制不能代替声明驱动的变化部分 |
| 普通 Swift 入口 | [Swift 适配源码](../../sdk/swift/skie/MadBSkieConcurrency.swift)提供句柄、显式 profile 和 native 访问 | 后续普通入口应装配期望身份与业务方法；底层访问需说明被绕过的保证，不把 native close 视为 Swift 投递排空 |
| 构建准备与重放 | [公共 pipeline](../../adapters/toolchains/common/pipeline.py)的 build 先取得实际输出身份，再调用受检构建；metadata 路径还读取面向人的输出 | 可定位优化点；未测得性能瓶颈，未修改 C04 已验收模型，也未交付稳定机器报告 |
| 扩展宿主 | [端口与模块规范](../architecture/02-modules-and-ports.md)区分现有内部端口与目标公开扩展端口 | 目录和 JSON 样例存在不等于 E01–E06 已实现或第三方接入已通过 |

## 角色成本与实施归属

角色应承担的最小知识由[使用流程](../product/03-developer-experience.md)统一维护。优化顺序与既有工作包的映射见[实施计划](../delivery/01-implementation-plan.md)，逐项 acceptance 只维护在[目录](../catalog/implementation-plan.json)。

在 C07 及约定回归收口后，优先完善受支持业务声明、普通 API 和 IDE 构建入口；构建性能先测量；扩展公共支持与已有工程持续使用按依赖交付。这个顺序是技术建议，不替项目所有者安排扩展开工，也不把现有主线要求推迟到 S6。

## 一手资料与推论边界

| 一手资料 | 可复用机制 | MadB 的设计判断 |
|---|---|---|
| [UniFFI 用户指南](https://mozilla.github.io/uniffi-rs/latest/) | 根据导出接口生成跨语言绑定 | 复用真实声明；MadB 补明确语义、必要适配、制品与消费验证 |
| [Gradle 增量构建](https://docs.gradle.org/current/userguide/incremental_build.html)、[Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) | 原生构建系统追踪任务输入输出与重建条件 | 各工具内部编译继续由原生系统负责；跨工具身份与候选提交由 MadB 管理 |
| [Gradle capabilities](https://docs.gradle.org/current/userguide/component_capabilities.html) | 表达依赖提供方冲突与选择 | 可作为构建适配手段，不能代替 ABI、所有权或协程桥责任校验 |
| [HashiCorp go-plugin](https://github.com/hashicorp/go-plugin) | 公共通信与版本机制封装插件接口 | 借鉴通用协议支持与领域实现分离；不据此引入 Go、gRPC 或新插件内核 |

以上是机制参考和架构推论，不表示新库已选定、当前锁已升级或相关 profile 已验证。语言工具候选继续以[技术栈规范](../architecture/16-technology-stack-and-versions.md)为单一入口。

## 降噪与一致性修正
+
+实施计划删除重复的 C01 开工历史、C04 子交付展开和 C05 检查流水，改为当前交接边界与原始证据链接；首页不再复制易过期的检查计数。历史报告、原始研究、公共 schema 和 Lab 固定副本保留不变。
+
+横向复核发现架构总览和进度页仍显示“实现待开始”“待进入 S0”，与目录不符。已在源模板修正，展示日期直接读取 catalog.as_of；实现与验收分别显示。初始化产品页也明确未交付的是 init 入口，避免被误读为整个源码 CLI 尚不存在。
+
+## 文件更新归属
+
+| 文件 | 修改原因 |
+|---|---|
+| product/01-positioning-and-requirements.md | 消除将全部路线概括为未实现的歧义，状态回到目录 |
+| product/03-developer-experience.md | 角色最小知识、简化默认旅程、保留当前显式 CLI 限制和机器结果要求 |
+| product/04-project-initialization.md | 明确 init 尚未交付及既有工程后续使用入口 |
+| architecture/02-modules-and-ports.md | 逻辑职责与物理单元分开，保留三种模型与单一目录权威 |
+| architecture/07-code-generation.md | 原生构建复用、重复执行成本基线及优化前的保证 |
+| architecture/12-extension-protocol.md | 公共协议支持与扩展业务实现分责 |
+| architecture/14-project-initialization.md | 已有工程接入、升级、禁用的计划、所有权和恢复契约 |
+| architecture/15-developer-ergonomics.md | 普通 API 自动装配必要接线和身份，高级入口说明责任 |
+| architecture/17-extension-composition.md | 有限组合、最小增量接入、影响回归与发布验证边界 |
+| adr/0012-extension-composition-and-delivery.md | 补充上述设计取舍，不新增平行架构或实施状态 |
+| catalog/implementation-plan.json | 细化 13 个未整体验收工作包的 acceptance/依据，治理字段保持不变 |
+| delivery/01-implementation-plan.md | 合并历史流水，保留当前交接及明确的技术优先顺序 |
+| delivery/03-decisions-and-risks.md | 将已定位缺口映射到既有工作包与证据边界 |
+| validation/05-language-onboarding.md | 独立扩展作者最小旅程及可复用组件证明 |
+| validation/06-project-initialization.md | 既有工程变更与增量成本实例，沿用 INIT 编号 |
+| validation/07-developer-ergonomics.md | 独立业务声明生成、普通入口与生命周期实例，沿用 DX 编号 |
+| README.md、reviews/README.md | 当前结论入口与历史记录区分，去除重复状态细节 |
+| tools/templates/architecture-overview.html、implementation-progress.html | 修正过时状态文案，日期读取唯一目录 |
+| visuals/ 下十三份 HTML | 从现行目录和模板重生成；无手改输出 |
+| 本报告与 architecture-simplification-validation.json | 保存本次范围、依据、检查与限制 |
+
+其余规范、示例、技术版本、公共格式和案例编号经过横向核对后保留；没有明确缺口的不做措辞性重写。并发推进的 C07 记录和新观察文件只作为检查输入，不覆盖或改写。
+
+## 检查与交付

最终结果随[检查记录](architecture-simplification-validation.json)保存，检查范围分别记录，不合并成产品完成率：
+
+- 全量文档静态检查：Markdown、JSON、TOML、表格、内部链接、共同格式样例、目录关系、研究封存摘要及十三页生成一致性。
+- 21 项独立文档工具测试；DOM 替身覆盖 29 配置、173 链路筛选、87 时序、3044 次节点键盘选择。
+- 真实 Chrome 检查十三页的桌面/窄屏、主题、配置/时序、键盘选择、任务深链接、筛选与进度草稿往返；截图另检查主要改动页面的文字布局。
+- 全部 41 个任务的状态、依赖、阶段、范围、日期、owner 和证据字段保护核对；Lab 固定规范摘要与仓库独立性检查。
+
+实施计划正文由 8766 字符收敛为 6864 字符，约减少 22%；保留约束和证据入口，新增要求写入其负责的规范。35 个文档、目录、模板或派生文件属于本次写回范围；未修改产品实现。
+
+未执行原生 SDK、设备、远端 CI 或发布验收，也未实测构建加速。静态检查不核实全部外部网页的可达性；一手调研只支撑文中引用的机制判断。历史通过结果和本次文档通过都不增加产品认证。
