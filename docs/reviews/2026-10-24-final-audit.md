# MadB 设计基线 0.5 历史复核

本记录保留 0.5 基线当时的结论和检查范围。当前决策、全部修改及验证结果见[0.7 复核记录](2026-10-24-route-reconciliation.md)，本文的版本、数量及浏览器结果不代表当前基线。

复核日期：2026-10-24。结论：保留当前模块架构和完整产品范围，修正术语、状态模型、示例及阶段追踪，补齐架构全景和实施进度入口。文档可以作为进入 S0 的基线；产品可行性、代码完成度、平台认证和发布资格仍须实施与实测。

## 1 范围与方法

本轮基线输入共 56 个文件：45 份 Markdown、10 份 JSON、1 份 TOML。按产品需求、架构、工程规范、验证、交付、ADR、格式与示例逐类阅读、交叉核对；检查引用、权威源、状态、编号、目录和示例一致性。原始文件摘要与处置见[复核清单](inventory-0.4.json)。

历史资料覆盖九组边界的 236 条原始问题，核对其到 48 个公共问题、96 个基础案例组的映射及原文摘要。重点回看 Kotlin/Swift 类型和生命周期、Kotlin/ArkTS 载荷、Rust 原生资源、取消与制品交付经验；抽查 MadDemo 当前对应源码与 ADR。历史分析不是对所有旧代码重新做一次安全证明，原项目中的“已通过”也不转换成 MadB 结果。

机制依据优先核对官方文档与上游源码说明；网页当前状态用于修正能力假设，不把网页 latest 直接写入项目工具锁。完整路径与具体版本由 S0 及后续平台探针确认。

## 2 本轮修正

| 问题 | 风险 | 已采取的文档优化 |
|---|---|---|
| Route 与 profile 混用 | 把路线名称误当作固定工具/目标组合 | 区分 Route、boundary profile、template profile；TOML 用 route；案例模板将未解析 profile/target 标为 null |
| 大写历史路线 ID 与命名空间协议未接合 | 扩展清单无法直接使用既有目录 ID | routes.json 增加显式 protocol_route_id 映射；宿主解析别名，不猜测第三方 ID |
| 五类/六类执行结果不一致 | 下游阻塞或缺 SDK 被当作跳过后放行 | 统一 passed、failed、error、skipped、unsupported、blocked；能力汇总、工作包与协议状态另行区分 |
| TYPE-006 保证过强 | “输入校验”被理解为证明任意外部地址有效 | 限定可检查形状、长度和数值；裸指针来源有效性仍需作者契约 |
| 生成暂存编译与初始化最终路径构建描述有歧义 | 把带暂存路径的缓存/二进制直接认作最终可用 | 源码原子提交与最终构建分开；移动产物需要重定位验证 |
| 扩展示例摘要统称“合成” | 无法区分真实样本摘要与虚构执行结果 | 明确示例 manifest 摘要按文件字节计算，其余未随附 payload/结果为示意；均不是发布身份凭据 |
| S0 探针范围与后续平台退出条件模糊 | 要求所有平台先实现才允许首链开始 | S0 完成首链必需前提并分配其余风险探针；各阶段按相关探针阻断 |
| 核心主线只有阶段，没有独立工作包 | 无法与 E/I/L 任务一起追踪依赖和验收 | 增加 C01–C11；保留 E01–E06、I01–I07、L01–L08；补 X01–X06 后续路线 |
| 设计与实现进度容易混淆 | 用文档完成量声称工具已完成 | 基线显示设计已复核、产品实施未开始、路线认证为零；草稿与基线分离 |
| 平台特例过度归并 | 装载、注册与混淆缺陷在通用“构建测试”下丢失 | 补 KNOI 装载顺序、sharedLib 注册、initBridge 和 Release 混淆探针，复用 BUILD-002/003、CAP-004 |
| SKIE 与新 Swift export 能力需保持准确 | 重复“不支持 Combine/Flow”等旧结论 | 明确 SKIE Combine 预览及其启动/取消限制；Swift export 作为当前 Alpha 扩展线独立验证 |
| 导航缺少全局入口 | 文档很多但难以理解实施路径 | 首页先给架构 HTML、进度 HTML 和复核记录；原文件编号和链接保留，不为排版重命名所有规范 |

