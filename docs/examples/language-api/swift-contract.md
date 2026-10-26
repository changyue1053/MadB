# L01 Swift 契约明细

本文逐项展开 [ADR-0014](../../adr/0014-swift-production-baseline.md)的十二项必要能力及 [ADR-0015](../../adr/0015-swift-extension-delivery.md)的九项扩展。序号沿用 ADR 的讨论索引，不是新能力 ID、配置 schema 或状态。与[共同契约](README.md)一起审查；发生冲突以 ADR、[语言体验](../../architecture/15-developer-ergonomics.md)及[扩展组合](../../architecture/17-extension-composition.md)为准。下表的生成形状是实现约束；可执行证据见[检查入口](../../../tests/integration/l01_contract/README.md)，没有产物的行不得据此声称已生成。

## 声明与目标边界

| 配置 | 结构来源、适用声明与普通产物 | 当前锁、目标与明确限制 |
|---|---|---|
| Rust → Swift / C06 | 真实 UniFFI ComponentInterface；当前固定 SampleEngine 图的精确标量、可空值、有界 string/bytes、record/variant、对象、同步 callback 及 async 方法。复用 UniFFI 已正确生成的值/对象，必要错误与 owner 接线位于普通 Swift 层 | [C06 锁](../../../adapters/toolchains/kotlin_rust/toolchain.json)与配套生成器/scaffolding/runtime/补丁；现有证据为 macOS Arm64 Release。新增方法或不同业务图先过现存策略门，不因 parser 接受而获得支持；借用参数、trait、默认值/泛型不得默默降级 |
| Kotlin → Swift / C06 | 实际 ObjC export + Swift SymbolGraph；SKIE DTO/sealed/suspend/Flow 已承担的部分直接复用。跨 actor 只有具有 owned Sendable 保证的值才能作为快照；擦除的 Swift.Error 保留后端错误通道 | 同一 C06 锁及 SKIE 配置；框架编译模式和 Swift 6 严格并发消费者分别记录。当前固定业务图，未支持的 inout、隔离/属性、默认值/泛型语法准确拒绝；不把 Swift 包装当作 Kotlin 对象线程安全证明 |
| C/C++ → Swift / C08 | C 走实际编译器头文件/布局与显式 owner/error/callback 策略；C++ 独立 SDK 优先 C ABI 或 ObjC++，direct-source 独立 profile。普通 Swift 面保留精确类型和显式资源 owner | C08 选定锁、方向、公开 C++ 类型、下游 interop、目标与最低系统必须一并固定。当前本目录不提供已验收 C08 profile；不能从 C06 推导支持。C++ 异常/未知布局/裸指针无释放与线程依据不得公开包装后放行 |
| Observation 首片 / L04 | 已有 `MadBStateSource<Value: Sendable>` → `@MainActor MadBStateModel<Value>`；纯本地状态投影，源码包不依赖 native/SwiftUI。普通后端入口仍可单独使用 | [Package.swift](../../../sdk/swift/observation/Package.swift)声明 Swift 6，macOS 14、iOS 17、tvOS 17、watchOS 10 为源码可用性下限；本目录新增检查限锁定 Xcode 的 macOS Arm64 编译，不授予这些系统运行认证 |

C02 的[类型/策略规则](../../schemas/README.md)仍限制结构映射：嵌套 resource/callback、缺逐字段资源映射、非标量 buffer 不会因 Swift 能表达而自动受支持；C record 布局必须来自真实编译器。显式策略绑定真实 SourceKey/public_id；两份显式策略冲突即拒绝，即使值相同。未知必需能力不能被忽略。选择新的目标、跨线程公开保证或分发支持属于对应 profile 的具体交付审查，不在这里凭空补值。

## 十二项必要能力

每行都继承上述实际后端和目标边界。“生成”指随业务声明变化的部分；“手写”指跨业务复用的普通运行原语。拒绝栏中的语义要求不是新诊断码；现存 reader/compiler 的精确拒绝另见下文。独立验收由 Lab 自己定义预期，通过冻结公开候选执行，不导入生产 reader 充当 oracle。

