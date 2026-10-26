# L01 语言 API 契约输入

本页把设计基线 0.9 的 [L01](../../catalog/implementation-plan.json)要求落到可检查的输入、责任和失败条件。依据 [ADR-0014](../../adr/0014-swift-production-baseline.md)、[ADR-0015](../../adr/0015-swift-extension-delivery.md)、[ADR-0016](../../adr/0016-arkts-production-baseline.md)、[ADR-0017](../../adr/0017-arkts-extension-delivery.md)；C07 运行契约继续使用 [ADR-0013](../../adr/0013-arkts-runtime-and-completion.md)的局部基线 0.10。这里不新增 wire 格式、公共 feature、工作包、支持平台或兼容承诺。

任务子范围：支持/拒绝声明、普通与便利入口、生成与手写分工、来源与身份、独立验收输入。实现路径限本目录及 [L01 检查](../../../tests/integration/l01_contract/README.md)；不修改 C06/C07 的固定接受范围、共享运行时或历史验收。当前状态只在 catalog 维护。

逐项契约见 [Swift 十二项必要与九项扩展](swift-contract.md)、[ArkTS 十二项必要与十二项扩展](arkts-contract.md)。[L01 验收索引](acceptance.md)按当前 catalog 的十一条 acceptance 给出证据入口及不能由本轮检查推导的结果；它是审查导航，不是第二份状态表。

## 可执行的现有声明边界

“reader 能表示”与“完整 profile 接受”分别检查。不能删掉旧 profile 的结构/策略校验，使新声明自动继承 SampleEngine 的安全语义。

| 输入与检查位置 | 可接受形状 | 必须拒绝或另行交付 |
|---|---|---|
| C06 Rust → Swift：实际 UniFFI ComponentInterface；[转换器](../../../adapters/generators/kotlin_rust/lowering.py)与[配方](../../../adapters/generators/kotlin_rust/README.md) | 当前固定 SampleEngine 全图；精确数值、可空值、有界字符串/字节、逻辑 record/variant、struct 对象、已声明同步回调及 async 方法；parser 版本与 profile 一致 | 新业务名字、增删方法/字段、错误/参数变化即使可解析，仍被 `reviewed C06 semantic surface` 拒绝；defaults、借用参数、trait 对象和未支持泛型不得继承旧策略 |
| C06 Kotlin → Swift：实际 ObjC 导出 + Swift SymbolGraph；[Swift 转换器](../../../adapters/generators/kotlin_rust/skie_lowering.py) | 固定 SKIE 导出面及实际 sealed 关系；普通 DTO 可直接用，明确的跨 actor 数据才投影为 owned snapshot；擦除的 `Swift.Error` 保留后端错误通道 | 未支持 attribute、inout/隔离修饰、默认参数、数组外层语法不得被抹去；不能凭 `async`/可编译 `Sendable` 推定 native 停止或线程安全 |
| C07 四路线：真实 ArkTS 导出 + 各自 provider 元数据 + 显式策略；[公共 reader](../../../adapters/generators/arkts/read_facade.cjs)与[lowering](../../../adapters/generators/arkts/lowering.py) | 固定完整 profile 的 record、resource interface、显式 Error 和函数；record 字段为 number/string，数值宽度及 UTF-8 边界来自显式策略；Promise 只在允许返回位置；listener 同步且返回 void | 选定普通 profile 不接受任意容器、可选/默认/rest 参数、混合字段与方法、任意泛型、异步或有返回值 listener。完整 Runtime/Engine 契约不被降成纯同步 profile |
| C07 已有作者变更输入 | [ArkTS rename-record](../../../tests/fixtures/c07_arkts_generation/README.md)与[Kotlin rename-record](../../../tests/fixtures/c07_kotlin_generation/README.md)变更实际声明/实现；显式 public_id 可保留，SourceKey 与 shape 随真实声明变化 | 这是既定角色的受限改名，不证明独立新业务的方法/字段增删已可生成；内部 native seam 仍为既定角色 |
| L04 普通观察与 Observation | `MadBStateSource<Value: Sendable>` 的 collect/requestStop/awaitStopped；MainActor 模型显式 start/stopAndWait，初值 nil、无制造重放 | 没有 owned Sendable 快照或真实停止协议的源不能仅靠包装获得保证；事件不得映射为一个可覆盖状态属性 |
| L05 普通观察与 ArkUI V2 | 现有 C07 Snapshot/Engine/Subscription；普通 `StateObservation` 与可选 `ArkUIStateModel`，显式 attach/requestStop/close | 当前固定 Snapshot 不是任意模型生成；跨 env、任意 Worker 共享、隐式重放、事件状态化及装饰器赋予线程安全均不成立 |