架构和实施的 HTML 使用版本化数据生成，生成脚本可检查漂移。新文档入口见[架构全景](../visuals/architecture-overview.html)、[实施进度](../visuals/implementation-progress.html)及[维护方法](../visuals/README.md)。

## 3 历史经验与新设计的关系

| 历史依据 | 新设计判断 | 需要保留的验证 |
|---|---|---|
| MadDemo ADR-028：鸿蒙会话使用受限 JSON Schema，iOS 经 SKIE；平台接口另有类型化路径 | JSON 是特定载荷选择，不是 Kotlin/ArkTS 的必经桥，也不推广到 Swift | 类型化值、JSON、字节分别测；严禁强制断言代替实际校验 |
| ADR-028 将生成物入库，原因包括当时工具尚无完整发布物 | 新产品可以有构建期生成和受管 SDK，但先交付锁定工具与依赖；旧工程局部决策不是通用限制 | 新环境与 IDE 不依赖未发布的兄弟仓库；确定性和漂移检测继续保留 |
| Kotlin/Swift 研究区分直接使用、Facade 和独立快照 | 三者按需求选择；不要求全部业务模型经历三套转换 | ID/可空值类、快照读取隔离、复制深度和不支持形状准确阻断 |
| MadDemo LinkRequests：取消/超时只放下等待，真实 work-count 仍可能占用；迟到结果另有认领 | 等待、工作、回调分别计数；停止等待不能冒充工作完成 | 迟到响应、重复响应、关闭及名额归还交错 |
| MadDemo ConversationViewController：在 viewDidLoad 注册，deinit 中取消，使用 SkieSwiftStateFlow 包装 | 对象释放与页面隐藏不同；不能据注释就宣称隐藏已退订 | 页面反复进入、隐藏/销毁、共享状态和显式停止 |
| 同一 Swift 页面使用 draftRevision 与 markedTextRange 处理草稿回声和输入法 | 通用层可提供同步元数据和钩子；业务决定何时丢弃草稿、何时视为受理 | 输入回声、组合态、迟到快照与受理后清空 |
| MadDemo 风险/阶段记录：SKIE 类加载器、Kotlin fork、x86_64 切片、SwiftPM 时序 | 不能只检查依赖下载成功；工具组合与消费者目标各自认证 | 干净构建、Release、不同架构、混合工具链与原生 IDE 重建 |
| mad / mad-boundary 研究：回调 context、输入缓冲、资源父子关系和绑定/二进制一致性 | 保留单一 owner、借用窗口、停止排水与冻结候选；不重复造一层通用资源系统 | 实际释放器、调用中关闭、迟到回调和错库负例 |

上述源码抽查只确认当前可读快照中的调用与清理方式，没有在本轮运行旧项目应用或复测其历史设备结果。原始材料入口见[参考资料](../references/README.md)；236 条映射保留原编号与摘要，不通过改写历史材料制造一致性。

## 4 行业机制核对

