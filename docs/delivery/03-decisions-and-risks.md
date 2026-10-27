# MadB 决策状态与开工风险

本页集中记录需要实验或项目所有者决定的事项。已选架构可以指导实施；未经验证的后端、版本和设备能力不能被宣传为已支持。

## 当前方案决策

| 决策 | 状态 | 依据 |
|---|---|---|
| 独立产品与独立 Lab，无旧工程迁移任务 | 用户确认 | 产品要求 |
| Rust 作为核心与 CLI，语言帮助程序独立 | 设计采用，S0 验证分发成本 | ADR-0002 |
| 每个边界单一结构权威源，策略补充语义 | 设计采用 | ADR-0001 |
| 实际发布包交给 Lab，证据绑定摘要 | 设计采用 | ADR-0003 |
| 开放语言身份、小型端口、独立协议与受控语义演进 | 设计采用，S0/S1 验证扩展成本 | [ADR-0004](../adr/0004-language-extension.md) |
| 跨路线扩展组合、机制分工与按需交付 | 设计采用；已授权范围自主实施，具体范围和实际进度分开记录 | [ADR-0012](../adr/0012-extension-composition-and-delivery.md) |
| 扩展 init 创建原生 IDE 工程，模板与边界模型分离 | 设计采用，S0 至 S4 验证生成及 IDE 行为 | [ADR-0005](../adr/0005-project-initialization.md) |
| I01 请求错误、创建结果、引用根/SDK 与目标默认四项增量 | 项目所有者确认；剩余格式另由 ADR-0023 采用，不重复裁定 | [ADR-0019](../adr/0019-project-init-incremental-contracts.md) |
| I01 完整模板/结果、原包库存与有限 renderer/首配方 | 项目所有者采用具体提案；格式、Host 创建与最终根执行分别取证，正式版本与工具镜像未齐不得执行准入 | [ADR-0023](../adr/0023-project-template-and-init-result.md) |
| 内部工具候选真实版本 0.1.0 | 项目所有者采用；九个工具 crate 重建，publish=false，SDK/历史候选不变；版本符合不等于执行通过 | [ADR-0024](../adr/0024-internal-tool-candidate-version.md) |
| 语言特性作为薄入口，普通 API 可用，诊断与必要保护分离 | 设计采用，随各平台验证工具和语义组合 | [ADR-0006](../adr/0006-developer-ergonomics.md) |
| explain 只读查询、来源核对及机器结果 | 项目所有者已确认具体格式；实现与运行证据另验 | [ADR-0020](../adr/0020-explain-query-contracts.md) |
| 首链生产诊断与普通 checked constructor | 项目所有者已采用单条 pull 修订2；默认7/诊断9导出分开固定和消费 | [ADR-0021](../adr/0021-native-diagnostics-and-checked-constructor.md) |
| 自然 Rust 有限值配方与普通 C 入口 | 项目所有者已采用；有限 syn/rustc 读取链、64字节文本与同步 Result ABI 分别实施验证 | [ADR-0022](../adr/0022-native-value-recipe.md) |
| Swift 十二项必要能力、普通 SDK 与作者工程分责 | 设计基线 0.9 采用；L01 收敛实施契约，L04/I04/C09/C10/L07 按原依赖交付；不计为实现或验收 | [ADR-0014](../adr/0014-swift-production-baseline.md) |
| Swift 九项扩展选型与分步实施 | 本次已启动 L01 Swift 契约及 L04 观察首个实现子范围，与 C07 并行，X01/X02 独立，L08 深改写不纳入默认交付 | [ADR-0015](../adr/0015-swift-extension-delivery.md) |
| ArkTS 十二项必要能力与公开交付 | 设计采用；C07 原验收、L05 普通 API、I05 工程和 C09/C10 发布分责 | [ADR-0016](../adr/0016-arkts-production-baseline.md) |
| ArkTS 十二项扩展与分步实施 | 已授权，L01/L05 观察首片开始；X03/X07/X08 独立，正式验收前置保留 | [ADR-0017](../adr/0017-arkts-extension-delivery.md) |
| CPF-KMP-CMP + akinterop 作为鸿蒙 Kotlin 主线，KNOI 保留扩展 | 设计采用，具体组合须通过公开构建与目标验证 | [ADR-0007](../adr/0007-technology-stack-and-cpf.md) |
| 官方 Kotlin、CPF、原生绑定、MadB 工具分别管理版本 | 设计采用，候选不等于已认证 | [技术栈选型](../architecture/16-technology-stack-and-versions.md) |
| AGP 新 KMP 插件、独立 app 和 native 包装配方 | 设计采用，I04 验收，不把新插件限制误写为不能包装 JNI | [初始化设计](../architecture/14-project-initialization.md) |
| 官方/CPF 首版独立 Maven GAV 与根 publication | 设计采用，P05/P08 验证目标解析与公开消费 | [构建分发](../architecture/09-build-packaging-release.md) |
| Swift C++ 按构建系统、方向和分发场景选 profile | 设计采用，源码 direct 与公开 SDK 包装分别验收 | [后端配置](../architecture/06-backend-profiles.md) |
| UniFFI ArkTS beta 保留实验候选 | X07/P13 独立验证，不替换主线、不增加主线数 | [技术栈选型](../architecture/16-technology-stack-and-versions.md) |
| Apache-2.0 项目自有内容许可默认 | 设计采用；C01 落实实际许可/权利核对 | [开源交付](02-open-source-and-contributing.md) |
| 十二项实施契约与阶段准入 | 设计采用；按配置与证据验收 | [ADR-0009](../adr/0009-implementation-contracts-and-gates.md) |
| 主机构建完成与独立目标运行分离 | C07 显式必需功能；原默认三阶段不变，本地集成已验证 | [ADR-0010](../adr/0010-external-runtime-verification.md) |
| ArkTS 原生 SDK 协议复用 | C/C++/Rust 共享私有协议实现，各语言 provider 和验收独立；设计采用 | [ADR-0011](../adr/0011-arkts-native-protocol-owner.md) |

