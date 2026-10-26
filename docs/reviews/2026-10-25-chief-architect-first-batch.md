# 接管首批：L01 契约、扩展格式与工程计划

日期：2026-10-25（America/New_York）。本记录只保存此次输入、差异和检查；当前状态仍只在[任务目录](../catalog/implementation-plan.json)维护，不建立第二份实施计划。

## 输入与隔离

MadB 根为 `/Users/changyue/Desktop/MadB/madb`，HEAD `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`、分支 `codex/c07-arkts`；独立 Lab 根为 `/Users/changyue/Desktop/MadB/madb_lab`，HEAD `a466501275bee1786d171199390f228f55eb38a0`、同名分支，接管时干净。MadDemo 不作为产品根。

实际读取交接及输入 JSON、两仓库 AGENTS、Lab 固定 reference 元数据/规范、目录、计划、风险及本批规范。交接所列源文件摘要仅 reviews 索引有变化；新增交接正文、提示词和输入 JSON。MadB 原有 69 个 tracked 修改及未追踪 ADR、观察源码、测试全部保留。另有 `/private/tmp/madb-c07-clean-closeout` detached 工作树，HEAD `c8e7e01c31a7bc23abf6716abcc85cb47d47f418`，未改动。

应用内相关聊天在接管核查时均空闲；进程核查未见 MadB 编译/设备任务，Cursor 仍打开 MadB。这只能证明观察时状态。为避免并行覆盖，从原根捕获 964 个 tracked/非忽略 untracked 源文件，复制前后复核文件集合、HEAD、状态和字节一致；在 `/private/tmp/madb-chief-takeover/madb` 的本地独立副本覆盖全部捕获字节后实施，不能只以 HEAD 作为输入。原始字节在同级 baseline 保留。构建和检查输出隔离，最终仅集成本批变化；集成前逐文件核对原根仍匹配捕获内容，冲突时不覆盖。没有提交整批继承改动、push、PR、远程 Actions 或发布。

## 本批职责与实际边界

依据设计基线 0.9、ADR-0012/0014–0017 及 ADR-0013 的局部 0.10 契约。主协调负责共享导出、catalog、风险、集成及最终验证；子范围写入路径不重叠，原始 Lab 和冻结候选只读。

| 工作包 | 本次交付 | 明确保留的整包缺口 |
|---|---|---|
| L01 | [声明契约与输入](../examples/language-api/README.md)、独立业务源码与实际 reader 正反检查；将支持/拒绝、来源、普通/便利、生成/手写和逐配置责任具体化 | 第二业务模块的通用生成尚未实现；固定 C06 策略不能冒充泛化；全部必要能力和增强配置的完整契约/消费证据仍按原验收收口 |
| E01 / E03 | `adapters/contracts/src/extension.rs` 对既有 manifest v1 解码，校验显式包集合、身份、引用、依赖环、端口、逻辑路径；必需语义按现行 CORE_FEATURES 拒绝未知项 | 不安装/授信/授予权限、不补造锁和来源验证；未实现每符号/阶段/实例的完整唯一责任协商、模型演进或证据失效 |
| E02 | `crates/madb-protocol/src/extension.rs` 严格 JSON Lines 帧编解码，复用既有 wire schema、共同向量、字节/深度预算和准确拒绝 | 不是交互进程会话；hello/ack/invoke 次序、运行中 cancel、timeout、输出域/清理与宿主提交仍待实现 |
| I01 | `crates/madb-plan/src/project/` 纯模块 DAG 和文件所有权计划，确定排序、路径/大小写/文件目录冲突拒绝 | 不是模板 wire/渲染器；schema、InitResult、Wrapper、符号链接/无覆盖提交、正常 IDE 探针及成本基线仍缺 |
| L04 | 扩展既有 SKIE 消费者，两个观察模型共享真实 Engine，停止一方后另一方继续，重进与最终资源清理有独立断言 | macOS Arm64 开发组合；不认证 iOS、SwiftUI/UIKit、完整业务生成或正式分发 |
| L05 | 在集成输入重跑现有宿主观察检查，明确真实 Native/HAP/页面接线入口 | 本批没有新增目标页面运行；不把宿主结果或历史 HAR/HAP 编译当作完整 L05 |
| C09 | 对冻结 C07 同 env 重开证据逐项对账，复用既有独立 Lab 规则检查原始观察 | 未重跑目标、没有扩张冻结候选认证；具体关闭窗口补证和全矩阵仍缺 |

内部 Rust 模块是已采用职责的窄实现，未新增 wire 字段、CLI 命令或外部支持承诺。E01/E03 使用现行核心功能 ID；旧设计向量中的 `core.call.sync@1` 等不自动映射成 `core.values@1`。结构有效的 design example 禁止注册；组件自报支持也不能使未知必需功能变成已支持。

## C07 重开证据对账

完整逐项路径与摘要见[对账 JSON](chief-architect-c07-reconciliation.json)。固定 producer `18256fcc4b26cf3cd1e2e79c39b86c2512e012d9`、Lab `a466501275bee1786d171199390f228f55eb38a0`，正式 SDK 26.0.0.105、API23 Release Arm64 模拟器、原社区调试签名。

8 份原 report 摘要、44 项相关附件摘要/大小匹配；四份普通原始日志重新经原 Lab parser 解析通过，相关测试主体与冻结源码一致。这是原始证据复核，没有执行新的 HAP 或重判全部 120 组。

