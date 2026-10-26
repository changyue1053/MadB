# ADR-0017 ArkTS 扩展选择与完整交付

状态：设计采用，已授权按子范围实施；实现与验收以任务目录为准。日期：2026-10-25。适用基线：0.9。关联 [ADR-0016](0016-arkts-production-baseline.md)、[ADR-0012](0012-extension-composition-and-delivery.md)、[扩展组合规范](../architecture/17-extension-composition.md)。

## 授权与实施边界

项目所有者已授权本轮 ArkTS 必要能力、十二项扩展的选型、完整计划和实施，并委托实施负责人安排顺序。所选范围满足技术前提后自主推进，不再等待重复开工许可；未选工具、未知深度改写及新增产品需求仍需具体决定。按需求启用的功能须先明确消费场景和 profile，不把授权解释成必须安装所有候选库。

先启动 L01 的 ArkTS 契约与 L05 状态/页面订阅首片，在独立开发组合中复用 C07 公开 API。C07 原四路线与约定回归继续并行，未完成部分不由 L05 替代。消费者按需启用 UI/便利能力，已经纳入 L05/I05 的选定配置仍是产品交付责任。计划、实现开始、目标验证和公开支持分别记录。

## 十二项选择、交付要求与验收

序号仅用于讨论和导航，不新增工作包、案例组或完成率。每项先给出普通替代用法，再交付选定配置。公共契约和通用 SDK 机制复用，不为每项能力建立独立包或第二套运行时。

| 项目 | 采用方案与交付范围 | 必须完成的验收 | 工作包与顺序 |
|---|---|---|---|
| 1 ArkUI 状态/页面 | V2 的 @ObservedV2/@Trace 专用状态模型；普通无 UI 控制层独立。V1 仅在所选兼容范围需要时交付 | 真实观察实例/UI 刷新、主线程、重复进入退出、迟到回调、关闭失败重试、共享 Engine 继续使用；无 UI 消费不引入 ArkUI | L05 首片先行，I05 工程，L07 收口 |
| 2 声明便利语法 | 真实声明的构建期生成优先；装饰器只作为选定的薄声明语法，保留普通调用 | 普通/便利结果一致，源码定位、重生成与目标编译；构造/getter 不隐藏订阅、释放或异步副作用 | L01/L05 在普通接口明确后，I05 接入 |
| 3 流操作/取消兼容 | 复用同一订阅和取消协议；有消费需求时先提供受限映射、去重/去抖状态操作及 AbortController 风格适配，不建立通用 Rx runtime | 操作适用状态/事件明确；事件不默认去抖；取消落到实际 provider，迟到完成、错误和排空有对照；目标 SDK/依赖关闭配置独立通过 | L05/C09，在源协议稳定后独立子范围 |
| 4 多 env 消费 | 官方 Worker/TaskPool/Sendable；独立任务或持久 env 按 profile 选择，已有 Native 异步不再默认套 TaskPool | 每 env 初始化/引用/关闭、复制/转移/共享限制、并发停止和错误环境拒绝；Sendable 不冒充线程安全 | L05/I05/C09；新增路线接线归 C07 的独立 profile，不改首片限制 |
| 5 大数据/低复制 | 先批量调用，再按负载选择 ArrayBuffer/TypedArray 的复制、借用、转移；零拷贝独立承诺 | 有效偏移/长度、detach/转移、失败回滚、最终释放、满载预算；对比复制基线及未插桩 Release，证明收益 | L05/路线适配器/C09，在类型与所有权基线后 |
| 6 Kotlin 共享 UI | CPF CMP 与配套 Compose/Skiko/ArkTS 组件，纯逻辑配置不带渲染依赖 | 唯一运行实例、页面生命周期、渲染依赖与混合 UI、Release/HAR 独立消费；过渡 abort 桩不成为支持实现 | L03/L05/I05，在 Kotlin 普通路线和组合装配后 |
| 7 UniFFI-ArkTS | uniffi-bindgen-arkts 独立 beta 配置，服务已有 UniFFI 声明 | 生成器/scaffolding/runtime/adapter 成套锁，类型/async/对象/回调、取消/退出、完整 Native 闭包和真实 HAP；仅构建不计目标运行 | X07；可提前 P13，正式验收保留 C11 前置 |
| 8 KNOI | 独立 Kotlin/ArkTS 后端，服务已有 KNOI/Kuikly 服务模型；不隐式串联 akinterop | 先验证实际编译器配套，再验证 sharedLib/initBridge/预载、双向服务、异常/线程/释放、混淆及 HAR | X08；可提前 P14，正式验收保留 C11 前置 |
| 9 ANI | 官方 ANI；Rust 侧评估固定版本 ani-rs，独立运行时配置 | 产品 SDK/语言模式、装载、ABI、引用/线程、取消/stream/shutdown 和目标运行独立证明，不继承 Node-API 结果 | X03；隔离原型按 SDK 前提推进，正式验收保留 C11 前置 |
| 10 深度诊断 | HiTraceMeter/Profiler 与按问题增加的 hooks；基本错误、版本和符号仍属必要能力 | 插桩/非插桩、开销和采集失败对照、符号匹配；不得改变容量/取消正确性或默认上传遥测 | C09 基础度量；深度插件归 L08，保留 L07 前置 |
| 11 IDE 向导 | 同一 CLI/模板/Hvigor 用例的薄图形入口 | 参数、错误、生成结果与 CLI 一致；无向导时普通工程可构建、升级和禁用 | I07，在 I06 正常工程矩阵后 |
| 12 特殊交付 | HSP/集成态 HSP、字节码 HAR、源码联编或企业配方按明确需要选择 | 宿主重签名/装载、重复实例、依赖闭包、工具链兼容、更新/卸载与独立消费；不继承普通 HAR 证据 | C10/I05 既有端口的独立子范围，不作为默认 HAR 新前置 |