## 必需探针

| 编号 | 问题 | 责任角色 | 完成证据 | 未通过的影响 |
|---|---|---|---|---|
| P01 | Rust CLI 跨宿主构建与外部工具协议 | 核心维护者 | 隔离安装、帮助程序错误与版本不兼容案例 | 阻断 S1 工具分发 |
| P02 | Clang/bindgen/cbindgen 的目标布局和导出 | 原生维护者 | 两种目标布局、真实双向调用和错签名负例 | 阻断 RUST-C-ABI |
| P03 | UniFFI/Gobley 配套与旧 foreign future 双完成模板 | KMP/Rust 维护者 | 旧路径受控复现；修复后成功完成回调抛错不触发第二次完成；取消/迟到结果只交接或释放一次；JVM/Native/CPF 分开运行，锁定生成器补丁 | 阻断受影响异步配置及其 S2/OHOS 组合验收；仅升 runtime 不解除阻断 |
| P04 | Swift 并发、SKIE 可调用/可分发及协程桥组合 | Apple 维护者 | 按 profile 严格并发、类型/错误、状态/事件、取消与关闭；custom cinterop/lambda 泛型限制；宿主回调、实际 API 调用、显式可分发构建和异机消费 | 阻断相关公开 API 或对应分发配置；不以普通 API 证据代替选用便利层或 X01 组合 |
| P05 | CPF + akinterop 与正式 SDK 的来源、工具链、互调与发布 | OHOS 维护者 | 公开可复现构建、KSP1/Gradle 配套、正式应用 SDK 与 sysroot 区分、设备互调及 Release/HAR；独立 GAV 解析；CMP 按配置验证 | 阻断该 Kotlin/ArkTS 主线配置；KNOI 扩展独立验收 |
| P06 | napi-ohos/Node-API 的工具配套、env 退出与 TSFN | OHOS/Rust 维护者 | napi crate 家族与 ohrs CLI 分别锁定；目标运行时的注册、停止、环境退出 | 阻断 ArkTS 原生路线 |
| P07 | Swift C++/CXX 对象及构建/方向/分发策略 | 原生/Apple 维护者 | direct-source 与 C/ObjC++ 包装分别验证；构建系统与 interop 依赖传播；对象移动、释放、异常和实际反向调用 | 阻断相应类型、方向或分发配置，不影响已独立通过的配置 |
| P08 | 新用户和新机器能否消费候选包 | 发布维护者 | 无开发仓库的旅程；官方/CPF 根 metadata 不覆盖；SwiftPM/native XCFramework、SKIE 分发及 native 依赖完整，消费者无需 MadB/Gradle/Cargo；包摘要、ABI/布局和 profile 分开核对 | 阻断正式发布 |
| P09 | 工具与边界性能基线 | 性能维护者 | 固定负载、环境、原始样本与噪声范围 | 阻断性能承诺，功能可继续 |
| P10 | 新语言能否只依赖公开扩展契约接入 | 核心/平台/验证维护者 | S0 框架探针；S1 独立 Dart C ABI 同步消费、禁用隔离与原路线回归 | 阻断“可由第三方扩展”的承诺及对应阶段退出 |
| P11 | 初始化工程能否在原生 IDE 持续开发，跨目标工具链能否共存 | 核心/平台/验证维护者 | 模板格式、宿主/IDE 准入、AGP 新 KMP/app/native 模块、显式测试、JNI 描述符策略、Wrapper、真实 IDE Run/重建及组合证据；Apple 发行消费与源码协作分开，输入输出/配置切换与已有工程恢复分别验证 | 阻断对应模板的“一键创建、IDE 可用”承诺和主线阶段退出；不阻断独立 SDK 能力开发 |
| P12 | 宏/KSP/装饰器与平台框架能否保持普通 API、生命周期和 IDE 体验一致 | 语言/核心/验证维护者 | 版本锁、宏组合、冷/热流停止、源码定位、诊断开关与未插桩消费证据 | 阻断对应便利配置推荐；普通 API 只有其自身协议测试通过才可独立交付 |