| 主题 | 当前核对结论 | 对 MadB 的要求与依据 |
|---|---|---|
| UniFFI | 从接口声明生成 Rust scaffolding 与目标语言绑定；不替代平台产品交付 | 复用生成机制，补契约、包、运行协议和独立消费者。[上游指南](https://mozilla.github.io/uniffi-rs/latest/) |
| Gobley | 面向 Kotlin/Rust 的工具与 Gradle 集成 | JVM/Native 分别锁定后端与目标，不推导任意 KMP 平台支持。[官方文档](https://gobley.dev/docs/) |
| bindgen / CXX | 前者产生 Rust FFI 声明，后者在受支持桥接面生成双端代码 | 对象、线程、别名和 ABI 前提仍显式审查。[bindgen](https://rust-lang.github.io/rust-bindgen/)、[CXX](https://cxx.rs/) |
| Swift C++ interop | Swift 能导入受支持 C++ 声明，也能向 C++ 暴露部分 Swift API；特征和引用寿命有具体限制 | 不退化为“一律 C 包装”，也不宣称全语言特征互通。[官方参考](https://www.swift.org/documentation/cxx-interop/) |
| Kotlin/Native ObjC | 桥接导出面、命名、可空值和泛型均受规则约束 | 直接类型优先，按真实导出形状生成受限 Facade。[官方参考](https://kotlinlang.org/docs/native-objc-interop.html) |
| SKIE / NativeCoroutines | 都提供协程接入；SKIE 有 Combine 预览，NativeCoroutines 有 Async/Combine/RxSwift 包 | 同一接口的协程桥指定唯一责任方，启动/共享/取消分别验收。[SKIE](https://skie.touchlab.co/features/combine)、[NativeCoroutines](https://github.com/rickclephas/KMP-NativeCoroutines) |
| Swift export | 当前官方描述 Alpha、直接集成及 suspend/Flow/enum/sealed 映射，同时仍有限制 | 保留独立候选，不用旧能力表否定新机制；新组合重做探针。[官方参考](https://kotlinlang.org/docs/native-swift-export.html) |
| KNOI | 公开互调之外，上游明确提示装载、注册和混淆注意事项 | 模板和真实 HAP 测试显式覆盖这些条件。[上游 README](https://github.com/Tencent-TDS/KuiklyBase-components/blob/master/knoi/README.md) |
| OpenHarmony Node-API | napi_env、handle scope、引用与实际执行线程有关；长时持有需正确引用机制 | napi_ref 不自动赋予跨线程能力；环境退出及回投逐目标测试。[平台稳定性](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-faq-about-stability.md) |
| KSP / ArkTS / Dart | KSP 不改写函数体；ArkTS 允许受限装饰器但禁止动态对象布局；Dart 当前采用构建期生成 | 便利语法不承担未知安全证明；普通 API 保持可用。[KSP](https://kotlinlang.org/docs/ksp-overview.html)、[ArkTS](https://github.com/openharmony/docs/blob/master/en/application-dev/quick-start/typescript-to-arkts-migration-guide.md)、[Dart](https://dart.dev/tools/build_runner) |

上游 ohos-rs 仓库的 README 存在继承 napi-rs 的 Node.js 平台说明；此类文案不能作为 HarmonyOS/OpenHarmony 目标可用性的独立证明。MadB 对应路线继续要求精确 SDK 的注册、装载、回调和 HAP 证据。[项目](https://github.com/ohos-rs/ohos-rs)

## 5 未被本轮文档检查替代的事项

- 协议 payload、project/profile/IR/lock/report schema 的可执行实现，属于 C02/E01–E03；本轮两份扩展 schema 只覆盖已定义结构层。
- P01–P12 风险探针、具体版本锁、目标设备和 CI 环境，需要在相应工作包执行；没有凭网页能力填入认证。
- 项目许可证和仓库组织等所有者决策仍保留；不存在默认为已经获准的发布操作。
- 任意 C/C++ 指针有效性、业务事务回滚、第三方原生算法正确性不属于通用工具可自动证明的内容。
- 旧项目采用的业务降级、重连、单订阅及状态回放策略不能变成通用默认值，必须在具体 SDK 契约中明确。

## 6 复核后的阅读顺序

架构师先看[全景](../visuals/architecture-overview.html)和本记录，再读[模块](../architecture/02-modules-and-ports.md)、[契约](../architecture/03-contract-and-ir.md)、[生命周期](../architecture/05-lifecycle-and-concurrency.md)。实施负责人从[进度页](../visuals/implementation-progress.html)进入工作包及其规范；平台开发者从[后端约束](../architecture/06-backend-profiles.md)进入特例和测试。原工程与语言规范的编号保持不变。

## 7 本轮检查与结果

文档检查涵盖引用、编号、表格、格式样本、原始映射摘要、任务依赖和视图生成漂移；扩展 schema 的正反样本覆盖结构边界。HTML 在本地浏览器检查桌面/窄屏、主题、筛选、模块/任务详情和草稿隔离。执行结果保存在[文档验证记录](validation-result-0.5.json)。

这些结果证明本轮文档与视图的一致性和可使用性；不代表 MadB 的跨语言运行时、CLI、模板或目标 SDK 已经过产品测试。