[second-business.ets](second-business.ets)是独立业务的实际源码输入：同步值记录与自由函数，没有资源、async 或 UI。现有 C07 reader 会准确拒绝其不符合固定完整 profile 的声明集合；这是**保留缺口的负例**，不是“第二模块生成通过”。它也说明纯值 SDK 不应为了使用普通函数被强制添加 Runtime/Engine；未来由 L02/L04/L05 基于实际后端元数据和显式策略接入受检配方，不能用修改业务为 SampleEngine 来满足 DX-01/02/11。

## 普通入口、身份和依赖

| 责任 | 具体契约与默认行为 | 对照及拒绝条件 |
|---|---|---|
| 普通业务调用 | 暴露受支持方法、类型、错误和资源协议；模型已正确表达时直接复用，必要快照才复制 | 不要求 raw/native 或手包 operation handle 才能完成普通业务；高级访问保留保活、线程和关闭说明 |
| 期望身份 | 生成/装配提供 interface/profile/配套版本并验证加载对象；最终 candidate digest 留在外部清单，避免自引用 | 错库、错 profile、ABI/布局与包字节摘要分别验证；隐藏参数不能取消检查。现有 C06 Swift `openEngine(expectedProfile:)`/`MadBEngineHandle(expectedProfile:)`仍是固定样例入口，自动接线由 L04/C10 补齐 |
| 作者与消费者 | 作者使用声明/策略、锁定生成器与构建工具；预生成 SwiftPM/native XCFramework 或 HAR 消费者只使用公开产品 | C06 源码联编、L05 手工放置开发源不能当作正式包；消费者不得依赖 MadB 私有源码、Cargo/Gradle 或手补 native 库 |
| 便利与工具依赖 | 普通 SDK 不导入未选 UI、Rx、自定义宏；作者生成工具、消费端编译依赖、目标运行依赖分别登记 | 禁用配置后检查解析、下载、编译、链接及最低系统；仅拆 target 或未 import 不算无依赖证明 |
| 唯一机制责任 | 每符号/语义阶段/实例的类型转换、输入保活、完成、交接、订阅、投递、停止、排水、释放及物理装配各有一个 owner | 两个桥重复接管、缺失阶段、冲突输出、重复内核、构建环在执行/提交前拒绝；不能用安装顺序选择赢家 |

后两项复用[扩展组合](../../architecture/17-extension-composition.md)、现有 ContractIR/profile/lock、TaskGraph 和 Source/Build/Candidate；本页表格不是可直接写入 v1 JSON 的新字段。跨组件共同语义不足时，由 E01/E03 按模型演进处理；I01 接入相同构建入口，不复制另一套声明模型。

## owner、错误、异步和观察的默认规则