| 路线 | 已定位的真实观察 | 尚未定位的明确目标场景 |
|---|---|---|
| C / C++ / Rust | 普通 `ordinary-runtime-reopened=[1,42,51,3]`，覆盖旧 Runtime/Engine/Operation/Subscription；伴随 partial-open 的 phase=2/read_mode=0 实际走 RuntimeAlreadyOpen 拒绝 | 正常 Runtime.close 未物理 ACK 的重新准入；物理 ACK 后但最终 receipt 未消费时的重新准入 |
| Kotlin | 普通重开及旧公开对象隔离；伴随 epoch 改变、四个旧 key 入口拒绝、当前 waiter `[7,7]` 和新结果42 | 上述两个正常关闭准入窗口；关闭/slot vacant 观察本身不证明尝试重开被拒绝 |

初始化回滚不能代替正常关闭窗口，Engine generation 拒绝不能代替 Runtime 准入拒绝。风险入口已改为具体补证，保持 ADR-0013 已采用、C07 原报告和 implemented / passed 结论。

## 责任决定

项目所有者已在本次聊天分别确认下述 Swift 分责方案和四个 CLI 的归属及验收要求。相关 acceptance 已追加，阶段和依赖均未改变；命令实现与新 wire 字段仍按既有任务和格式流程交付。E05/L03 已批准修订不重开。

### L04 与 C08/L06 的验收责任

**当前差异**：L04 的整包前置是 C06/L01，却同时承担通用 Swift 能力；C08 的 Swift-C/Swift-C++ 路线接入和 L06 原生资源包装不能自动继承 C06 的 Swift 证据，L06 也没有完整 Swift UI/actor 验收职责。

**用户已确认的决定**：保留现有阶段和依赖。L04 收口 C06 配置上的普通 API、Observation/UI 与已选便利层及通用机制；C08 每个新增 Swift-C/Swift-C++ profile 在 native 导入、ABI、异常、方向和包装证据之外，负责接线并重跑适用 Swift DX、关闭/取消、跨 actor、共享 owner 和 UI 配置；L04 维护通用用例与复验入口，C09 执行独立验证。L06 负责 C/C++ RAII、move-only、静态属性和等价 C 后条件，不代替 Swift 便利层。L07 汇总实际候选组合，I04/C10 分别供工程与制品证据。

**已写入 acceptance 的差异**：L04 明示 C06 整包基线与后续原生组合复验责任；C08 增加“对所交付 Swift-C/Swift-C++ profile 接线并执行适用 L04/DX 回归，证据绑定该候选”；L06 明示不包办 Swift API/UI/actor 验收。所有原有条目保留，不机械增加 L04→C08 全包依赖。影响是明确证据责任和路线收口，不增加新路线、减少必需测试或改变阶段。

### 目标 CLI 的归属与可见行为

| 入口 | 用户已确认的责任 | 用户可见输出与失败条件 |
|---|---|---|
| `doctor` | I02 | 选定 profile/检查级别的工具、SDK、锁观察及修复指导，默认只读；未知配方、错版本/摘要/宿主、缺必需工具明确失败；未执行运行检查不显示通过 |
| `verify` | C09 | 固定候选、Lab revision、suite 对应既有 Report/覆盖结果；混候选、未知套件、缺必需案例或清理缺口不得通过；编译与运行分别报告 |
| `lock --update` | C10 | 显式新旧来源/版本/摘要、受影响批次和证据差异；完整解析校验成功前保留原锁；冲突、坏摘要、未知必需语义或取消不得留下部分更新 |
| `explain` | L02 | 稳定诊断码的含义、触发条件、声明来源和可执行修复入口；未知码明确失败；只读且不暗中探测或构建 |

**已写入 acceptance 的差异**：在上述原包各追加该入口的正反例、机器可读输出、明确退出结果及无意外副作用检查；复用已有诊断/Report/锁格式，字段或兼容语义不足时先走格式评审。不撤销 C04 既有六命令验收，不宣称这些目标 CLI 现已实现。批准责任归属不等于批准未经审阅的新 wire 字段。

## 检查与候选身份

本批实际命令、结果、源码摘要、环境与未执行项见[验证记录](chief-architect-first-batch-validation.json)。仅成功执行的检查记为通过，首次沙箱受限的 Swift 调试符号失败单独保留；随后具备权限的完整重跑另记，不覆盖原失败。

开发输入在合并后重新校验，纯规则/格式、现有 reader、受影响仓库检查与文档生成检查分别留证。未修改原生运行时或生命周期语义；新增共享 owner 的真实 SKIE 消费检查由独立 agent 复核。全产品/远程 CI、其他 Apple 目标、ArkUI 页面运行、公开包安装与发布不在本记录的通过范围。

## 后续执行队列

沿既有目录继续 L01 完整契约与 L02 公共声明/诊断；随后按就绪接口推进 L03、L04、L05，L03 的 CPF CMP 与 L05/I05 联合验收保持原义。工程继续 I01→I02→I03→I04→I05→I06，扩展继续 E01–E03→E04→E05→E06，C08/L06 与 C09/C10 按实际前置和候选并行。具体 depends_on 和完整出口仍只读目录；本批子范围通过不解锁依赖整包验收的任务。X、I07/L08 不反向阻断普通主线。
