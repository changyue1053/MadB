# MadB 构建 打包与发布

交付对象包含语言包装、原生库、运行时依赖、公开声明及 manifest。各部分用契约和内容摘要关联，避免“能编译的错版本组合”。

## 工具链锁与构建身份

锁记录工具来源、版本或 commit、校验摘要、宿主平台、目标平台、SDK、编译参数、功能开关和传递依赖。平台 fork 记录上游及补丁集合。环境变量采用显式白名单，影响输出的值进入构建身份；凭证不进入锁、日志和包。

生成器、模板补丁、运行时和构建 CLI 分别锁定，不因为属于同一生态就套用同一个版本号。工具首页列出的下一版本不能代替可获取的发布制品；缺少已知必要修复的组合不能进入对应能力的推荐配置。候选及修复准入以[技术栈选型](16-technology-stack-and-versions.md)为准。

目标编译必须使用目标的 sysroot 和头文件。C/C++ 的宏、pack、异常、RTTI、标准库、链接方式和 LTO 影响兼容性，分别记录。原生库页大小与装载属性纳入适用目标检查，不按文件名或宿主编译成功推断。

## 主机构建完成与目标运行

默认 Build 需要 compile/link/run 三种阶段全部成功。profile 只有在 required_features 明确选择 `core.external-runtime-verification@1` 时，才使用 compile/link 完成策略，Build 图仅允许这两种阶段，并在执行任何 helper 前拒绝其他阶段；只声明支持该功能不会改变默认值。选择和证据都受 profile 摘要约束，规则见 [ADR-0010](../adr/0010-external-runtime-verification.md)。

该模式由真实 Package 端口装配 HAR 等制品，独立 Lab 消费候选并执行必需目标用例。构建提交与目标验收保持分离；没有目标报告就不能通过认证。准备/重放中的 helper 不承担签名、安装、设备启动或退出清理，不制造占位运行证据。现有 C05/C06 默认三阶段与父证据完整性规则保持。

## 制品形态

| 消费者 | 包含内容 | 必须验证 |
|---|---|---|
| Android/JVM | 语言 jar/AAR、ABI 对应 native 库、必要规则 | 实际装载、Release/R8、反向调用、最低系统 |
| Kotlin/Native | 对应目标的 klib/框架与原生依赖 | 目标链接、运行时重复、导出边界 |
| Swift | SwiftPM 源码包装或 XCFramework 及 module 配置 | 真机/模拟器、Release、声明支持的 Xcode 组合 |
| ArkTS | 类型声明、HAR/原生库和构建配置 | 实际 HAP 消费、注册、装载与运行环境 |
| Rust/C/C++ | crate、头文件、静态或动态库、CMake/package 信息 | 编译参数、符号、运行库和真实反向消费者 |

源码包装和预编译模块具有不同的工具链兼容要求。静态库也会带入全局状态与运行时，不能因为“没有动态库”就忽略重复实例。

## 平台发布配方

### Android 的共享模块与原生制品

采用 AGP 9 的 KMP 共享库使用 `com.android.kotlin.multiplatform.library`，Android 应用独立成模块。这个插件支持包装已有原生制品，但不提供 Android 的 `externalNativeBuild` 或多 build type/flavor；初始化必须选择以下明确配方：

| 原生制品来源 | 包装与任务依赖 |
|---|---|
| Cargo/MadB 已构建的 `.so` | 通过所选 AGP 版本支持的 `jniLibs` 源接入点登记输出目录和构建依赖，核对 ABI、传递库和重复打包 |
| 需要 CMake/NDK `externalNativeBuild` 或 Android 多变体 | 使用独立 `com.android.library` 模块；KMP 的 androidMain 消费其公开依赖 |