| 语义 | 普通 API/必要接线 | 正反输入与责任 |
|---|---|---|
| 值与错误 | 保留宽度、符号性、nil 与编码；转换失败走可表达错误。业务失败、取消、装载/协议失败分别表达 | 越界、不合法编码、未知变体和无法表达的转换失败拒绝；不以零/空串替换缺值。L02 与语言 lowering；DX-02、CRIT-07/12 |
| 资源 | 共享业务 owner 与页面观察 owner 分开；准入、保活、停止、排水、释放按既定 DEC-07/08 | close 请求或析构不是完成；别名/子资源/迟到值由原协议处理；关闭失败保留清理责任并拒绝重启。对应 C 路线 + L03–L06；DX-08、CRIT-01/08 |
| 线程与反向接口 | 同步返回保留同步契约；声明合法执行器、env、重入和 callback/context owner | 不支持的同步回投不得用阻塞等待 UI 模拟；async callback 不能套同步回调策略。跨 actor/env 需真实值与 owner 前提。DX-02/09、CRIT-11 |
| async 与取消 | 等待取消、请求停止、停止完成与业务撤销分开；终态通知与资源交接各一次 | 成功通知抛错不得再次失败完成；取消后的迟到资源必须交接或释放。普通/便利共用后端与受控适配；DX-07、CRIT-02/03 |
| state | 仅在声明允许时合并最新值；初值/重放与冷热/共享由原 profile 明示，观察模型不创造默认值 | 首片 C06/C07 观察不自动继承 StateFlow 的重放；读取/getter/构造不注册；重入与重复 start/attach 先拒绝。DX-04/05/06 |
| event | 独立事件通道，保留顺序和有界容量；满载按原协议拒绝或终止观察并可观测 | 不静默合并/去抖事件；listener 返回不是业务持久化 ack；不为每条同步回调开无界 Task。DX-06/07、CRIT-11 |
| UI 与收尾 | 页面离开先关本地投递，再由稳定 owner 保留/等待同一异步 close；共享 Engine 仍可用 | 不把 Task.cancel、Promise 返回、deinit/GC 或同步页面钩子当排水完成；正常退出与 env teardown 分别验。DX-05/08/09 |
| 诊断 | 普通错误保留稳定类别及实际来源；日志/指标/sink 可关闭或采样 | sink 失败不改变成功业务与必要保护；不伪造行列，不将追踪当业务 ack。L02/对应语言/C09；DX-10/12 |

Swift 的原语与[普通观察用法](../../../sdk/swift/observation/README.md)、ArkTS 的[普通/ArkUI 对照](../../../sdk/arkts/observation/README.md)是当前可编译示例入口；本页不复制或改名其 API。Swift `stopAndWait` 不因调用者取消而提前确认；ArkTS close 失败保留同一 Subscription，重试不重订阅。其宿主测试、SDK 编译和真实 native/UI 运行分别记录。

## 生成与手写分工、来源和变更序列

| 环节 | 权威输入 → 输出 | 失败边界 |
|---|---|---|
| reader | UniFFI 元数据、实际 ObjC/Swift 导出、ArkTS 编译器 AST、CPF KSP 等 → 真实结构与 SourceKey/location/precision | 未知类型/属性/签名先拒绝；不从命名推导线程、owner 或取消。缺少精确位置则说明文件级，不能伪造行列 |
| 规则与 lowering | 结构 + 明确策略 + 选定能力 → 受检 ContractIR/责任分配 | 缺释放器、错误类型、策略漂移或两份语义冲突阻断；不改核心去适配某个工具名字 |
| 按声明生成 | 受检声明 → 变化的业务方法、必要值投影、错误与资源接线、期望身份和来源映射 | 第二模块增删改必须沿公开输入重生成；不得手改生成输出、扩大固定私有名单或复制 SampleEngine Facade |
| 通用手写原语 | 已定协议 → 通用准入/停止/排水、语言同步原语、UI 生命周期薄适配 | 不包含逐业务方法映射；同一源不新增第二套 producer/owner/队列；原生与并发修改独立复核 |
| 工程与装配 | 同一 Wrapper/命名任务、精确锁及冻结批次 → IDE/CLI 产物与 Candidate | 输出 owner 冲突、递归构建、失配身份拒绝；失败保留旧完整批次和用户文件；不凭增量命中补运行证据 |

