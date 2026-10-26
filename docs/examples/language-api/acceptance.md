# L01 逐条验收审查索引

本页按 [catalog L01](../../catalog/implementation-plan.json)的十一条 acceptance 组织本目录证据，序号只表示原数组顺序。它不维护进度/verification，不改变任务依赖，不从文档采用或某项检查通过推导整包通过。依据为当前 [AGENTS](../../../AGENTS.md)、[共同方法](../../engineering/07-agent-development.md)、L01 的 documents 及 ADR-0014–0017；本目录未新增产品范围、公开 wire 格式、兼容承诺或门禁。

## 逐条核对

| 原 acceptance | 本次具体审查证据 | 检查能证明什么；仍须保留的交付 |
|---|---|---|
| 1 普通/便利/生成/运行依赖与唯一机制 | [共同契约](README.md)的普通入口、owner、生成手写三表；Swift/ArkTS 每项分别列声明生成、手写运行原语及依赖 | 符号/阶段/实例不因便利层多一套转换、终态、producer、停止、排水或装配。组合冲突与包依赖隔离仍由 E01–E03/I/C10 实现/验证，本页表格不是新执行 schema |
| 2 主线/配置便利/S6/E04 | 共同契约“已选增强”；[Swift 明细](swift-contract.md)九项扩展及[ArkTS 明细](arkts-contract.md)十二项扩展 | L04 通用 Combine/Rx 与 X01 Kotlin NC 分开；X02、X03/X07/X08、I07/L08 保留原前置；E04 仍是同步探针。消费者可选不免交付，实验不继承认证或反向阻断主线 |
| 3 普通/便利对照、展开、失败诊断与 DX | [普通 Swift/Observation 对照](../../../sdk/swift/observation/README.md)、[普通 ArkTS/ArkUI 对照](../../../sdk/arkts/observation/README.md)、本目录 Swift 编译夹具/真实宏展开与 ArkTS AST 拒绝；下文 DX 输入表 | 可执行检查只覆盖当前 ordinary/model 编译、系统宏和 reader 反例；真实 native/UI、全部选定便利配置的实现与运行分别归语言包/I 链/C09/L07。API/兼容基线在对应发布前冻结，不将系统宏检查扩大为自定义宏交付 |
| 4 支持声明/手写生成/消费选型/组合 | 共同契约“可执行现有声明边界”“生成与手写分工”；两语言声明/目标表；现存 schema 与 SourceKey/显式策略规则 | 既有 C06 完整面、C07 AST 子集及 profile gate 保留；新增方法/字段/改名/删除正反检查区分 parser 与完整策略。不把所有后端都套成一种结构，也不让消费框架决定 native 内核 |
| 5 普通业务、身份与接线 | Swift 必要 2/3/9；ArkTS 必要 2/4/6/10/11；共同契约普通入口/身份 | 生成器应提供变化的业务方法/错误/owner 和 expected interface/profile 接线，外部 Candidate 持最终包摘要；raw 才有额外保活/线程/close 义务。**当前 C06 固定 Facade、需要显式 expectedProfile 与第二业务拒绝仍是实现缺口**，不能以本页要求当实现证据或转嫁给消费者 |
| 6 Swift 十二项必要 | [Swift 契约](swift-contract.md)“声明与目标边界”“十二项必要能力”，与 ADR-0014 同序逐项列适用声明、默认、生成/手写、拒绝、正反输入及责任 | 当前锁定 C06 和 Observation 编译边界明确；未承诺的新 target/工具/分发不填假值。C08 新 profile、第二业务生成、身份自动接线、真实 iOS/包/工程结果仍须各责任包完成；讨论序号未新增任务/案例 |
| 7 Swift 模型/Facade/owned snapshot 与 DEC | Swift 声明表、必要 3–8、扩展 1/2/5/6；两个实际编译反例 | 后端已正确表达的普通模型直接复用，必要才投影；跨 actor 不可变 owned Sendable 与普通 foreign object 分开。通用 runtime 手写，业务投影/接线生成；编译反例不是 native 安全证明，owner/取消/反向接口继续沿既有 DEC |
| 8 Swift 分发/作者/UI宏/授权 | Swift 必要 9–12、扩展 2–4；ADR-0014 后续由 ADR-0015 明确执行授权 | 公开消费 SwiftPM 普通源码 + 必要 native XCFramework；作者源码和工具链单独配方，正式消费者不装 MadB/Gradle/Cargo。UI/宏及其依赖只在所选配置；当前开发源包和 macOS 编译不代表正式分发 |
| 9 Swift 九扩展普通前提/默认/冲突/验收 | Swift 九项扩展逐行含普通替代、依赖、支持边界和独立正反输入 | X01 独占选定 Kotlin 响应式符号的 NC bridge，SKIE 仅非重叠增强；不能从 L04 通用适配推导 X01 通过。锁定配置消费与禁用解析/编译/链接/最低系统验证均保留；L04/L07/C11 不增加等待 X01 的前置 |
| 10 ArkTS 十二必要 + 十二扩展 | [ArkTS 契约](arkts-contract.md)两张逐项表、声明/目标表和实际反例说明 | 四路线与独立后端逐配置、ordinary/便利/禁用、真实目标/Release/混淆/公开包分别验；没有工具/目标/输出的项不声明已生成。表只沿用 ADR 讨论序号，未增加案例组或删除 L03/L05/I05/C09/C10 联合责任 |
| 11 ArkTS 层级/重放/owner及无虚假保证 | ArkTS 纯值输入、必要 3–7、扩展 1–5；共同契约默认规则 | Reading 无强制 Runtime；当前完整 C07 仍保留完整资源/异步/退出。Promise/装饰器/类型断言/Sendable 不授予停止、排水、共享或线程安全。模型重放/初值遵源，同 env 顺序重开遵 ADR-0013，不再作为未裁定方向 |