## 独立扩展探针

| 编号 | 问题 | 工作包与阻断范围 |
|---|---|---|
| P13 | UniFFI ArkTS beta 的版本、async/回调、env 及设备消费 | X07；固定独立锁，build-only 不计设备结果，只阻断此扩展 |
| P14 | KNOI 初始化、依赖预载、Release/HAR、env 与关闭 | X08；独立验证，不以 akinterop 通过替代 |
| P15 | 远程制品身份、传输、取消与宿主提交 | X09；截断、错来源、离线和权限负例，不阻断本地 C10 |

每项记录版本、输入、命令、结果和最小复现；SDK/设备缺失与代码失败分别归类。P01/P02 是首链前置；P03/P05 优先排查高风险，但按表中对应配置阻断。配置锁和推荐资格分别冻结，见[技术栈规范](../architecture/16-technology-stack-and-versions.md)。

## 已定位的简化与体验缺口

| 事项 | 处理与验收位置 | 证据边界 |
|---|---|---|
| 固定样例尚不足以证明业务声明扩展 | L01 定义子集，L02–L06 交付，L07 以第二个独立业务模块验证 | 后端绑定生成和固定 Facade 复制分别评价，不推导任意业务自动支持 |
| 普通作者/消费者暴露过多内部接线 | I01 与 L01 固定默认旅程，I/L 各平台落实；C10 验证包依赖与身份 | 保留高级入口与必要语义，不以少写参数取消检查 |
| C05两个错配案例的构建/运行日志路径重用 | C09 后续修正runner并用新Lab revision/新报告留证 | 现有运行拒绝证据有效，原构建日志未完整保留；见[第七批独审](../reviews/l02-values-macos-review.md)，历史结论不回写 |
| 构建准备与冻结重放的成本未知 | I01/C09/P09 建立四类基线，再决定是否调整阶段执行 | 只确认路径存在；未测量收益，不修改 C04 既有验收 |
| 已有工程接入、升级与禁用不足 | I01 定义受检计划，I02–I05 按配方交付，I06 验收恢复与文件保护 | 不承诺任意工程自动迁移，不增加旧 mad/MadDemo 迁移范围 |
| 第三方接入与组合规模成本 | E01–E06 复用公开组件和公共协议支持，先验证有限推荐组合 | 可解析清单不等于可用扩展；更多组合由实际需求与证据决定 |

这些事项细化现有计划，不另建工作包或状态表。优先顺序见[实施计划](01-implementation-plan.md)，观察依据与一手资料见[架构复核](../reviews/2026-10-25-architecture-simplification.md)。

## Swift 生产基线的实施风险

[ADR-0014](../adr/0014-swift-production-baseline.md) 已收敛 Swift 十二项必要能力的主方案和取舍，下面记录其实现前提与验收边界；不是重新要求决定已采用的路线，也不是新增探针、案例 ID 或任务状态。实施先复用 SKIE、UniFFI、Clang C 及已有 DEC；普通 C++ SDK 优先 C/ObjC++ 包装，direct-source 配置另验。

