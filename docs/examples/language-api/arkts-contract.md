# L01 ArkTS 契约明细

本页逐项展开 [ADR-0016](../../adr/0016-arkts-production-baseline.md)的十二项必要能力及 [ADR-0017](../../adr/0017-arkts-extension-delivery.md)的十二项扩展，结合[共同契约](README.md)审查。序号沿用讨论索引，不增加任务、门禁或 wire 字段。生成/运行预期是交付约束；[检查入口](../../../tests/integration/l01_contract/README.md)只证明列明的 reader/编译边界。当前完成情况只由 catalog 记录。

## 声明、普通调用与目标

| 配置/输入 | 适用形状、默认与普通入口 | 工具、生成责任及限制 |
|---|---|---|
| C07 当前四路线普通 profile | [实际 Boundary 声明](../../../sdk/arkts/facade/Boundary.ets.in)：Snapshot 的 number/string 字段，Runtime.connect/close、Engine.roundtrip/startOperation/publishState/publishEvent/observeState/observeEvents/requestClose/close、Operation.result/requestCancel/close、Subscription.requestStop/close 及显式 BoundaryError。同步 listener 返回 void，Promise 在受支持方法返回位置 | 真实 ArkTS AST + 各后端编译器/provider 元数据 + [显式 policy](../../../adapters/generators/arkts/profiles/snapshot-v1.json)，生成普通声明/业务接线与期望身份。不同路线锁分别维护，不互借 native 结果；完整 profile 至少 record/resource/function 和恰好一个 Error，不能用纯函数规避异步/观察/退出验收 |
| C/C++ ↔ ArkTS | Node-API 与 ADR-0011 私有原生协议；真实 C/编译器布局、C++ 公开边界和显式所有权策略提供结构与语义 | 普通 API 不直接泄漏私有句柄；callback/context/释放/线程协议必须齐全。C++ 不把异常跨 ABI、裸指针寿命或未定布局默认为安全；按冻结的实际 SDK/sysroot/ABI/方向验收 |
| Rust ↔ ArkTS | napi-ohos + 当前 MadB C07 必要接线，复用所锁版本能力；真实 Rust 导出与公开 facade 共同校验 | Promise/AbortSignal 不保证工作停止，不能丢弃 C07 completion/owner 责任。生成器、runtime、native 闭包成套锁；UniFFI-ArkTS 是 X07 独立配置，不在此叠加 |
| Kotlin ↔ ArkTS | CPF akinterop + KSP/实际 Kotlin 导出与显式策略；普通 Kotlin 逻辑和 ArkTS 面对应真实源 | CPF 编译器/SDK/sysroot/Gradle/运行组件同一受管配置，不能覆盖官方安装。CMP 共享 UI 单独选择；KNOI 是 X08 独立后端，不串联成双桥 |
| 独立纯值业务 | [Reading/normalize](second-business.ets)只表达值 record 与同步自由函数，没有 Runtime/Engine/close；这是未来普通生成的独立输入和当前 reader 拒绝探针 | 当前完整 C07 profile 拒绝，不能称为已支持纯值生成器。正式实现必须从各后端真实业务声明生成同一普通需求，不让 Rust/Kotlin 作者再维护重复 ArkTS 结构权威源，不把它改造成 SampleEngine 来过门 |
| L05 普通/ArkUI 首片 | `StateObservation.attach/requestStop/close` 与可选 `ArkUIStateModel.attach/requestStop/close`；当前固定 C07 Snapshot，普通层不导入 ArkUI，模型快照 undefined 到第一次真实交付 | `@ObservedV2/@Trace` 仅用于选定 V2；[现有源码说明](../../../sdk/arkts/observation/README.md)所列 API 可用性不是产品 SDK 或真实 UI 认证。源码手工放置的开发例不是完整 HAR 交付，不代表泛型业务生成 |

每个执行 profile 必须绑定实际产品/应用 SDK、版本、sysroot、ABI、最低系统、语言/运行时模式、正反方向和 env 类型；未冻结就不得进入该目标的构建/验收。当前 CLI26.0.0.851 安装中的 TypeScript 4.9.5 API 可用于本目录的真实 AST reader 探针，不能据此把任意 HarmonyOS/OpenHarmony 版本、目标或 Worker 加入支持范围。[API26 编译首片](../../../tests/integration/l05_arkts_observation/SDK_README.md)同样只证明其实际编译输入。

