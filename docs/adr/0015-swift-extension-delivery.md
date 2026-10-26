# ADR-0015 Swift 扩展选择与完整交付

状态：设计采用，已授权按子范围推进实施；实现和验收结果以任务目录为准。日期：2026-10-25。适用基线：0.9。关联 [ADR-0014](0014-swift-production-baseline.md)、[扩展组合契约](../architecture/17-extension-composition.md)与 L01、L04、X01、X02、L08。

## 决策与授权

在普通 Swift API 和既有资源、线程、取消协议之上增加可选择的语言体验。项目所有者已授权本轮选定 Swift 需求的文档、计划和实现推进，并委托实施负责人安排子范围顺序；不再逐项等待开工许可。L01 的 Swift 设计子范围已开始，先明确当前切片所用契约，再推进 L04 的 Observation 切片。实现开始不等于整包验收通过。

本决定衔接 ADR-0014 原先仅作设计采用的范围。保留既有工作包、phase 与验收依赖，不将选择表当作完成清单。C07 按其既有范围独立推进，保留并行改动；后续 ArkTS 必要能力与扩展已由 [ADR-0016](0016-arkts-production-baseline.md)、[ADR-0017](0017-arkts-extension-delivery.md)另行裁定并授权实施，不改变本决定的 Swift 范围。未选择的候选库不因本轮授权自动安装或进入交付承诺。

## 九项选择

序号仅用于讨论，不新增任务或验收 ID。普通 API 是所有已选便利层的共同基础；具体版本、目标和最低系统按所选 profile 与[技术栈规范](../architecture/16-technology-stack-and-versions.md)固定。

| 项目 | 采用方案及理由 | 范围与任务 |
|---|---|---|
| 1 Observation | Apple Observation 的本地 Swift 模型，复用系统观察能力；不以观察属性承担 native 生命周期 | L04；只在明确承诺旧系统时提供 ObservableObject 兼容配置，不维护第二份业务状态真相 |
| 2 SwiftUI / UIKit 生命周期 | 使用各框架官方生命周期入口，薄适配连接普通观察协议 | L04/I04；两个框架分别验证可见、离开、销毁和前后台，页面只拥有自己的观察 |
| 3 薄宏 | 确有重复声明或接线需求时使用官方 SwiftSyntax 宏机制，展开为稳定普通 API 的调用 | L04/I04；当前使用系统 `@Observable` 不要求先实现自定义 MadB 宏，未选宏有普通替代用法 |
| 4 属性封装 | 优先系统状态/绑定机制；选用自定义封装时只绑定已有 owner 和已同步的本地值 | L04；不新增自研 resource wrapper，不以初始化、getter 或析构偷偷注册和异步关闭 |
| 5 Combine | 系统 Combine 作为正式可选消费能力，适合已有 Publisher 工程 | L04 可复用普通 Swift 适配；选定 Kotlin 响应式配置采用 X01 的 NC 桥，不成为全部 Swift SDK 前置 |
| 6 RxSwift | RxSwift 作为正式可选消费能力，服务已有 Rx 工程，独立声明运行依赖 | L04 可复用普通 Swift 适配；Kotlin 的 NC 配置归 X01；不向未选 Rx 的消费者传播依赖 |
| 7 SKIE + NativeCoroutines | Kotlin→Swift 的响应式配置使用 NC，保留 SKIE 非重叠类型增强；同一符号只保留一个协程桥 | X01；Kotlin/Swift 组件及生成配置成套锁定，不推广到 Rust/C 路线，也不从普通 SKIE 继承组合证据 |
| 8 官方 Swift export | 保留 Kotlin 官方 Swift export 的独立 Alpha 配置，用于验证另一种导出机制 | X02；先验其 direct integration 子集，不替换 SKIE 主线，不默认承诺 SwiftPM/XCFramework 分发 |
| 9 诊断与深度插件 | 优先显式边界 hooks，Apple 适配使用 OSSignposter；已有机制可定位调用且无需深改写 | 基础诊断归 L02/L04，验证归 C09/L07；OTel 仅在明确跨系统追踪需求时评估，SIL 深改写或定制编译器不进入默认承诺，确需深插件的范围归 L08 |

