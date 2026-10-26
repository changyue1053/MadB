# ADR-0016 ArkTS 生产能力与公开交付基线

状态：设计采用，已授权实施；实现、目标验证和发布资格分别验收。日期：2026-10-25。适用基线：0.9。关联 [ADR-0011](0011-arkts-native-protocol-owner.md)、[ADR-0013](0013-arkts-runtime-and-completion.md)、[ADR-0017](0017-arkts-extension-delivery.md)及 L01、C07、L05、I05、C09/C10。

## 决定与范围

MadB 的 ArkTS 产品面向公开 SDK 作者和独立消费者。作者从受支持的真实声明生成普通 API；消费者通过平台开发环境安装公开 HAR，不依赖作者仓库、MadB、Cargo 或 CPF Gradle。需要修改 Native 的作者使用独立源码构建配方。工具发行与生成的业务 SDK 发行分别版本化和验收。

本次已授权选型、完整实施计划、验收细化及扩展实现，由实施负责人按依赖安排顺序，无需再次逐项申请开工。该授权不产生目标认证或发布结果。C07 原有四路线、最低完整 profile 和约定回归继续执行；独立 L05 子范围不修改其协议、状态策略或容量预算。

| 路线或交付 | 采用方案 | 责任与限制 |
|---|---|---|
| C/C++ ↔ ArkTS | 平台 Node-API，加必要生成与接线 | 复用 ADR-0011 私有原生协议；业务 provider 和各路线证据分别维护 |
| Rust ↔ ArkTS | napi-ohos 注册/类型前端与 Rust provider | 按所选 feature 锁定，复用已定生命周期协议；上游 async/stream 存在不等于当前 profile 启用或通过 |
| Kotlin ↔ ArkTS | CPF Kotlin + akinterop | 独立 managed owner，匹配 helper/runtime/KSP/补丁；官方 Kotlin 与 CPF 独立构建和发布；CMP 仅在共享 UI 配置中启用 |
| 已有 Rust C ABI | 复用 C ABI，按需 cbindgen 头文件及平台网关 | 不将两段独立桥接自动计为 Rust/ArkTS 直接路线认证 |
| 默认公开 SDK | ArkTS 源码门面、声明、预编译 Native 及必要依赖组成 HAR | Native 二进制不等于字节码 HAR；HSP、字节码或源码联编属于独立配方 |

具体版本与目标由[技术栈](../architecture/16-technology-stack-and-versions.md)和实际锁管理。上游主分支能力、正式发布包、目标 SDK 可用性和 MadB 验收结果分别记录，不由任一层推导其余层。

## 十二项必要能力

序号是讨论索引，不新增工作包、案例编号或完成率。必要指产品在适用声明和承诺 profile 下必须正确承担责任，不要求所有函数引入同样对象层级；也不能将 C07 最低完整 profile 的异步、观察或退出要求全部标为不支持来绕过验收。