本页区分“L01 的设计契约与可执行样例是否可审查”和“各语言实现/目标是否已满足这些契约”。L01 不包办后者，也不能改写 acceptance 把本应实现的普通 API、第二业务生成、全部选定增强和独立消费从后续任务退出中删除。是否登记 L01 整包由主协调在独立复核和集成检查之后决定。

## 现有 DX 的独立输入

| 既有用例 | 冻结输入与独立预期 | 当前样例/检查及后续责任 |
|---|---|---|
| DX-01/02/11 | 第二业务真实声明 + 显式策略 + 工具/profile 锁；新增方法、增加字段、重命名、删除后只走公开构建，公开类型/错误/身份及消费者对应变化。失败不得提交半批次；删除不得留旧符号 | Reading/normalize 给普通需求；C06 描述符变体与 C07 真实 rename/负例定位当前接受边界。现有 Reader 拒绝不满足生成成功；L02/L04/L05/路线与 I 链补真实产物，Lab 另写期望 |
| DX-03 | 同一模型普通声明/调用与所选便利声明/展开；对照公开符号、值/错误、订阅/close 次数，禁用配置依赖独立 | 本轮真实 Swift 系统宏展开、public model 正例；自定义宏/装饰器只在具体选择后验，实际展开/错误源位置由 L04/L05/I04/I05 补。不得按模板模拟编译结果 |
| DX-04/05/06 | owned state 与 event 两源；无初始重放/源声明重放分别处理；页面普通/便利出现、离开、停止未完重进，状态合并、事件顺序、满载与 listener 重入 | 现有 L04/L05 观察正反例是入口；真实 native/各 UI 框架/目标由相应语言包与 C09/L07 执行，宿主替身不充当 UI 证据 |
| DX-07/08/09 | 取消与迟到资源、stop 请求与 drain、共享业务 owner 和局部观察 owner、错误 actor/env/线程、源停止失败与重试 | Swift Sendable/actor 编译反例只证明静态拒绝；C06/C07/C08 的 owner/并发仍需候选级真实交错与资源计数，原生变化独立复核 |
| DX-10/12 | 真实声明错误、调用失败、来源精度、固定候选/符号；诊断开/关/采集失败三配置与普通结果对照 | ArkTS reader 要求真实文件行列和精确类别；Swift 编译器定位实际消费者源。L02 的 explain 对应同一来源/规则，C09 验证实际运行与诊断失败不改变业务 |

独立包输入还包含每个所选 profile 的完整候选、普通/便利/禁用工程、精确工具/目标/依赖、Release/混淆或严格并发设置、兼容基线及资源清理预期；采用现有 INIT/CRIT，不新增替代门禁。缺环境、未运行和 cancelled 均保持未证明，远程 Actions 待项目完成后独立补跑，不进入本轮通过口径。

## 可执行检查的使用

命令、前提和结果范围见 [tests README](../../../tests/integration/l01_contract/README.md)。Python 直接使用真实 lowering/policy；Node 使用已审查 SDK 的真实 TypeScript parser/生产 reader；Swift 使用现有工具锁编译生产 Observation 源与消费者夹具，保存实际展开/失败诊断和输入摘要。检查输出放在仓库外的新目录，由主协调将最终集成输入/命令/结果绑定到既有审查记录。

本目录没有复制状态台账，没有改 schema/reader/profile/candidate，也没有修补历史报告。其具体缺口是上述**已知实现与环境证据**，不是已采用路线重新待批准。若实施发现需要新的公开 API、兼容保证、target 或所有权/并发语义，先给具体差异、影响和推荐方案走既有决定；无冲突工作继续。