| 风险 | 处置与交付位置 | 不能据此推导的结论 |
|---|---|---|
| 固定样例包装被当作通用业务生成 | L01 明确支持子集及生成/手写边界，L04 生成业务 Facade，L07 用第二个业务模块增删改验证 | 不要求为所有 DTO 重建 Swift struct；简单模型可直接导出；复制 SampleEngine 包装不证明新业务可接入 |
| 严格并发、actor 与固定线程混用；ARC/Task.cancel 被当作停止完成 | L04/C09 按 profile 验证隔离/Sendable、执行器、owner/保活、异步关闭与取消竞争，复用 DEC-03/07/08 | 主线程回调、编译通过、deinit 或取消等待不能替代完整资源排水及关闭后保证 |
| 状态合并吞掉事件，UI 重建重复订阅，宿主闭包延长生命周期 | L04 明确状态/事件、冷/热、重放与有界缓冲，验证回调持有、线程及重入；选用 UI/薄宏另验等价普通入口 | 一个页面停止不默认关闭共享上游；Observation 或宏不自动提供 native 注册、取消或线程安全 |
| 包校验、函数 checksum 与 native 唯一实例被混为一谈 | C10 提供公开依赖/身份配方，C09 验包摘要、ABI/数据布局、profile 与链接/运行实例反例，复用 DEC-09/10 | 库自己的 Package.resolved 不锁住下游；UniFFI 函数检查不证明全部 record/enum 布局；链接成功不证明共享实例唯一 |
| CLI 示例成功但正常 Xcode 工程仍需手工准备 | I01/I04 区分发行消费与源码协作；后者共用 Wrapper，声明实际目标/配置、输入输出及权限；无法证明输入闭包时保守构建 | build/prebuild 插件不是任意下载构建器；Xcode 假 stamp、工程文件存在或命令行编译不能代替增量与 IDE 证据 |
| 本机源码联编被当作可发布 Swift SDK | C10 交付普通 SwiftPM 源码与必要 native XCFramework，L07/C09 用无作者工具链的干净消费者验证；按承诺目标、工具链和二进制边界核对分发设置及兼容 | 不预设新的最低 OS/Swift 版本；现有 C06 本机结果不认证所有 Apple 目标、异机分发或新业务 API |

实现前仍须选定本次承诺的目标/工具链、声明子集、公开 API 细节、缓冲预算和二进制边界，并形成对应样例；这些属于既有 profile 与工作包的实施输入，不自动重开技术选型。SwiftPM 源码包装与预编译 Swift 模块的稳定性要求不同，不能对所有产物一概开启或豁免 library evolution。

当前按 ADR-0015 的授权并行推进 C07 原范围与 Swift L01/L04 首个子范围；I01、E01–E03 按接口就绪推进，I04 保留 I02/I03 验收链。UI/薄宏由 L04 按配置交付，Kotlin Combine/Rx 正式组合由 X01 独立验收，X02 保持 direct integration 实验。L08 深改写不纳入默认交付；ArkTS 后续已按 ADR-0016/0017 裁定并授权，与 C07 原范围分别落实。

## Swift 扩展的实施风险

九项扩展已按 [ADR-0015](../adr/0015-swift-extension-delivery.md)确定默认方案与实施归属。以下复用 P04/P08/P11/P12 及既有 DX/CRIT，不另增风险或案例编号。