普通 C07 调用在持有所属 env 的 Runtime/Engine 后，使用 `engine.roundtrip(snapshot)`、`engine.startOperation(snapshot)`、`operation.result()`；取消用同一 `operation.requestCancel()`，收尾仍 join `operation.close()`。状态/事件分别 `observeState`/`observeEvents`，订阅结束先 requestStop 再等待 close。这个对象层级仅属于此完整 profile，不推广成纯 DTO/同步函数的硬性要求。

## 十二项必要能力

“生成”是随声明变化的业务面；“手写”是共用运行原语。拒绝列规定语义，只有下文列出的现存诊断是本目录已执行的精确码；不发明新的稳定错误码。正反输入由独立 Lab 以公开候选重新构造和验证，不把生产 reader 当 oracle。

| ADR-0016 项目 | 适用声明、默认与生成/手写形状 | 普通消费与拒绝 | 独立输入、依赖与责任 |
|---|---|---|---|
| 1 支持范围 | 每个符号/目标采用前表所定真实 profile；生成普通声明及来源时保留方向和目标约束，手写 runtime 不扩大支持子集 | 普通业务不需装饰器/UI。未知必需能力、未定 SDK/ABI/env、未支持的 optional/default/rest/generic 或混合字段/方法准确拒绝 | 正：冻结目标普通 facade；反：错误 ABI/SDK/方向/env 和 unsupported 声明。C07/C10 提供精确锁及包支持范围；主 env 结果不授予 Worker 或 ANI 能力 |
| 2 真实声明生成 | 真实源/元数据 + 显式策略，生成方法、值映射、资源接线、错误与身份；通用 runtime 手写。字段和方法增删改不能靠私有固定名单 | 普通消费者从正常工程生成/安装；禁止手改输出和仅改 facade 不改 provider。当前 rename-record 仅角色改名，native seam 仍固定，不能当独立模块生成 | Reading/normalize → 新增方法 → 加字段 → 改名 → 删除分别正常构建/消费；反：缺数值策略、async listener、不支持类型。L05/C07/I05/L07；当前纯值输入拒绝如实保留，目标是解除固定业务映射而非放宽校验 |
| 3 类型与错误 | number 的精确宽度/范围、string Unicode/UTF-8 边界、nullability、bytes/container、copy/borrow 均需明示；生成实际校验/转换，手写共用错误边界。当前 C07 record 仅 number/string | 普通参数/返回表达业务类型；输入 getter/listener 异常保持原对象与身份，不检查其伪造 SDK code。类型断言、默认零/空串不能替代校验；未支持容器/借用拒绝 | 正：范围边界/Unicode/普通错误；反：越界、非整数、异常 getter、listener 抛错、原型污染/重入。C07/L05 绑定真实 native 值，C09 独立验证；不能用 TypeScript 类型擦除证明目标 ArkTS 类型检查 |
| 4 owner 与关闭 | 资源按实际长期责任引入 owner，pure value 无全局层级；生成接线只连接既有准入/停止/排空/释放；Runtime、Engine、Operation、Subscription 各自 owner 保留 | 普通显式 close；GC/析构/Promise 已返回均不等于停止完成。关闭幂等且失败保留清理责任，父子/别名不提前释放；页面只停止自己订阅 | 正：纯函数无需 runtime 与完整 profile 各 owner close；反：并发 close、父先关、别名/迟到资源与失败清理。C07/C09 各路线计数守恒；不删除完整 profile 的最低案例 |
| 5 env 与线程 | 所属 env/活动 generation/合法线程检查和有界回投属于手写 runtime，生成入口复用它。ADR-0013 允许同 env 顺序重开，前一代物理关闭及未完成责任必须结清 | 普通入口拒绝 wrong env、stale generation、关闭中准入；不能将旧“永久单实例”文字当现行方向。正常 Runtime.close 与系统 teardown 是不同路径，后者不能执行 ArkTS 时仍回收 native | 正：同 env 顺序重开和正常退出；反：旧回调/句柄复用、重开过早、teardown。C07 最新冻结场景按范围对账；正常 close 的各 ACK/receipt 窗口须有对应 C09 输入，不能拿 teardown 代替 |
| 6 async/cancel | Promise 是结果通道；requestCancel、provider 停止、结果消费/释放、close completion 各有唯一 owner。生成业务 async/result 接点，手写完成账本不被便利层替换 | 普通 Operation.result/requestCancel/close；取消等待不承诺业务撤销，resolve/reject 通知失败不得重复终态，迟到资源仍释放/交接 | 正：结果成功/业务失败/取消后 close；反：取消/完成竞争、重复 result、通知抛错、迟到交付和关闭中操作。C07/L05/C09 真实 provider/候选绑定；AbortController 适配不能仅触发 Promise reject |
| 7 state/event | 原 profile 明示初值/重放、state 合并、event FIFO、容量/满载反馈和停止排水。生成业务订阅方法/类型；手写投递、owner 和有界预算复用原实现 | 普通 state/event 分开订阅；没有隐式 replay，state 属性不能承载无损 event。同步 listener 返回不是业务 ack；构造/getter 不订阅，页面离开先关本地准入 | 正：状态合并与事件序列、独立消费者；反：满载、迟到回调、重入、重复 attach、失败 close 重试。C07/L05/C09，L05 本地模型无第二 producer/队列，native 排空依然必须实测 |
| 8 工程接入 | DevEco 与 CLI 复用同一受管 Hvigor/Wrapper 构建与生成批次；生成源/映射可索引，业务变化能增量更新；通用工程模板手写 | 普通公开 HAR 消费者只需平台工具；Native 作者走独立源码配方。输出冲突、旧锁/旧源、生成失败、构建环拒绝并保留旧完整批次/用户文件 | 正：新工程、已有工程、新增/改名/删除、移动/禁用/升级；反：中断、冲突文件、旧生成符号。I01/I05/I06 实际 IDE/CLI 对照；不要求 GUI 向导才能普通构建 |
| 9 可复现构建 | Source/Build/Candidate 分离，锁源码、生成器、依赖、SDK/sysroot 和选项，生成 batch identity 跟输入；官方与 CPF 工具安装隔离，native 依赖闭合 | 普通构建不修改全局 SDK 或下载浮动分支替代锁。缺工具闭包/错 sysroot/ABI/未闭合符号或动态库时拒绝；缓存不授予目标运行证据 | 正：独立路径重放同一锁；反：替换 sysroot/依赖字节/选项。C07/C10/I05 分别记录重放成功和字节一致，不能从前者推后者，冻结候选不因验证重新构建 |
| 10 完整 HAR | 普通 ArkTS 源/声明、所选 ABI native、运行依赖、消费者混淆规则、兼容元数据、许可及符号对应同一身份；生成业务面，C10 配方装配 | 消费者安装 HAR，无 MadB/Cargo/CPF Gradle 或手补 `.so`；错库/profile/ABI、漏公开声明/运行依赖/必要规则阻断此包 | 正：独立干净 HAP 安装普通/选定便利 HAR；反：漏 native/依赖、混淆丢导出、身份替换。C10/I05/C09，开发源复制或 HAR build-only 不能代替实际装载 |
| 11 独立消费 | 从最终候选安装，普通/便利/禁用分别执行；生成期望身份独立于库自报，实际 HAR/DSO 字节与报告绑定 | 类型、取消、退出、满载和错误库/ABI/profile 的结果明确；宿主、模拟器、真机结果不能互换。装饰器编译不证明 UI 更新 | 正：独立作者 Reading 生成消费及固定完整 profile；反：装载失配、错误值、停止竞态。C07/C09/C10，Release/混淆、设备/env 与清理计数齐全才覆盖相应目标 |
| 12 公开维护 | 工具/业务 SDK 分别版本化；生成公开 API/来源映射，兼容/依赖/许可/匹配符号随候选维护。基本诊断即使不开深插件也可用 | 普通错误有稳定类别与真实来源；可选诊断关闭不关必要校验。升级不应靠伪造 public_id 隐藏 shape 变化；发布后下载必须验证原候选 | 正：旧/新普通消费、升级/禁用、原样候选下载；反：不兼容声明/行为、符号错配或包被替换。C09/C10/C11/I06/L07 分担，当前不执行发布、远程 CI 或下载验收 |