| ADR-0014 项目 | 适用声明、默认行为与生成/手写形状 | 普通消费及拒绝条件 | 独立正反输入、依赖与责任 |
|---|---|---|---|
| 1 支持范围 | 每个公开符号属于确定后端/方向/profile/目标；声明子集跟真实工具锁。生成文档及声明面只列该组合实际可表达的类型和方法；手写 runtime 不补未知类型 | 普通调用不要求 UI/宏。未声明方向、目标、ABI、最低系统、必需能力或声明形状应在构建/加载相应阶段拒绝，不能按主机猜默认 | 正：锁定 C06 普通输入；反：同输入换不匹配目标/版本/方向。检查 profile 与实际模块/SDK，不能用文档表替代 C06/C08/C10 的目标证据 |
| 2 真实声明生成 | 实际元数据 + 显式语义策略是输入；生成方法、模型差异、错误/资源接线和来源。手写原语不能出现第二业务名的允许名单。新增/改名/删除都产生相应公开面变化 | 普通消费者只改业务声明和正常构建输入；不得手改 Facade、生成文件或新增私有映射。当前 C06 固定面拒绝仍保留，不把拒绝改成通过 | 第二业务使用 Reading/value/label 与 normalize 的普通预期，按新增方法、增加字段、重命名、删除分别重生成并消费；反：不支持借用/默认值及缺策略。L02/L04、C06/C08 实现，I04 构建，L07/C09 独立验证；此输入目前未生成 Swift SDK |
| 3 普通 API | 正确后端模型直接公开；有类型安全/独立快照需要才投影。生成业务方法、必要错误映射和资源接线；通用 owner 可手写。不把所有对象强制复制成 struct | 业务正常路径不需 `.native`、手包 operation 或手传摘要。高级 raw 入口必须说明保活/线程/close。类型窄化、未知变体、nil/编码或无法表示的错误不得用默认值吞掉 | 正：普通业务调用与同一便利调用比较值/错误/次数；反：越界、未知变体、错误通道丢失。现有 `openEngine(expectedProfile:)` 与 `.native` 仍是固定样例入口，自动身份/业务接线由 L04/C10 补；C08 的 C/C++ 对等接线由 L06 配合 |
| 4 actor 与线程 | 每入口明确 actor/执行器、是否阻塞、允许重入；跨 actor 值必须实际 owned Sendable。生成隔离/值投影只反映真实保证；手写受控投递不为每个值增加无界 Task | 普通与便利共用执行上下文，不靠 `@unchecked Sendable`、注解或锁住调用者制造安全。同步 callback 不能阻塞等待 MainActor | 正：严格并发编译的 owned 值 + 合法 MainActor 调用；反：可变非 Sendable 值和无 await 的隔离调用，现有编译夹具定位消费者源。跨线程 native 正反例仍归 C06/C08/L04/C09，编译失败不能充当运行竞态证据 |
| 5 owner 与关闭 | 调用、对象、操作、订阅各有对应 owner；别名/子资源保活及准入→停止→排水→释放沿 DEC-07/08。生成资源与方法连接；手写统一生命周期实现，页面只拥有观察 | 普通显式 close/stop 可用；析构仅兜底请求，不确认异步排水。关闭失败保留清理责任，排水未完成不得重新订阅；页面不得关闭共享业务 engine | 正：别名存活、并发 close、页面停止后 engine 可用；反：父先释放、close 未完再入、停止失败、迟到资源。普通/便利对照 owner/释放计数；C06/C08/L04/L06 实现，C09 独立复核 |
| 6 async 与取消 | 等待取消、工作停止、业务撤销、完成通知、结果交接分别有 owner。生成 async 方法/错误/必要 handle 接线；手写通用终态规则复用后端，不再建第二完成状态机 | 普通 async 调用保持原结果/错误语义；Task.cancel 不代表 native stopped。终态认领一次，迟到资源交接或释放；成功通知抛错不得再次失败完成 | 正：正常成功/业务失败/取消/最终 close；反：取消与成功竞争、通知抛错、迟到资源、调用者在 cleanup 期间取消。锁定后端任务与 Swift runtime，C06/C08/L04→C09；每条结果绑定候选 |
| 7 state 与 event | state 仅在声明允许处合并，event 保留 FIFO/容量/满载反馈。冷热、共享、初值/重放由源声明，默认不凭外观补重放。生成订阅业务类型；手写通用观察 owner/投递 | 普通源可直接订阅；Observation 只消费 state，event 仍走独立普通订阅。getter/构造不注册，stop 请求不等于 drain，listener 返回不等于业务 ack | 正：state 快照、event 顺序、独立消费者与共享源；反：无初值时读、满载、重入、停止后投递、重复 start、失败 drain 重试。C06 固定无历史重放与 SKIE 语义分别保留；L04/C09 逐源验证，不以 StateFlow 名称推定 |
| 8 宿主接口 | 只导出所选后端已支持的 callback/反向接口；参数/返回/错误/上下文/同步性/保活显式。生成 callback 转换与 context 接线，手写原语负责 admission 和 release | 普通 protocol/closure 是可用入口；同步返回不能擅自变 async，返回资源需要完整 owner 映射。未支持 async callback、错误返回或跨线程回投准确拒绝 | 正：支持同步回调、重入与错误传播；反：异步 callback 套同步策略、context 早释放、关停后回调、要求同步 UI 返回。C06/C08 各后端各方向独立输入，不能以单方向通过推定任意宿主反向导入 |
| 9 身份与依赖 | interface/profile/配套版本由生成/装配提供，ABI/layout/checksum 按后端验证；包 digest 在外部候选清单避免自引用。每实例、每语义阶段与物理库有唯一 owner | 普通创建路径自动带期望身份，消费者不拼摘要；隐藏参数不删除校验。错库、ABI/layout/profile、不完整依赖、重复运行内核或文件 owner 冲突拒绝 | 正：安装冻结包正常创建；反：调换 native 库/声明/profile、重复装配相同内核。E01–E03/C10 提供组合/装配，路线负责 ABI；独立安装核对实际链接、加载与候选字节，不能只比包名 |
| 10 工程接入 | 作者普通 IDE/CLI 构建调用同一生成计划；生成源可索引，新增/改名/删除及失败恢复有确定输出。生成器不抢用户文件；通用 Wrapper/任务接线手写 | 预生成包消费者用 SwiftPM/Xcode；作者使用锁定工具。缺作者工具、冲突输出/构建环/来源过期准确失败，保留旧完整批次，不要求手补索引或路径 | 正：新建/已有工程、干净与增量、移动工程；反：生成中断、失配锁、删除后残留符号、用户文件冲突。I01/I04/I06；IDE 和 CLI 使用同一入口的实际结果分别记录 |
| 11 正式包 | 生成 Swift 声明/适配源码与选定 XCFramework 模块切片身份一致；包配方包含 native/transitive 依赖、许可、符号和最低目标。作者源码配方与预生成消费包分开 | 默认公开消费 SwiftPM + native XCFramework；消费者不装 MadB/Cargo/Gradle。选定切片缺失、公开类型泄漏未声明依赖、符号/许可不匹配均阻断该包 | 正：无作者仓库/工具的干净工程安装；反：移去切片/运行依赖、未选 UI/Rx 却要求解析下载。C10/I04/I06，iOS 真机与模拟器为首发重点但须有真实候选目标矩阵，macOS 通过不代替 |
| 12 独立消费与维护 | 工具版本与 SDK 版本独立；声明/ABI/行为兼容基线和升级方案按实际公开面。生成来源/符号映射随候选；运行与发布清单保持原候选身份 | 普通、已选便利与禁用配置各自消费；ABI 不变不代表行为兼容。公开发布只能发布已验收原候选；重构建或换包必须重新绑定证据 | 正：第二业务、干净安装、升级/禁用、原候选下载复核；反：旧客户端/新包不兼容、符号失配、下载被替换。C09/C10/I06/L07/C11 各自完整出口保留；当前不执行远程 CI、发布或下载验收 |