| 风险 | 本次处置 | 保留的验收边界 |
|---|---|---|
| Observation 被当作原生线程、所有权或完整事件协议 | L04 只镜像声明可合并的状态，事件单独消费；复用普通 owner/停止协议与 MainActor 更新 | macOS 原型不覆盖 iOS、UIKit、真实 native 路线或完整发布；系统 @Observable 不算自定义 MadB 宏 |
| 薄宏/属性封装隐藏原生订阅和释放 | 官方机制优先；SwiftSyntax 只用于选定的重复生成用例，属性封装绑定已有 owner；不另造原生资源包装体系 | 必须有等价普通 API、展开/诊断与无宏配置，编译期通过不证明线程或排水 |
| Combine/Rx 消费需求错误地让 L04 等待 X01 | Kotlin NativeCoroutines 正式组合只在 X01 收口，主线 L04/L07/C11 不反向依赖 X01；明确子集可提前原型 | X01 的 C11 前置仍约束正式整体验收，实验通过不解锁正式扩展发布 |
| 响应式包装改变启动、取消、错误与共享语义 | X01 逐接口选择协程桥，核对订阅次数、冷/热源、需求/缓冲和错误，cancel/dispose 进入已声明协议 | Deferred 只改变启动时点；dispose 返回不等于排水；SKIE Future 预览不承担需要取消的默认调用 |
| 名义可选依赖实际进入普通包闭包 | C10/I04 核对编译、解析、链接与运行依赖；NativeCoroutines 发行变体和 Kotlin/Swift 两侧成套锁定 | 未链接 Rx target 不等于无 Rx 解析依赖；禁用 UI/宏后仍需独立普通消费 |
| Swift export 实验继承现有主线认证 | X02 独立 direct integration 配方，记录导出形状、并发与构建差异 | Alpha 或直接集成支持不证明公开 XCFramework/SwiftPM 分发，也不与主桥自动叠加 |
| 为增强诊断过早引入编译器深改写 | 先复用已有诊断钩子与系统工具，明确实际缺口再评估 L08 | 默认交付保持无深改写；关闭采集不关闭必要的资源和并发保护 |

## ArkTS 必要能力与扩展的实施风险

复用 P05/P06/P08/P09/P11/P12 及 X07/X08 对应探针，完整选型见 ADR-0016/0017；不新增平行风险编号或状态表。

| 风险 | 处置与工作包 | 保留的验收边界 |
|---|---|---|
| 上游能力、开发分支或固定样例被当作完整支持 | C07/L05 固定实际 SDK、后端及声明子集；X03/X07/X08 分别验证 ANI、UniFFI ArkTS 与 KNOI，不借用其他后端证据 | 可生成、可编译、目标可运行、可发布分别成立；CPF 配套不自动适用 KNOI |
| 页面退出只停止 UI 更新，却遗失 native 订阅或失败关闭责任 | L05 先禁止本地继续投递，再请求停止；保留 subscription 到关闭成功，合并并发关闭，失败可重试；C09 独立验证重入与迟到回调 | 主机替身不能证明 native 排水；页面不得关闭共享 Engine，不能以 GC 替代清理 |
| HAR 内源码可编译却缺少原生依赖、发布元数据或消费混淆规则 | C10/I05 验默认 HAR、原生库闭包、按需 consumerFiles/nativeComponents、许可和真实下载；I06/L07 验干净工程 | HSP/字节码 HAR 独立配置；工具公开发布不等于业务 SDK 也已完成公开交付 |
| 多 env、Sendable 或低复制被误当作自动线程安全 | L05 按专用 profile 定义 env 所属、传输、取消、buffer 生命周期及退化策略；C09 验竞态与成本 | 不跨 env 复用旧 handle，TaskPool 取消不自动终止 native，零复制宣传须有证据 |
| 扩展叠加扩大依赖和主线等待范围 | E/I/L 复用同一普通接口与构建入口；选用 UI/CMP/后端分别记录依赖，X 正式验收保留原前置 | 未启用配置不传播运行依赖；L05/L07/C11 不反向等待 S6 扩展，ArkAnalyzer/AKI 不进入默认闭包 |

<a id="本轮文档审查待裁定项"></a>

## 本轮文档审查责任决定（已确认）