## 十二项已选扩展

消费者禁用某项不免除项目交付该已选能力。下面保留每项实际触发条件和原任务前置：独立实验 profile 不继承主线证据，也不反向阻断主线。未明确选择的版本、目标和支持子集不能由库名字推断。

| ADR-0017 项目 | 适用输入、默认/生成与手写分工 | 普通替代与明确拒绝 | 依赖、目标和独立正反输入 |
|---|---|---|---|
| 1 ArkUI V2 | 已有普通 owned state；手写 StateObservation/ArkUIStateModel，随业务变化的类型接点生成。snapshot undefined 直到真实值；成功新 attach 清旧值，失败 attach 保留；源不凭装饰器重放 | 普通 StateObservation 或原 observeState。重复 attach、close pending/failed 再入拒绝 Busy；event 不映射 snapshot；getter/constructor 不订阅 | 选定产品 SDK 的 V2；V1 仅明确兼容配置。L05/I05/C09 真页面出现/离开/重进/前后台/销毁 + native drain；反：同步 callback 后注册失败、迟到值、重复渲染和失败 close；API26 编译不足以证明页面更新 |
| 2 声明便利语法 | 真实声明的构建期生成优先；选定薄装饰器仅展开既有普通调用，业务声明与来源由 generator 维护，通用 runtime 不复制 | 不加装饰器的显式普通声明/调用可用。未知位置/参数/类型、重复生成与来源缺失拒绝；构造/getter 不能隐藏订阅、释放或 async 副作用 | L05/I05 锁定实际编译器与生成配置；正：普通/便利声明生成形状、结果、来源与增量对照；反：不支持语法指实际源位置，未选配置无额外工具/运行依赖。当前无自定义装饰器输出，不把系统 V2 编译当该交付 |
| 3 流操作/取消 | 有消费需求才对状态提供受限 map、dedup/debounce；适配复用同一订阅及实际取消 provider，不建立通用 Rx runtime。生成业务类型连接，通用操作手写；计时/缓存若引入必须有容量与 owner | 普通 observeState/observeEvents、requestCancel/requestStop/close 是替代。event 不默认去抖；AbortController 风格信号不是工作停止或 drain 完成，不能只 reject Promise | L05/C09 在明确源语义/实际 SDK/依赖下验证。正：状态变换、取消、正常停止；反：事件丢失、迟到完成、错误及计时器/缓存未释放。普通、所选操作和关闭配置各自编译/运行；具体新增公开操作接口实现前仍须审查 |
| 4 多 env | 每 profile 选择独立 TaskPool 任务或持久 Worker/env；每 env 初始化/引用/close 独立。生成/普通资源声明不自动增加 Sendable，手写环境适配显式复制/转移/允许共享 | 主 env 普通 API 替代；已有 native async 不默认再包 TaskPool。wrong env/stale 引用、不支持共享、不可转移值准确拒绝，不靠 Sendable 注解制造安全 | 官方 Worker/TaskPool/Sendable 与产品 SDK/语言模式锁；L05/I05/C09 + C07 新 profile。正：各 env 独立调用与并发停；反：跨 env 引用、任务退出时未排水。当前 C07 单 env 结果不授予此配置 |
| 5 大数据/低复制 | 先批量调用；ArrayBuffer/TypedArray copy、borrow、transfer/零拷贝分别声明偏移/长度/owner/预算。生成检查与交接接点；手写共用释放与传输机制 | 普通复制路径是比较基线；detach 后使用、越界视图、异步借用无保活、失败回滚缺失或零拷贝无 owner 必须拒绝，不能从 buffer 类型推导零拷贝 | L05/路线适配器/C09 在类型与所有权后落实。正：批量与复制/选定模式结果一致、最终释放；反：detach/转移/满载/失败释放。未插桩 Release 比较吞吐/延迟/内存，未测收益不宣传优化 |
| 6 CPF CMP | Kotlin 普通路线 + 明确共享 UI；Compose/Skiko/ArkTS 组件配套装配，一实例的生命周期/渲染依赖有唯一 owner。生成业务/组件接线，通用 UI 生命周期薄适配手写 | 纯 Kotlin 逻辑 + 普通 ArkTS/ArkUI 是独立配置，不带渲染依赖。重复 runtime、关闭后渲染、错误共享实例、过渡 abort 桩拒绝作为支持实现 | L03/L05/I05 联合锁 CPF/Compose/Skiko/ArkTS 工具/组件；正：混合页面、离开重进、Release/HAR 独立消费；反：重复装配/依赖缺失/销毁中回调，纯逻辑禁用依赖独立证明 |
| 7 UniFFI-ArkTS | 已有 UniFFI 声明的独立 beta 后端；生成器/scaffolding/runtime/adapter 成套锁，生成值/对象/async/callback 与必要接线，通用 owner 沿明确协议 | napi-ohos 主线是另一配置，不串联同符号双桥。不支持类型/取消/退出、配套版本错或 native 依赖不闭合则拒绝该 profile | X07，可提前 P13，正式保留 C11 前置。正：普通 SDK 完整 HAR/HAP 独立安装与真实目标运行；反：对象/回调释放、迟到结果/退出和错库。smoke build-only 不计运行，主线 C07 结果不复用为其认证 |
| 8 KNOI | 既有 KNOI/Kuikly 服务模型的独立 Kotlin/ArkTS 后端；先核实际 Kotlin 编译器配套，再生成双向服务声明/接线；不隐式叠加 akinterop | 原 CPF akinterop 是另一配置。sharedLib/initBridge/预载、服务错误/线程/owner 未明或版本不配套拒绝，不能以包能下载推导支持 | X08，可提前 P14，正式保留 C11。锁编译器、KNOI/运行组件/产品 SDK；正：双向服务 Release/混淆/HAR 独立消费；反：未初始化、异常、退出中回调/重复装配，证明实际加载/释放 |
| 9 ANI | 官方 ANI 与独立语言/运行时 profile；Rust 侧只评估固定 ani-rs。生成 ANI 真实声明/引用接线，手写该 runtime 所需引用/线程原语；不借 Node-API 状态机证据 | Node-API 主线是另一配置；产品 SDK/语言模式不支持 ANI、错 ABI/引用/线程或不支持 async/stream/shutdown 准确拒绝 | X03 原前置 C11；锁实际 SDK、ANI 与 Rust adapter/runtime。正：装载、调用、async/stream、shutdown 目标运行；反：错误线程/引用释放/取消竞争。固定源码存在能力不等于正式包或目标认证，不回退为旧“仅同步 iterator”假设 |
| 10 深诊断 | 基本错误、版本、来源/符号仍必要；按实际问题加 hooks，HiTraceMeter/Profiler 是选定平台适配。生成边界/符号接点，手写采样与输出，默认不上传遥测 | 普通错误与显式 hooks 可用。采集失败、关闭/采样不得改容量、取消、错误类别或成功结果；trace 不当业务 ack | C09 基础度量，L08 深插件保留 L07 前置。正：插桩/非插桩、匹配符号与开销；反：sink 失败/关闭、过量记录、符号错版，目标工具和运行配方分别固定 |
| 11 IDE 向导 | I07 是同一 CLI/模板/Hvigor 的薄图形参数入口，生成相同受管文件与任务，不引入第二构建计划或私有状态 | 无向导正常 CLI/IDE build/update/disable 可用。非法选项、冲突输出/未满足 profile 与 CLI 同类诊断，不让 GUI 隐藏失败或重写用户文件 | I07 在 I06 工程矩阵后；正：同参数两入口结果/身份一致、已有工程升级；反：非法选项、失败恢复/禁用、未安装向导。图形工具不成为普通 HAR 消费依赖 |
| 12 特殊交付 | HSP/集成态 HSP、字节码 HAR、源码联编、企业配方只按明确需要选，沿 C10/I05 端口装配对应 native/声明/宿主规则；不新增所有 HAR 的前置 | 普通完整 HAR 是替代。宿主重签/加载、重复实例、未闭合依赖、工具链/更新卸载约束未定则该配方不获支持，不能继承普通 HAR 结果 | C10/I05 独立子范围锁目标/配方。正：对应公开产物干净消费、更新/卸载与宿主装载；反：重复 runtime、错工具/字节码/签名及残留库。具体非默认分发承诺实现前逐项定稿，不把每种候选形式默认变发布产品 |