L02/L04/L05 的下一组业务输入沿 `second-business.ets` 所示值需求，在各自既定权威声明中定义同一业务：先新增 `Reading` 和同步 `normalize`，再新增方法、增加字段、重命名、删除导出。字段/方法名字没有私有允许名单；新语义仍需明确策略。检查输出及实际消费随变更更新、删除不留旧公开符号；失败输入不提交新 Source。ArkTS 源码文件仅是 ArkTS 侧预期普通 API 与拒绝探针，不要求 Rust/Kotlin 作者维护第二份重复结构权威源。

当前可运行检查使用实际 C07 AST reader，以及 C06 已存档编译器描述符的真实 lowering/policy gate。描述符变体是边界反例，不是假称本轮重新运行 UniFFI/SKIE 编译器。新业务的后端生成、普通消费者构建与运行、公开包/目标/IDE 验收仍须另行执行。

## 已选增强的承接条件

| 配置 | 所消费普通前提与启用责任 | 继续保留的出口 |
|---|---|---|
| Swift Observation、SwiftUI/UIKit | owned Sendable 状态、显式观察 owner/停止；L04 接口，I04 工程 | 两框架、各目标真实页面生命周期；基础包无 UI 依赖 |
| Swift 薄宏、属性封装 | 展开为已定普通 API；没有重复用例时不造自定义宏；系统机制优先 | 展开、来源、冲突、普通替代与增量编译；不隐藏注册/关闭 |
| Combine/Rx | 所选普通 Swift API 的启动/共享/错误/取消/容量清楚；L04 通用适配 | Kotlin NC 配置归 X01，每符号唯一协程桥；不成为 L04/L07/C11 新前置 |
| Swift export、深插件 | X02 独立导出 profile，L08 只在明确深改写需求时 | 各自依赖、目标和证据；不继承 SKIE 认证 |
| ArkUI V2、声明便利、状态操作 | L05 普通方法/状态/取消；I05 工程；事件不默认去抖 | 真实 UI、准确语法拒绝及未启用配置；普通包不导入 ArkUI |
| ArkTS 多 env、低复制、CPF CMP | 每 env owner/传输；buffer 生命周期；Kotlin/ArkTS 唯一实例与共享 UI | L03/L05/I05 联合证据；TaskPool/Sendable 不自动安全；性能收益需实测 |
| ANI、UniFFI-ArkTS、KNOI | X03/X07/X08 各自锁定后端和普通接口 | 不叠加替代桥、不继承 C07；提前原型不改变 C11 正式前置 |
| 诊断、IDE 向导、特殊交付 | hooks 不改变语义；I07 复用 CLI/模板；C10/I05 按需配方 | L08/I07 原前置不变；HSP/字节码/源码共同构建不继承普通 HAR 结果 |

这张表给出接入条件，逐项默认、生成形状、拒绝、依赖和独立正反输入由上述 Swift/ArkTS 明细展开；完整单项出口仍由 ADR 和对应任务 acceptance 决定。消费者可选不免除承诺交付。L04 与 C08/L06 的验收责任已在[接管决定](../../reviews/2026-10-25-chief-architect-first-batch.md#责任决定)中获得用户确认；doctor/verify/lock --update/explain 的归属及输出/拒绝验收亦已在同一记录中获确认，新增 wire 字段继续走格式审查。本声明子范围不自行扩大任何一方的支持承诺。

## 检查和独立交付

运行命令、工具身份和输入摘要见 [L01 检查入口](../../../tests/integration/l01_contract/README.md)。本子范围使用既有 DX-01/02/03/11 组织声明和诊断输入；L04/L05 的 owner/观察例继续使用 DX-04 至 DX-09，诊断使用 DX-10/12。独立 Lab 应仅消费冻结公开候选，自己定义预期，不能直接导入这些生产 reader 作为 oracle。

本目录将各承诺项的声明边界、工具/依赖/目标约束、普通/便利形状、可定位拒绝及独立验收输入交给逐 acceptance 审查；尚不存在的生成器/目标/配置不能据此声明已实现。这里的检查通过不自动登记 L01，也不代表 L04/L05、第二业务生成、目标或公开包验收通过。最终 API 和兼容承诺须在对应发布前收敛，未交付的已选能力保留原任务出口。