这些交付进入对应包 acceptance，状态只反映已实际开始的子范围。AKI 与 ArkAnalyzer 保留研究候选，不进入默认依赖或本轮具名交付；没有明确需求的编译器深度改写、通用响应式框架和任意版本组合求解器均不启动。

## 无环实施与完整出口

1. 当前：L01 明确本片普通协议、公开 API 与依赖；L05 交付状态订阅控制层和 V2 外壳，以真实 SDK 编译及可运行语义检查先建立开发证据。C07 并行完善四路线候选；替身和静态编译不授予 Native/UI 认证。
2. 普通协议与生成就绪后：L05 完成选定便利语法及真实业务生成；I05 完成 DevEco/CLI、已有工程接入和组合模板。状态/取消便利操作在同一协议上实现，多 env、低复制与 CMP 按各自 profile 分片。
3. C09/C10/I06/L07：从固定候选进行普通/增强/禁用对照，完整 INIT/DX/CRIT、Release/混淆、目标与新环境消费、来源许可及原样发布下载。已承诺的主线增强缺项仍阻断其完整交付。
4. E01–E03 提供有限推荐组合、清单/版本/依赖/文件 ownership、公开协议和语义协商；E05/E06 证明 EXT、升级/禁用及另一作者独立接入。不创建每语言独立插件平台，E04 仍是既有 Dart 同步探针。
5. X03/X07/X08、I07/L08 在原前置下独立验收；允许已明确子范围的早期实现，不反向加入 L05/L07/C11 的完成依赖。C11 只收口本版本声明的主线配置，不等待 S6 扩展。

上述是实施顺序和技术触发条件，不编造设备排期或日历日期。实施负责人可自主安排已授权子范围；每次绑定输入、候选与真实检查，不将设计表视为完成清单。

## L05 首个实现切片契约

首片位于 `sdk/arkts/observation`，使用 C07 公开 Engine/Snapshot/Subscription。普通 StateObservation 不依赖 ArkUI；独立 ArkUIStateModel 使用真实 @ObservedV2/@Trace 实例。通用控制逻辑手写维护，业务类型适配在本片明确限定 C07 Snapshot，不冒充任意模型生成器。

每个控制实例最多持有一个订阅。attach 建立代次和准入后调用 observeState；重复 attach 或排空未完时拒绝。requestStop 先同步禁止本地继续投递，再向原 Subscription 请求停止。close 保留异步完成责任，合并并发关闭，失败保留原订阅供重试；成功后才允许再次 attach。旧代回调不能更新新页面。工厂同步回调、listener 重入和异常不得造成遗失订阅或重复源注册。页面离开不关闭共享 Engine。

模型仅表示状态，没有事件队列或隐式重放。新 attach 的本地快照清理须在准入成功后发生，失败的重复 attach 不损坏活动状态；具体同步回调与初始值顺序必须有测试。UI 调用限所属主环境/线程；装饰器编译通过不证明页面刷新或跨 env 安全。

首片最小验证覆盖重复 attach、停止后迟到回调、关闭合并/失败重试、关闭未完再入、成功后重进、同步回调重入、listener 异常及普通无 UI 入口。正式 ArkTS 编译与宿主替身测试分别记录；真实 Native 组合、页面运行、Release HAR/HAP、生成/禁用和公开候选消费由相应后续切片补齐，未执行保持缺失。

## 来源与证据边界

[ArkUI 状态管理](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/ui/state-management/arkts-state-management-overview.md)、[ArkTS 语法](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/quick-start/typescript-to-arkts-migration-guide.md)、[TaskPool/Worker](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/arkts-utils/taskpool-vs-worker.md)、[Sendable](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/arkts-utils/arkts-sendable.md)、[AbortController](https://ohos.rs/ecosystem/polyfill/abort-controller)、[CPF](https://gitcode.com/CPF-KMP-CMP/docs)、[UniFFI-ArkTS](https://github.com/ohos-rs/uniffi-bindgen-arkts)、[KNOI](https://github.com/Tencent-TDS/KuiklyBase-components/blob/master/knoi/README-zh.md)、[ANI](https://github.com/openharmony/arkcompiler_runtime_core/blob/master/static_core/plugins/ets/runtime/ani/ani.h)、[ani-rs 固定异步指南](https://github.com/ohos-rs/ani-rs/blob/4239549a3ef1701b27571611f939abe143de4074/website/src/content/docs/guide/async.md)、[HiTraceMeter](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/dfx/hitracemeter-guidelines-arkts.md)、[集成态 HSP](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/quick-start/integrated-hsp.md)。

本次核实的 ani-rs 固定主分支已有可替换执行器和异步 stream，不能继续以旧 README 将其限制为同步 iterator；源码能力不代表已发布包或具体产品 SDK 认证。napi-ohos AbortSignal 的错误通知不等于 OHOS Native 工作停止，UniFFI-ArkTS 的 smoke 可仅构建而未设备执行。各后端均以所锁版本和目标证据作为交付依据。