## 九项已选扩展

普通 SDK 独立可用；“可选”只针对消费者。已选交付不可从项目清单省略。禁用验证同时看依赖解析/下载、编译、链接、最低系统及运行；只拆 target 或未写 import 不足以证明隔离。每个配置只增加其负责的消费投影，不再次拥有类型转换、源订阅、终态、停止、排水或物理装配。

| ADR-0015 项目 | 适用输入、默认/生成与手写分工 | 普通替代与拒绝 | 编译/运行依赖及独立输入 |
|---|---|---|---|
| 1 Observation | owned Sendable state；手写 `MadBStateSource`/`MadBStateModel`，业务适配随声明生成。初值 nil，显式 start，一次 source/collector，成功重启才清快照；不生产第二业务真相 | 普通 subscribe/collect/requestStop/awaitStopped；mutable foreign 对象、event 映射 state、重复 start、失败 drain 后重启拒绝。现有模型 `collectionFailure` 与 `stopFailure` 分离 | Apple Observation + 源码可用性下限见前表。正：真实值更新、stop/restart；反：非 Sendable/错误 actor 编译、真实 stop/重入失败。L04 的 native 接线/真实页面/目标仍分别验收；旧系统 ObservableObject 仅明确选定时交付 |
| 2 SwiftUI / UIKit | 页面 owner 显式连接同一观察；构造/render/getter 不注册。SwiftUI 与 UIKit 生命周期薄适配手写，业务显示属性引用生成模型；共享 engine 归应用 | 普通显式 start/requestStop/stopAndWait；同步离开钩子只发停止并把 join 留给稳定 owner，不能返回即认定排水。页面清理不关闭共享 engine | SwiftUI 与 UIKit 只进入各自选定配置；两框架分别做实际出现/离开/销毁/前后台和停止未完重进，记录真实 native。L04/I04，iOS 模拟器与真机独立；当前 macOS 编译不覆盖 UIKit |
| 3 薄宏 | 已确认重复声明/接线才使用 SwiftSyntax 宏，展开为稳定普通调用；系统 `@Observable` 已用于当前模型，不据此要求自定义 MadB 宏。业务部分生成，宏不得藏 runtime owner | 普通显式声明/调用始终可用；不支持位置/声明、重复生成符号、冲突属性给实际源诊断。宏展开不得偷偷订阅、关闭或改隔离 | 消费端宏插件/SwiftSyntax 与 compiler 配套锁；作者工具依赖另列。正：实际系统宏展开含 registrar/访问跟踪；反：展开/源位置和普通替代结果对照。自定义宏只有选定用例与接口后才可验收；L04/I04 不借系统宏通过授予其证据 |
| 4 属性封装 | 优先系统 state/binding；自定义 wrapper 若选用，只绑定既有 owner 和已同步本地值。手写薄包装，业务属性按声明生成；初始值遵循普通源 | 普通存储属性 + 显式生命周期可替代。初始化/getter/deinit 注册、异步 close、隐藏资源 wrapper 或跨 actor 非安全值必须拒绝/不得生成 | 所选 UI/system 模块；不引入独立资源内核。正：读取/绑定只改变本地展示；反：多次构建/读取不得新增订阅，销毁不能确认 drain。L04 的计数/生命周期对照与 I04 禁用依赖检查 |
| 5 Combine | 对选定普通 async/state/event 生成类型化适配接点；通用 Publisher 适配手写。沿源的启动/共享/错误/取消/容量，不把 event 静默合并；需求与反压不能被无限缓存替代 | 普通 async/订阅 API；同时为同符号选择 SKIE 与 NC 协程桥、缺停止完成/满载语义或无法映射错误时拒绝该配置 | 系统 Combine 仅所选消费者依赖。正：订阅/取消、错误、多个消费者与 bounded 满载；反：取消晚结果、重复 bridge、未选配置依赖泄漏。L04 交普通 Swift 适配；Kotlin NC 特定配置归 X01，不能反向阻塞 L04 |
| 6 RxSwift | 同上但使用明确选择的 RxSwift 类型/调度；生成业务类型接线，手写普通适配。dispose 只按原协议发停止，后续 drain 仍有 owner，不默认无界排队 | 普通 async/订阅可替代；不得把 dispose 返回当 native release 或引入第二 producer。未承诺终止/背压的源不能以 Rx 名称补保证 | RxSwift 精确版本及公开类型依赖独立锁；只有选 RxCocoa UI 时加入 RxCocoa。普通、Rx、禁用三配置分别验证安装/编译/运行及停止异常。L04 通用层；Kotlin NC 桥属 X01，不默认传播到 Rust/C |
| 7 SKIE + NC | Kotlin 所选 suspend/Flow 符号仅一个协程桥：NC 负责所选响应式桥，SKIE 保留不重叠增强。生成配置按符号责任分配，普通资源内核不替换 | 原 SKIE 普通 API 是替代配置；同符号/阶段重复接管、Kotlin/Swift 组件版本不配套或不支持类型拒绝，不能按安装顺序选赢家 | X01 成套锁 Kotlin 插件/注解、Swift NC 库与 SKIE 配置；独立比较基础 SKIE、NC 组合、禁用的类型/错误/取消/Flow/owner 与依赖，真机目标另证。正式前置 C11 保留，提前原型不进入主线认证 |
| 8 官方 Swift export | Kotlin 独立 Alpha profile 的实际导出声明；生成/导出由该后端，MadB 只补必要契约/接线。先 direct integration，支持子集由其锁定编译器输出决定 | ObjC export + SKIE 是另一个普通配置；不能混用其模块、类型能力、运行内核或验收。超出实际导出子集/目标与非选定分发准确拒绝 | X02 独立锁 Kotlin/export/Swift 工具及工程；正：真实声明 direct integration 消费；反：支持外类型/重复装配/关停。SwiftPM/XCFramework 不默认承诺；若新增该承诺须单独定配方和验收，X02 不等待主线改路线 |
| 9 诊断与深插件 | 生成符号/边界来源接点，通用 hooks 手写；默认无需深改写，Apple 适配 OSSignposter。错误/调用身份来自同一边界，采样/关闭不改变业务、容量或停止 | 普通错误和来源查询可用；sink 失败不得替换业务结果，记录不得冒充业务 ack。类型/ABI/生命周期必要检查不可随诊断关闭 | L02/L04 提供 hooks，C09/L07 比较插桩/非插桩 Release、采集失败、符号匹配和开销。OTel 仅明确跨系统需求评估，SIL/自定义编译器归 L08 的独立选择/目标/版本，不默认安装 |