SDK 发布必须把原生库及其必需运行依赖交付给消费应用，不能以示例 app 手动复制 `.so` 作为完成条件。KMP host/device 测试显式启用；Lab 从发布 AAR/依赖入口验证加载、R8 与反向调用。[Android KMP 插件](https://developer.android.com/kotlin/multiplatform/plugin)、[AGP jniLibs 接入](https://developer.android.com/reference/tools/gradle-api/9.3/com/android/build/api/variant/Sources)

### 官方 Kotlin 与 CPF 的发布身份

首版采用官方 Kotlin 与 CPF **独立 Maven 坐标**，由模板按构建根映射依赖。两个根分别发布自己的根 metadata 和目标制品，锁中记录实际坐标、变体及摘要；不得向同一 group/artifact/version 重复写入根 publication。

公共 API 和兼容源码可以共享，生成 SDK 的身份与二进制仍按工具链区分。若未来提供统一坐标，须由单一发布编排汇总根 metadata、目标引用和完整性检查，并作为新的发布配置验收，不能依靠两个发布任务先后覆盖。Lab 使用普通 commonMain 依赖入口验证目标解析，不能只直接引用单个 OHOS klib。[KMP 发布结构](https://kotlinlang.org/docs/multiplatform/multiplatform-publish-lib-setup.html)

### Swift 的分发与 C++ 接入

包含 SKIE 的二进制 SDK 发布模板显式配置 `produceDistributableFramework()`，检查 Swift library evolution、module 信息、静态 framework 调试引用及源码路径；不依赖 XCFramework 构建时的隐含默认行为。独立消费工程必须在另一目录或机器实际调用承诺的公开 API。[SKIE 分发配置](https://skie.touchlab.co/configuration/swift-compiler)

Swift/C++ 源码共同构建且工具链受控时可采用 direct interop。面向希望隐藏 C++ 构建条件的 Swift SDK，C/ObjC++ 包装是正式发布配置。公开 C++ 类型及 interop 构建要求会影响消费者兼容性，不能仅靠增加一层 Swift 类型别名消除。SwiftPM 配置不得默认承诺 C++ 消费 Swift API；该方向使用已验证的 Xcode/自定义构建或明确 C 回调配置。[Swift C++ 工程支持](https://www.swift.org/documentation/cxx-interop/project-build-setup/)

## Manifest 与版本

manifest 至少记录 package_id/version、schema_version、contract_digest、policy_digest、profile、运行时和调用方向、交付形态、构建系统、tool/runtime/backend/toolchain 标识、targets、文件摘要、依赖、构建来源和适用的 evidence 引用。Maven 等包管理系统的坐标与平台变体单独记录，不从展示名称推断。

工具、运行时、后端插件、SDK 包独立版本化，并声明兼容范围。API、ABI、行为和 wire 协议分别评估兼容性；改回调线程、取消效果或成功阶段也属于兼容审查。新产品从首次公开发布起承担自己的版本契约。

使用扩展时，锁和构建身份还包含扩展包/清单摘要、组件绑定、协商后的 protocol/IR 与必需功能。扩展清单是安装和能力声明，SDK 制品 manifest 是交付内容记录，两者分别管理。ArtifactPackager 只形成候选，宿主校验后提交；认证不得由扩展清单自报。

## 两工程的发布闭环

1. MadB 从固定 revision 和锁构建候选，冻结包与摘要。
2. MadB-Lab 使用候选包和独立固定 revision，在干净消费目录运行目标套件。
3. 报告记录每个目标、用例、制品摘要与环境，不修改候选包。
4. Gate 核对必需问题覆盖、用例结果、证据有效性、许可和包完整性。
5. 发布已验证候选的相同内容；不重新生成或重编译后直接沿用原证据。
6. 从公共发布通道下载并校验一次，运行安装冒烟测试；失败时停止推荐该版本并发布明确状态。

工具发布与被生成的用户 SDK 发布是两个流程。MadB 自身版本通过通用 Lab 验证；用户 SDK 仍需其领域测试与目标验收，通用工具不替用户决定发布权限。

## 完整性和可复现性

文本生成要求相同输入逐字节一致；二进制可复现性按工具链测试能力记录。无法做到字节一致时仍须固定输入、记录构建来源和差异原因，不能宣称 reproducible。

打包禁止包含开发机路径、凭证或未声明依赖。下载和外部命令只使用锁中来源。最终分发包包含许可和必要第三方通知，调试符号作为可关联的独立产物保存。

## 模板和 IDE 工程交付

工程模板与模块配方有独立版本、摘要、工具兼容范围和认证记录。发布工具包后，Lab 从候选模板实际运行 init，再验证 IDE 导入、构建、运行及共享代码变更；已有 Demo 编译成功不能替代该步骤。

生成工程附项目 Wrapper、模板来源、文件所有权与完整依赖锁；IDE 和 CLI 使用一致的工具版本。配置同时声明宿主 OS、IDE 版本及目标 SDK，而不只声明编译器版本。推荐模板的 [INIT 门禁](../validation/06-project-initialization.md)是产品发布条件，专用 IDE 向导不是基础工程可用性的前置条件。模板和 Wrapper 的许可及下载来源也进入分发审查。

## 远程存储扩展与唯一内核

X09 在 C10 本地候选存储之上增加可选传输适配器。下载先落 staging，宿主核对清单、摘要与可信来源；上传需要明确授权，上传成功不等于发布或认证。离线使用匹配的本地内容；重试只适用于可重放传输，同一内容键不得覆盖成另一份字节。取消先关闭本次提交资格，再停止写入，具体交错见[关键场景](../validation/08-critical-scenarios.md)。

该服务通过宿主存储端口接入；当前扩展进程协议没有 storage 操作。若要交付第三方独立存储进程，X09 先完成协议演进及 StorageRequest/Result schema，不能挪用 package 消息隐藏下载、上传或发布。C10 与 S5 不依赖远程服务。

组合应用的单一内核通过真实 owner 身份验证：从两个入口访问同一测试实例和计数器，关闭一个观察者不影响另一入口；最终关闭一次。构建时另查链接清单和导出符号。两个入口都返回正确 checksum 不能证明它们没有各自创建一套内核。

## 组合应用的装配合同

DEC-09 将共享内核的创建放在应用组合根；各语言入口通过受检适配器连接该实例。默认把需要共享的内核及原生导出入口放入同一 native 链接单元；每个后端仍管理自己的 wrapper/引用，不互相解释裸句柄。组合图记录实例 owner、factory、借用入口、观察者、关闭顺序、静态/动态库及符号命名空间。

多个动态库需要共享时，必须显式选择单一核心库及稳定的服务接口，每个适配库只依赖该核心，不各自静态嵌入一份。该配置另验版本/来源握手、装载寿命、句柄交接和唯一实例。工具链不支持所需组合时返回 CapabilityUnsupported；分别通过的后端不自动组成可用模板。

独立业务实例可以并存，单一 owner 针对装配契约声明共享的那一个实例，不等于引入全局单例。CRIT-04 同时检查对象身份、状态互见、创建/释放计数、链接清单及关闭一个观察者后的另一入口行为。

## 消费端的匹配检查

DEC-10 将文件完整性、API/ABI 兼容和运行装载分成三层检查。

| 层次 | 必须检查 | 能证明的范围 |
|---|---|---|
| 解析与打包 | 锁、目标、生成批次、依赖、文件与候选摘要 | 选中并交付的是同一候选 |
| 消费编译与链接 | 实际公开符号、布局、调用约定、运行库和链接清单 | 此目标的公开入口可编译和链接 |
| 初始化与实际装载 | 后端现有 ABI/checksum 协议、实际库身份及 profile 要求 | 当前装载对象满足声明的检查；不能用包文件名替代 |

UniFFI 配置复用其自带版本/checksum 机制，再核对 MadB 策略/profile 身份，不重复发明一套 UniFFI ABI。MadB 控制的首链 C ABI 提供固定签名、无分配的 bootstrap 版本/接口摘要查询，在首次资源创建和业务调用前比较；查询缺失或不匹配即失败，不能先调用未知业务签名试探。外部既有 C 库不可改时，采用该库已有查询或受管装载/文件检查，并明确剩余前提，不能宣称所有第三方库均能运行时自证。

静态链接配置使用链接记录和内嵌身份；动态配置同时记录实际解析的模块。接受兼容升级必须有明确的兼容规则及新消费证据，不能因为包主版本相同就忽略 ABI/profile 变化。不同职责的错误分为 ArtifactMismatch、AbiMismatch、ProfileMismatch；SDK 使用者不需要安装 MadB CLI 才能执行必要的运行检查。验证见 CRIT-12。