## 拒绝、来源与验收边界

实际 [reader 检查](../../../tests/integration/l01_contract/check_arkts.cjs)有六组输入：当前普通面、真实 rename-record、`reject-async`、`reject-async-listener`、`reject-callback-result`、独立 Reading。前三个负例分别要求 `ARKTS_DECL_UNSUPPORTED_MODIFIERS` / `ARKTS_DECL_CALLBACK_RESULT` / `ARKTS_DECL_CALLBACK_RESULT`，并要求真实文件及正行列；纯值输入要求 `public declaration counts outside restricted profile`。模块 hash 来自已审查工具清单并在前后核对，不将任意脚本摘要当作完整工具链认证。

这组 reader 结果不执行 ArkTS 目标编译、native 或 UI；rename-record 改名成功不证明方法/字段任意增删生成，Reading 的受控拒绝也不满足 DX-01/02/11。源行移动只改来源/输入身份，不改稳定 SourceKey；重命名改变 SourceKey，显式 public_id 可保留但 shape/兼容仍重验。真实来源缺列号时保留精度，不填看似准确的位置。

独立验收复用 [DX](../../validation/07-developer-ergonomics.md)、[INIT](../../validation/06-project-initialization.md)、[CRIT](../../validation/08-critical-scenarios.md)与 C07 既有场景，输入含声明/策略/锁、普通与选定便利、未选配置、目标/候选身份、源码诊断与资源观察。L01 明确这些输入，不删除 L05、I05、C09/C10/L07 的实现与目标退出条件。实验 X03/X07/X08、I07/L08 的原前置不变；未完成主线增强不能靠“消费者可选”免交付。
