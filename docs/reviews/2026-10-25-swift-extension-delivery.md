# Swift 扩展计划与 Observation 首片

日期：2026-10-25。设计基线：0.9。起始 MadB HEAD：`0fd4594092a2c6646e8dc01a505d189340e93d91`，包含开始整合时已有工作区。实施范围为 L01 的 Swift 契约细化、L04 Observation 首片及九项扩展的完整计划；不新增工作包，不改变 C07 的实现和验收范围。

## 决定与完整计划

项目所有者已授权推进选定 Swift 扩展，实施负责人安排顺序；本次授权替代先前仅设计采用、由所有者逐项决定开工的 Swift 口径。ArkTS 新增需求留待后续裁定。

[ADR-0015](../adr/0015-swift-extension-delivery.md)集中维护九项选型与排除方案，[实施计划](../delivery/01-implementation-plan.md)和[目录](../catalog/implementation-plan.json)维护交付归属：先完成普通观察与 Observation，再接 SwiftUI/UIKit、工程与所需薄宏/属性封装；Combine/Rx 的 Kotlin 正式组合由 X01 收口，官方 Swift export 属于 X02 独立实验；诊断优先显式 hooks 与系统工具，定制编译器深改写不进入默认承诺。

每项已选能力的出口包含契约、实现/生成、IDE 构建、版本与公开包、独立正反消费、升级与禁用及对应证据。X01/X02 保留 C11 的正式验收前置，不能反向成为 L04/L07/C11 的新依赖。所有 41 个工作包保留原 phase、depends_on、日期和证据，L01/L04 仅更新为 `in_progress / missing`；没有把首片算成整包完成。

## 实际实现与边界

新增 [MadBObservation 源码包](../../sdk/swift/observation/README.md)，复用 Apple Observation 和现有普通订阅操作，没有新增第三方依赖或跨语言资源内核：

- `MadBStateModel<Value: Sendable>` 在 MainActor 保存本地展示快照；初值为 `nil`，当前 SKIE 状态订阅没有初值重放。
- 显式 `start(using:)` 在准入后才构造一次 source；初始化、getter 和重复启动均不偷偷取得订阅。
- `requestStop()` 关闭本会话接收；`stopAndWait()` 是 owner 的排水等待，调用者取消不使它提前返回。清理任务不继承收集取消或 backend 回调的 TaskLocal 上下文。
- 停止失败保留 source 与错误，可重试但不可重新启动；确认排空后才允许新会话。页面模型不关闭共享引擎，析构只提供尽力停止，不充当异步关闭证明。
- 可选 SKIE Integration 只使用固定 C06 公开 facade 的 state/collect/stop/drain；第三方后端和 SwiftUI/UIKit 没有成为基础源码包依赖。

独立代码审查找到并修复启动/完成属性通知期间的同步重入：启动期间停止请求需要记账，完成通知结束前不能允许新会话覆盖。对应测试使用真实 Observation 通知，另验证工厂重入、取消后排水、重试及弱引用释放。

当前源码包可独立构建。开发组合使用已有 C06 公开候选和新增 extension 源码，各自固定摘要；不修改旧候选身份，不把本轮结果写成新的 C10 候选认证。iOS、tvOS、watchOS 的清单部署声明不等于实测支持；SwiftUI/UIKit、完整 XCFramework、真实渠道安装和远端 CI 仍待对应工作包交付。

## 一致性与降噪

选择与取舍集中于 ADR；接口及生命周期在架构 15，跨路线组合/隔离在 17，版本条件在 16，工程和分发在 09/14，验收在 INIT/DX 文档。计划、任务目录、首页和进度模板同步本次授权；不重复抄写上游调研正文或维护第二套进度数字。新检查入口见[验证说明](../../tests/integration/l04_swift_observation/README.md)。

保留历史 review、校验 JSON、旧来源、公共格式/向量、路线认证和 C07 并行文件；本次只写入经过起始摘要比较的变更文件。十三页 HTML 从模板和目录重新生成，不手改生成结果。

## 验证记录

使用仓库固定 Python 3.14.4、Node 26.8.2、Rust 1.99.1 与受管 Apple Swift 6.4 / Xcode 27.0。配套[检查记录](swift-extension-validation.json)保存精确工具、候选身份、输入与变更文件摘要。

| 检查 | 实际结果与边界 |
|---|---|
| Swift Release 严格并发、warnings-as-errors | 14 项 Swift Testing 通过；停止、错误、重试、弱引用释放、TaskLocal 和同步重入均有确定性交错 |
| 重入修复的反向验证 | 在独立副本撤回修复后，3 条新回归测试均失败；没有修改正式测试预期 |
| 独立源码包消费 | 新 SwiftPM 消费工程仅依赖公开产品，通过状态接收、重复注册拒绝与停止检查 |
| SKIE 开发组合 | 校验固定公开候选的 58 个文件后复制公开制品；真实状态初值、待处理合并、快照、重复 start/stop、restart、共享引擎继续调用及资源/载荷/waiter 归零通过 |
| 候选准入正反例 | 8 项通过，覆盖错身份、受信摘要不符、修改字节、目录外路径、符号链接、额外文件及缺失候选 |
| 全量文档 | 静态一致性通过；21 项工具测试通过；13 页生成结果与输入一致；DOM 替身覆盖 29 配置、173 链路筛选、87 时序与 3044 次节点键盘选择 |
| 工作包与历史保护 | 41 项依赖无环；仅 L01/L04 状态开始，verification 仍 missing；112 份基线目录、格式、来源和历史检查文件字节未变 |
| 通用仓库回归 | 完整入口运行到既有 C07 原生清单差异后停止；此前 Rust fmt/check/clippy、285 项 Rust 测试、373 项 Python 用例及对应向量/CLI 检查通过；其后 6 项检查另跑通过，含 21 项架构测试 |

通用仓库门禁没有全绿：起始副本已有 `sdk/rust/arkts-bridge`，但同一副本的 `check_native_sdk.py` 尚未登记它，报 `NATIVE001`；在未加入本轮 Swift 改动的 baseline 副本复现。并行 C07 随后更新的检查器对当前工作区清单验证通过；用它复核隔离副本时，bridge 引用 provider 源文件的 Rust edition 格式差异仍使 fmt 失败。因此未完成的 native 单元门禁继续记为缺口，没有修改 C07 源码、删除包或放宽门禁来消除失败。

首轮宿主沙箱曾阻止既有 Unix socket 测试与 Swift Release 的 dsymutil；获准在相同隔离副本重跑后相关检查通过，未禁用调试符号或删减断言。Swift 检查器独立执行，缺候选/工具明确失败；不把未请求 native 的包检查计为 native 通过。

真实浏览器布局复核未执行，保留此前本机 `file://` 访问被工具策略拒绝的边界，未尝试其他入口绕过。DOM 替身不代表真实浏览器，macOS 运行不代表最低 OS、设备、iOS/UIKit 或完整远端发布验证；本次未发布或新增平台认证。

## 写回与后续

规范、实现及验证文件从隔离副本逐文件与起始摘要比较后写回，已有文档工作和并行 C07 内容保留；写回后重新检查实际文档、源码/验证摘要和任务保护字段。运行期间实际仓库已有新的 C07 提交，起始副本的仓库回归不覆盖该提交后继续变化的 C07 工作。

当前完成的是九项扩展的文档与完整计划，以及可验证的 Observation 开发首片。SwiftUI/UIKit 工程、选用薄宏、响应式正式组合、官方 Swift export、发布收口等仍按原工作包推进；本报告不表示全部 Swift 扩展完成，也不启动 ArkTS 新需求。