| 能力 | 必须交付及验收的结果 | 归属 |
|---|---|---|
| 1 支持范围 | 产品/应用 SDK、工具链、ABI、最低系统、正反方向、主 env/Worker 和分发配方明确，未知必需能力准确拒绝 | C07、C10 |
| 2 真实声明生成 | 读取真实符号及显式策略；第二业务模块新增/改名/删除后可重生成并消费；拒绝给出源码位置，不靠固定样例映射 | C07、L01、L05、L07 |
| 3 类型与错误 | 数值范围、空值、Unicode/字节、容器、复制/借用及错误类别有定义；保留调用者 getter/listener 异常；类型断言不代替实际校验 | C07、L05 |
| 4 资源所有权 | 分配、引用、任务和订阅有唯一释放责任，关闭幂等且有后条件；纯函数/DTO 不强制 Runtime/Engine/close | C07、C09 |
| 5 env 与线程 | 所属环境和合法线程校验、有界回投、停止准入；正常关闭与系统 env teardown 分别验证，无法执行 ArkTS 时仍回收 Native 责任 | C07、C09 |
| 6 异步取消 | Promise 结果、取消请求、工作停止、结果释放及关闭完成分别有语义；迟到结果、重入与终态竞争有独立反例 | C07、L05、C09 |
| 7 状态事件 | 初值/重放、合并、顺序、容量、满载反馈及停止排空明确；不将 state 属性用于静默丢弃离散事件 | C07、L05、C09 |
| 8 工程接入 | DevEco/CLI 共用受管构建；生成源可索引，增量生成、已有工程接入、升级/禁用及失败恢复通过 | I05、L05、I06 |
| 9 可复现构建 | 固定源码、工具、依赖、SDK/sysroot 与选项；官方/CPF 互不覆盖；Native 依赖、符号和装载闭合；重放与字节一致分别实测 | C07、C10、I05 |
| 10 完整 HAR | 普通门面、声明、所选 ABI 的 Native、运行依赖、许可/符号策略、必要消费者混淆规则与兼容元数据一致 | C10、I05 |
| 11 独立消费 | 从最终候选安装，验证 Release/混淆、实际装载、类型错误、取消/退出/满载、错库/ABI/profile；宿主、模拟器和真机证据分开 | C07、C09、C10 |
| 12 公开维护 | 工具/SDK 分别版本化；公开依赖、API 兼容、基本诊断、来源摘要、许可、匹配符号、升级和原候选发布后下载验证齐全 | C09、C10、C11、I06、L07 |

接口、类型及失败行为分别由[数据规范](../architecture/04-data-and-errors.md)、[生成规范](../architecture/07-code-generation.md)、[运行时](../architecture/08-runtime-and-idiomatic-api.md)和[语言体验](../architecture/15-developer-ergonomics.md)维护；工程和发行由[初始化](../architecture/14-project-initialization.md)及[打包规范](../architecture/09-build-packaging-release.md)维护。本文不复制各协议状态机。

## 必要责任与具体实现分开

C07 的 Runtime→Engine→Operation/Subscription、首次终态认领、完成账本和有界 state/event 策略继续按 ADR-0013 执行；它们不是所有 ArkTS SDK 的强制类层级。同步函数可以调用级管理资源，长期资源才引入对应 owner。单 env/活动 generation 和固定容量是当前 profile 的限制，不是全产品永久上限。

状态语义不能凭外观相互替换。StateFlow 的当前值与重放不等于 C07 无历史重放通道；映射必须保留源契约或显式说明转换。页面拥有订阅，应用拥有运行实例；页面离开请求停止，不伪造同步排水完成，也不关闭共享 Engine。

必要适配复用已有后端；上游未提供的停止、排空或释放保证由明确协议补足。Promise、TSFN、装饰器、Sendable 或 AbortSignal 名称均不赋予额外安全保证。类型/功能支持范围与完整 profile 的最低验收分开。

## 交付与准入

L01 收敛声明子集和普通/便利分工，C07 完成四路线和最终候选，L05/I05 交付正常 API 与工程，C09/C10/L07 以独立消费及正式分发收口。依赖按现行目录解释，不新增等待本次所有扩展的主线门槛。E01–E06 提供公开扩展接入，具体增强见 ADR-0017。

所有验证复用既有 INIT、DX、CRIT 和路线用例。第二业务模块不能修改私有映射或手改生成文件；最终候选与实际 HAR/DSO 字节必须绑定。build-only、源代码检查、装饰器编译和协议替身均不能替代目标运行、ArkUI 刷新、Native 退出或正式渠道消费。

来源：[Node-API](https://github.com/openharmony/ace_napi)、[线程/生命周期规范](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/napi/napi-guidelines.md)、[napi-ohos](https://github.com/ohos-rs/ohos-rs)、[akinterop](https://gitcode.com/CPF-KMP-CMP/akinterop)、[KSP](https://kotlinlang.org/docs/ksp-overview.html)、[HAR](https://github.com/openharmony/docs/blob/master/zh-cn/application-dev/quick-start/har-package.md)、[OHPM 元数据](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/ide-hmos-oh-package-json5)。这些资料是选型依据，不是 MadB 认证记录。