选择依据：[Apple Observation](https://developer.apple.com/documentation/observation)、[SwiftUI 模型管理](https://developer.apple.com/documentation/swiftui/managing-model-data-in-your-app)、[SwiftSyntax](https://github.com/swiftlang/swift-syntax)、[Combine](https://developer.apple.com/documentation/combine)、[RxSwift](https://github.com/ReactiveX/RxSwift)、[NativeCoroutines](https://github.com/rickclephas/KMP-NativeCoroutines)、[官方 Swift export](https://kotlinlang.org/docs/native-swift-export.html)、[OSSignposter](https://developer.apple.com/documentation/os/ossignposter)。上游资料说明机制，不替代锁定版本及实际候选的验证。

Perception、KMP-ObservableViewModel、moko-mvvm、SKIE UI 预览适配，以及自研资源属性封装器和定制编译器不进入本轮默认承诺；保留作为有具体缺口时的替代研究，不建立默认待安装清单。ObservableObject 是明确旧系统配置的兼容选择，OTel 是有明确需求时的诊断候选，两者都不自动进入基础产品。

## 责任与依赖边界

Observation、UI、薄宏、属性封装及 Combine/Rx 的普通 Swift 适配可以跨 Rust、Kotlin、C 路线复用；前提是对应路线已经提供所需普通 API 和协议。不能从统一外观推导 C 回调具备取消、任意对象可跨 actor 或各后端错误完全相同。SKIE/NC 与官方 Swift export 只属于 Kotlin→Swift；深插件按实际编译器、目标和版本分别实现。

便利层不重建类型转换、源订阅、终态认领、资源交接、停止、排水或物理库装配。机制分工仍由 DEC-07/08/09/10 及[运行时规范](../architecture/08-runtime-and-idiomatic-api.md)负责。普通 API 与便利入口共用实现，增加的生命周期作用域、排队和调度必须明确纳入同一责任链。

九项能力不要求九个包。普通 SDK、可选集成、消费端宏、作者工具和开发诊断按实际依赖与发布边界划分；包级最低系统、公开类型依赖和解析/编译成本均需验证。仅拆 target 不能证明基础消费者不受影响，具体隔离规则由[扩展组合契约](../architecture/17-extension-composition.md)维护。

## 完整交付出口与无环实施

| 出口 | 交付责任 |
|---|---|
| L01 契约 | 当前支持子集、普通/便利用法、生成与手写分工、选定与未选配置、失败预期和独立验收输入明确；子范围交付不冒充 L01 完成 |
| L04 及对应路线 | 普通 Swift API、选定 Observation/UI/语法或通用响应式适配形成可编译实现；后端能力复用 C06/C08，C/C++ 资源接线仍由 L06 承担 |
| I01/I04/I06 | 配方、正常 CLI/IDE 构建、依赖隔离、来源定位、增量与失败恢复形成工程证据；现有 I02→I03→I04 关系保留 |
| C09/C10/L07 | 独立普通/便利消费、竞态与资源计数、固定候选、公开包、兼容升级、禁用和未插桩 Release；复用既有 INIT/DX/CRIT，不新增另一套门禁 |
| C11 | 收口本版本声明的主线发布配置，不等待未纳入该版本的 X01/X02/L08 |
| X01/X02/L08 | 按自身前置和支持范围交付扩展；复用已经建立的工程、打包及验证工具，针对新候选形成该扩展自己的证据 |

L04 不以 Kotlin NC 组合为整体验收前置；X01 保留 C11 前置，不能反向加入 L04/L07/C11 的依赖链。L04 可以交付其明确选定的普通 Swift 响应式适配，Kotlin NC 的正式消费结果归 X01。X02 保留 C11 前置，L08 保留 L07 前置。依赖表示验收前置，允许在所用契约明确后安排独立原型或子范围实现；提前结果不提前授予整包通过或发布资格。

本轮先推进 L01 的 Swift 契约及 L04 Observation 首片，具体接口由[语言体验规范](../architecture/15-developer-ergonomics.md)维护。该片的目标、工具锁和已执行检查单独记录；协议替身、macOS 运行或 SwiftUI 编译均不能替代真实 native 接线、iOS 真机/模拟器、UIKit 或完整包消费证据。所有选定能力完成对应出口后才进入其公开支持承诺，当前状态与缺口只在[任务目录](../catalog/implementation-plan.json)维护。
