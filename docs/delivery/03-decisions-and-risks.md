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
| 扩展 init 创建原生 IDE 工程，模板与边界模型分离 | 设计采用，S0 至 S4 验证生成及 IDE 行为 | [ADR-0005](../adr/0005-project-initialization.md) |
| 语言特性作为薄入口，普通 API 可用，诊断与必要保护分离 | 设计采用，随各平台验证工具和语义组合 | [ADR-0006](../adr/0006-developer-ergonomics.md) |
| CPF-KMP-CMP + akinterop 作为鸿蒙 Kotlin 主线，KNOI 保留扩展 | 设计采用，具体组合须通过公开构建与目标验证 | [ADR-0007](../adr/0007-technology-stack-and-cpf.md) |
| 官方 Kotlin、CPF、原生绑定、MadB 工具分别管理版本 | 设计采用，候选不等于已认证 | [技术栈选型](../architecture/16-technology-stack-and-versions.md) |
| AGP 新 KMP 插件、独立 app 和 native 包装配方 | 设计采用，I04 验收，不把新插件限制误写为不能包装 JNI | [初始化设计](../architecture/14-project-initialization.md) |
| 官方/CPF 首版独立 Maven GAV 与根 publication | 设计采用，P05/P08 验证目标解析与公开消费 | [构建分发](../architecture/09-build-packaging-release.md) |
| Swift C++ 按构建系统、方向和分发场景选 profile | 设计采用，源码 direct 与公开 SDK 包装分别验收 | [后端配置](../architecture/06-backend-profiles.md) |
| UniFFI ArkTS beta 保留实验候选 | X07/P13 独立验证，不替换主线、不增加主线数 | [技术栈选型](../architecture/16-technology-stack-and-versions.md) |
| Apache-2.0 项目自有内容许可默认 | 设计采用；C01 落实实际许可/权利核对 | [开源交付](02-open-source-and-contributing.md) |
| 十二项实施契约与阶段准入 | 设计采用；按配置与证据验收 | [ADR-0009](../adr/0009-implementation-contracts-and-gates.md) |

## 必需探针

| 编号 | 问题 | 责任角色 | 完成证据 | 未通过的影响 |
|---|---|---|---|---|
| P01 | Rust CLI 跨宿主构建与外部工具协议 | 核心维护者 | 隔离安装、帮助程序错误与版本不兼容案例 | 阻断 S1 工具分发 |
| P02 | Clang/bindgen/cbindgen 的目标布局和导出 | 原生维护者 | 两种目标布局、真实双向调用和错签名负例 | 阻断 RUST-C-ABI |
| P03 | UniFFI/Gobley 配套与旧 foreign future 双完成模板 | KMP/Rust 维护者 | 旧路径受控复现；修复后成功完成回调抛错不触发第二次完成；取消/迟到结果只交接或释放一次；JVM/Native/CPF 分开运行，锁定生成器补丁 | 阻断受影响异步配置及其 S2/OHOS 组合验收；仅升 runtime 不解除阻断 |
| P04 | Swift 并发、SKIE 可调用/可分发及协程桥组合 | Apple 维护者 | 严格并发、类型/流/取消；custom cinterop/lambda 泛型限制；实际 API 调用、显式可分发构建和异机消费 | 阻断相关公开 API 或对应分发配置 |
| P05 | CPF + akinterop 与正式 SDK 的来源、工具链、互调与发布 | OHOS 维护者 | 公开可复现构建、KSP1/Gradle 配套、正式应用 SDK 与 sysroot 区分、设备互调及 Release/HAR；独立 GAV 解析；CMP 按配置验证 | 阻断该 Kotlin/ArkTS 主线配置；KNOI 扩展独立验收 |
| P06 | napi-ohos/Node-API 的工具配套、env 退出与 TSFN | OHOS/Rust 维护者 | napi crate 家族与 ohrs CLI 分别锁定；目标运行时的注册、停止、环境退出 | 阻断 ArkTS 原生路线 |
| P07 | Swift C++/CXX 对象及构建/方向/分发策略 | 原生/Apple 维护者 | direct-source 与 C/ObjC++ 包装分别验证；构建系统与 interop 依赖传播；对象移动、释放、异常和实际反向调用 | 阻断相应类型、方向或分发配置，不影响已独立通过的配置 |
| P08 | 新用户和新机器能否消费候选包 | 发布维护者 | 无开发仓库的旅程；官方/CPF 根 metadata 不覆盖；SKIE 二进制与 native 依赖完整 | 阻断正式发布 |
| P09 | 工具与边界性能基线 | 性能维护者 | 固定负载、环境、原始样本与噪声范围 | 阻断性能承诺，功能可继续 |
| P10 | 新语言能否只依赖公开扩展契约接入 | 核心/平台/验证维护者 | S0 框架探针；S1 独立 Dart C ABI 同步消费、禁用隔离与原路线回归 | 阻断“可由第三方扩展”的承诺及对应阶段退出 |
| P11 | 初始化工程能否在原生 IDE 持续开发，跨目标工具链能否共存 | 核心/平台/验证维护者 | 模板格式、宿主/IDE 准入、AGP 新 KMP/app/native 模块、显式测试、JNI 描述符策略、Wrapper、真实 IDE Run/重建及组合证据 | 阻断对应模板的“一键创建、IDE 可用”承诺和主线阶段退出；不阻断独立 SDK 能力开发 |
| P12 | 宏/KSP/装饰器与平台框架能否保持普通 API、生命周期和 IDE 体验一致 | 语言/核心/验证维护者 | 版本锁、宏组合、冷/热流停止、源码定位、诊断开关与未插桩消费证据 | 阻断对应便利配置推荐；普通 API 只有其自身协议测试通过才可独立交付 |

## 独立扩展探针

| 编号 | 问题 | 工作包与阻断范围 |
|---|---|---|
| P13 | UniFFI ArkTS beta 的版本、async/回调、env 及设备消费 | X07；固定独立锁，build-only 不计设备结果，只阻断此扩展 |
| P14 | KNOI 初始化、依赖预载、Release/HAR、env 与关闭 | X08；独立验证，不以 akinterop 通过替代 |
| P15 | 远程制品身份、传输、取消与宿主提交 | X09；截断、错来源、离线和权限负例，不阻断本地 C10 |

每项记录版本、输入、命令、结果和最小复现；SDK/设备缺失与代码失败分别归类。P01/P02 是首链前置；P03/P05 优先排查高风险，但按表中对应配置阻断。配置锁和推荐资格分别冻结，见[技术栈规范](../architecture/16-technology-stack-and-versions.md)。

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

C03 发现的宏常量导出/signedness 与 LLVM archive 检查版本问题已进入固定配方和编译断言。完整输入闭包、扩展退出/取消、SDK 资源与并发、干净候选消费继续按既有工作包验收，不由本次探针结论外推。