## 可执行输入与诊断精度

本目录的 Swift [编译正例](../../../tests/integration/l01_contract/fixtures/OrdinaryObservation.swift)实际导入公共 `MadBObservation` 并调用 `start`/`requestStop`/`stopAndWait`；[可变值反例](../../../tests/integration/l01_contract/fixtures/RejectMutableSnapshot.swift)和[错误 actor 反例](../../../tests/integration/l01_contract/fixtures/RejectUnsynchronizedActor.swift)必须分别由 Swift 编译器以 Sendable/actor 错误拒绝并定位实际文件。宏检查保存真实系统展开，不从字符串模板模拟 compiler 成功。它们是编译夹具，不是已生成第二业务 SDK 或已运行 native 生产者。

C06 的 [reader 检查](../../../tests/integration/l01_contract/test_declaration_contracts.py)直接调用实际 lowering/policy，使用既存编译器描述符；新增字段/方法、改名/删除能够解析后仍由 `reviewed C06 semantic surface` 门拒绝。该拒绝明确证明当前真实业务生成缺口，不授予新声明支持。错误位置精度以 reader 实际提供的 SourceKey/location/precision 为准；不能给文件级来源编造行列。

完整 Swift 契约审查应检查“上述必要输出仍属于责任包”，不能用这里的编译反例将 C06/C08/L04/L06/C09/C10/I 链/L07 的实现或目标验收移走。按已确认的[责任决定](../../reviews/2026-10-25-chief-architect-first-batch.md#责任决定)，L04 在 C06 普通接口上交付其完整语言体验；C08 每个新 Swift profile 必须重新接入适用 L04 套件，C09 独立验收；L06 保留 C/C++ 资源与 C 对等责任。后续 API/兼容基线在正式发布前冻结，新增公开承诺仍走既有决定流程。