以下发现来自 2026-10-25 的 `1f36aa2` 及当时未提交快照，完整依据、影响和第二意见见[审查报告](../reviews/2026-10-25-current-document-audit.md)。审查后，项目所有者已批准 E05/L03 保留早期开工、调整最终阶段与依赖：E05 从 S1 开展并在 S5 收口，L03 从 S2 开展并在 S3 收口；联合切片与具体输入见[实施计划](01-implementation-plan.md)，修改检查见[修订记录](../reviews/2026-10-25-e05-l03-scheduling.md)。以下两项均已由项目所有者在本次接管中确认，具体差异与决定见[接管首批记录](../reviews/2026-10-25-chief-architect-first-batch.md#责任决定)。同 env 顺序重开是已定契约，其证据对账单列在表后，不重新裁定方向。

| 事项 | 推荐处理 | 当前约束 |
|---|---|---|
| L04 与 C08/L06 的 Swift 验收边界 | 用户已确认：L04 收口 C06 通用层，C08 接线并复跑新增 Swift 原生 profile 的适用用例，L06 负责 C/C++ 资源外观，L07 汇总 | 已写入各包 acceptance；阶段和依赖不变，不能借用 C06 结果或让 L06 替代 Swift UI/actor 验收 |
| 目标 CLI 的 doctor、verify、lock --update、explain | 用户已确认分别归 I02、C09、C10、L02，具体输出、拒绝条件与正反验收已写入各包 | 责任决定不等于命令实现；新增 wire 字段仍须格式审查，C04 既有范围验收保留 |

同 env 顺序重开已由 ADR-0013 采用。C07 冻结四路线普通候选已有 close 后重开及旧公开对象隔离的目标证据；Kotlin 伴随另有旧 epoch/key 隔离，C/C++/Rust 伴随有初始化回滚未物理 ACK 前的准入拒绝。首批对账未定位正常 Runtime.close 的两个关闭窗口证据。第二批 C09 使用既有冻结四路线伴随候选，在 API 23 Arm64 模拟器真实 hold 窗口补齐物理 ACK 前的精确准入拒绝、释放后原关闭/receipt 消费及新 Runtime 正例；报告绑定新 Lab 源码，保留 provisional 范围。物理 ACK 已取得但最终 receipt 尚未消费的第二窗口，已按批准的[测试回调决定](../reviews/2026-10-26-c09-receipt-window.md)交付并补证。独立审查发现的 Error getter 异常逃逸已在测试派生层修复；新 producer a31ac4b / Lab a6e04ce 的四对候选完成构建、适用目标场景和独立原始证据复核。正常返回才确认原关闭与 receipt 消费；六类异常场景只确认预期拒绝，清理未确认保持缺项。两个窗口分别绑定旧/新候选范围，不宣称同一候选全部矩阵，不靠 timer 竞速或总体 passed 外推。C07 原报告及 catalog 结论不变。逐路线观察、范围和摘要见[原对账记录](../reviews/chief-architect-c07-reconciliation.json)、[首批记录](../reviews/2026-10-25-chief-architect-first-batch.md)及[第二批补证](../reviews/2026-10-26-chief-architect-second-batch.md)。

## 范围控制

“所有问题”指问题目录及新增纳入条目的明确处置与验证，不指证明任意第三方 C/C++ 程序没有缺陷。无法自动证明的前提必须在 SDK 作者接口可见，并用动态检查与负例尽可能验证。

所有阶段允许发现新问题并增补目录。新问题不复用旧编号覆盖含义；如果影响公共保证，发布判定要立即反映。

## 设计准入与实现证据

DEC-01 至 DEC-12 的规则已采用，具体 schema/接口、运行实现、设备结果仍由相应任务交付。证据不足保留缺口，不将“已有设计待验证”重复登记为新的架构缺陷。历史研究和历次修改说明见[复核归档](../reviews/README.md)，当前案例来源关系由[来源目录](../catalog/source-coverage.json)维护。

## C03 后的探针责任与执行次序

仓库维护者 `changyue1053` 负责 P01–P15 的排期、认领和证据收口，具体技术责任仍按上表角色划分；任务目录同步具名 owner，不把未启动工作计为通过。P01/P02 结果见 [C03](../reviews/2026-10-24-c03-native-probes.md)。

- P03：C06，在相应 JVM/Native/CPF 异步配置集成前，先复现旧 foreign future 双完成，再对锁定生成器补丁做回归；不得只升级 runtime。
- P05：C07，在 CPF/akinterop 配置推荐前，先核验正式 SDK/sysroot/工具来源，再做设备互调和 Release/HAR；缺设备仍保留未执行状态。
- P04/P06/P07 分别跟随 C06/C07/C08；P08/P09 跟随 C10/C09；P10/P11/P12 跟随 E04/I01/L01；P13/P14/P15 只跟随 X07/X08/X09。各自阻断点不变，P03/P05 不成为 Rust/C 的全局依赖。

C07 当前的环境观察和 P05/P06 源码风险见[C07 实施记录](../reviews/2026-10-25-c07-arkts.md)。Beta 开发探针与后续正式 SDK 26 编译、API 23 Release 模拟器运行分别留证；akinterop 引用/队列/线程退出与 napi async cleanup 的具体责任必须在候选冻结前闭合。源码审查和开发探针均不替代目标验收。

C03 发现的宏常量导出/signedness 与 LLVM archive 检查版本问题已进入固定配方和编译断言。完整输入闭包、扩展退出/取消、SDK 资源与并发、干净候选消费继续按既有工作包验收，不由本次探针结论外推。
