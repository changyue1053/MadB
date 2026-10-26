# 接管第二批：L01 契约验收与真实进程、页面补证

记录日期：2026-10-26（America/New_York），本批工作跨 10 月 25–26 日。本文记录具体输入、交付和检查，当前任务状态仅在[目录](../catalog/implementation-plan.json)维护；不新增任务系统或发布资格。

## 输入、隔离和责任

产品根 `/Users/changyue/Desktop/MadB/madb`，HEAD `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`；独立 Lab 根 `/Users/changyue/Desktop/MadB/madb_lab`，HEAD `a466501275bee1786d171199390f228f55eb38a0`；均为 `codex/c07-arkts`。产品未提交文档与首批代码全部保留，Lab 捕获时干净；旧 detached 工作树未动。MadDemo 不作为产品根。

本轮分别捕获产品 978、Lab 416 个 tracked/非忽略 untracked 源文件，复制前后核对 HEAD、分支、文件集合及 SHA-256。实施副本为 `/private/tmp/madb-chief-round2/{madb,madb_lab}`，各有独立 baseline/input.json。不能只从 HEAD 克隆后忽略脏输入。集成脚本逐个核对原根捕获摘要和新路径冲突，只复制本批已审查差异，保留不相关内容；不提交继承改动，不 push/PR/触发 Actions 或发布。

主协调单写共享 module 导出、检查入口、catalog、风险与 HTML 生成。L01 契约、E02 会话、E02 文件域、L05 Lab 消费及 C09 关闭窗口各自有不重叠目录；独立审查针对具体差异和最终输入。API 23 模拟器由 L05、C09、L05 最终复验依次独占，专用测试应用均清理后交接。

## L01 契约包

[共同契约](../examples/language-api/README.md)、[Swift 明细](../examples/language-api/swift-contract.md)、[ArkTS 明细](../examples/language-api/arkts-contract.md)及[十一条验收索引](../examples/language-api/acceptance.md)逐项固定声明子集、默认行为、普通替代、生成/手写边界、拒绝输入、依赖和独立预期。分别覆盖已有 ADR 的 Swift 十二项必要/九项扩展、ArkTS 十二项必要/十二项扩展，不新增工作包、案例组、wire 字段或兼容承诺。

实际正反检查包括 L01 Python 5 项、既有 C06 reader 16 项、正式 SDK parser 的 ArkTS AST 6 项，以及锁定 Xcode 上普通 Swift/Observation 消费编译、真实系统宏展开、非 Sendable 值和错误 actor 访问两项定向编译拒绝。Swift 首次普通沙箱阻止系统宏插件，失败原样保留；工具权限获准后在新输出目录完整重跑，最终脚本未覆盖 HOME。报告 `/private/tmp/madb-l01-round2-swift-final/result.json`，SHA-256 `aa224b1a7c24dc51f3dfc113303f091167ed52f95b355593693b0c35b32fa108`。

[独立审查](chief-architect-second-batch-l01-review.json)逐条核对十一项 acceptance，最终十文件与 Swift 七项输入摘要匹配、无阻断发现。契约与可执行设计样例是 L01 的交付层次；第二业务真实增删改生成、C06 固定 Facade/显式 expectedProfile 自动接线、语言增强实现、iOS/工程/正式包仍按原 L02–L07/I/C09/C10 验收，未被删除、延期为可省略或推导通过。

## E02 真实协议会话与文件域

复用既有 v1 wire/schema、C04 准入与进程 owner；新的交互端口只传 bytes，protocol 提供作者支持，contracts 负责领域转换，host 负责管道、文件与清理。一次会话只执行一个请求，result 后关闭进程，符合现行协议；不是多请求并发复用。

真实子进程完成 hello/ack/invoke、严格协商与帧序、取消、阶段/全局超时、终态后 EOF/退出和进程组清理。公共作者模块只依赖公开协议。文件域先核对精确引用闭包、配方及工具路径/摘要/大小/参数/环境，冻结传递；终态观察后等待退出并重新核对同一快照与精确输出树。read/generate/package 复用已有 C04 配方准入及输出检查，verify 核对必需覆盖及候选案例。两组件案例在上游校验后传递冻结输入，仅全部成功才提交实际 LocalBatchStore；失败和取消保持原索引。

[独立复审](chief-architect-second-batch-e02-review.json)绑定最终源码，46 项定向测试、独立 18 个执行身份向量及真实进程 11 组/44 个模式通过。最终独立进程证据目录 `/var/folders/h_/h9j1tz1n59x4mw1sdppn3_f00000gn/T/madb-e02-sessions-0fxe17tw`；七个输出负例均确实收到 result，并核对六项 OutputInvalid、一项 ToolFailed，两项慢回调案例确实进入 observer 后以 ToolTimeout 拒绝。此前部分负例提前超时却被宽松断言接受的结果保留，已由精确诊断与终态证据的新运行取代。普通夹具启动预算调整为 10 秒，专用超时负例的 200 毫秒/3 秒预算未放宽；产品策略未改。

本机为 Darwin Arm64，Linux 未执行；同步文件 I/O 或回调不会被强制打断，而是在返回后、准入前重新检查期限。进程组不是 OS 沙箱，两次快照不证明期间从未写过相同字节或未捕获早期写入。受控构建回执/两组件事务不能替代真实工具链或 SDK 消费验收。E01 包来源及精确 lock 映射、E03 每符号/阶段/实例责任与 ArkTS 有限配置/禁用模型仍缺，故 E02 整包暂不验收。

## L05 真实普通 C HAR 与 ArkUI 页面

独立 Lab 通过公开模块 re-export shim 消费冻结 C ordinary HAR，以及单独导出、独立固定 SHA-256 的两份观察源；没有导入工具私有实现、修改候选或重写 native。固定 candidate `sha256:7fce7dc8b8befbb985694ca08dfecff0b769fef5d0443608b61cc79edf1d921a`，producer `18256fcc4b26cf3cd1e2e79c39b86c2512e012d9`。

最终 `/private/tmp/madb-chief-round2/l05-native-ui-002/report.json` 的编译、Native、UI、清理分别通过，报告 SHA-256 `88c5aa862d38a05e41973c7db157d4b195896fdfca21d3c568d0c389b2c3718c`。正式 CLI `26.0.0.851`、SDK `26.0.0.105`，API 23 / OpenHarmony `6.1.0.115` / Arm64 Release 模拟器，使用社区调试签名。真实 HAP 摘要 `497683993517d4197c2ff6a81405f55576239fda831b17ff244d9e8c3a739bb1`。

检查包括双观察独立停止、共享 Engine 保持可用、close 合并和重进；真实 ArkUI V2 页面隐藏、排水、隐藏后发布、返回后的空状态与可见更新、最终关闭。九条 UI 层级文本断言来自目标 `uitest` 实际树；点击坐标来自实际节点 bounds。独立复核核对 32 项 Lab 输入、原始 nonce/PID 日志、UI 附件摘要与测试包 stop/uninstall；详见[目标审查](chief-architect-second-batch-target-review.json)。解析器与清理文本成功条件收紧后重新运行 002，旧 001 报告未改。

隐藏发布发生在观察 close 已排水后；未强制“已入队 native 迟到回调与 close”交错。页面销毁、后台/旋转、实际设备、close 故障重试、严格资源计数、其他路线与正式 C10 包仍缺。本结果不授予 L05 整包或公开发行通过。

## C09：正常关闭在物理 ACK 前拒绝重开

四份既有冻结 test companion 在同一正式 SDK/API 23 Arm64 模拟器完成新消费者补证。C/C++/Rust 使用真实 phase2 Work hold，Kotlin 使用真实 worker exit hold；两次 owner turn 均验证原 close pending、尚无物理 ACK/receipt、重开精确拒绝、旧 hold 未被新准入破坏。释放后原 close 完成、资源配平及 receipt 消费，然后第二 Runtime 实际结果为 42 并完成关闭。

最终报告为 `/private/tmp/madb-chief-round2/c09-{c,cpp,rust}-run-002/report.json` 与 `c09-kotlin-run-003/report.json`。[目标审查](chief-architect-second-batch-target-review.json)记录四份报告/候选/nonce/PID/目标，复核七个新增源文件、重解析原始日志并核对 60 个附件摘要。它们是当前未提交 Lab 源码绑定的 provisional 补证，不改变 C07 原验收，也不授予 C09 整包通过。

保留的失败：首次 ohpm 缓存写入受沙箱限制；新 Kotlin oracle 首次误用 C 路线的 `RuntimeAlreadyOpen`，其冻结 bootstrap 的准确错误为 `AlreadyInitialized`；第二次误把累计 WorkerStarts/PhysicalJoins 在第二 epoch 仍期待为 1。主协调独立核验固定 producer 的 `bootstrap.cpp.in:170`、公开 facade 错误保持，以及 `OwnedCore.kt` 计数定义、beginEpoch、worker 创建和 join 更新后，才把新 oracle 固定为路线精确错误、第一段严格 1/第二段严格 2。旧失败报告未重标，未修改 SDK 或放宽为任意错误。

旧冻结 SDK 对物理 ACK 已到但最终 receipt 尚未消费的第二窗口没有公开确定性切点。项目所有者随后确认[最小 test-only 同步 before-receipt 回调方案](2026-10-26-c09-receipt-window.md)，新候选实施和验证另记实际结果；旧 producer/candidate 字节不就地修改。该缺证项不是重新裁定 ADR-0013 的同 env 顺序重开方向。

## C09：修复后的 receipt 消费前窗口

新测试公开入口 `testBeforeRuntimeReceipt(runtime, probe)` 在已取得物理 ACK、waiters 已清零、receipt 尚未消费的同步调用栈内执行。普通 SDK 和 native owner 未增加控制点；正常返回后继续原关闭流程。普通和测试候选同 revision 配对，不能拿旧普通二进制与新测试包混配。

第一份本地冻结 `4edc8c7` 曾在 C/C++/Rust 普通错误场景通过；独立审查随后用实际生成 facade 复现 Error 的 `code` getter 再抛错，使 close pending/timer 残留。旧候选、报告及 450 份 Lab 源保留，并明确排除最终 hook 验收。修复只在测试派生层隔离用户异常：Error 原对象保持，非 Error 和分类陷阱归现有 CallbackFailure；不读取用户 code/message/then，不进入 nativeError。既有 fail/rejectClose 保留未确认清理和未消费 receipt，返回 false 退出 tick。详见[批准与实施细化](2026-10-26-c09-receipt-window.md)。

最终产品本地冻结为 `a31ac4bb9bbb7f7270e69d3848011eaeb986e981`，完整包含 1012 份继承输入、本批 E02/L01 和修复，独立 Lab 本地冻结为 `a6e04ce3e4863edc767b8279182b5b203b30ee51`、450 份源文件；两者均 clean，实际原根 HEAD 未改。新目标执行从冻结 Lab 读取源码，外部固定 ordinary/test 摘要后才准入。

[生命周期源码审查](chief-architect-second-batch-receipt-source-review.json)、[额外异常审查](chief-architect-second-batch-receipt-repair-review.json)与[清单审查](chief-architect-second-batch-receipt-binding-review.json)分别绑定 13 个源码文件、18 个原有及 10 个独立主机案例、旧源码反例和 47 项审查回归。主协调和独立审查者分别 git apply 重放派生 facade，均得到 34693 字节、SHA-256 `74fa0386bf18a24996741dda85919388c3a75bae59c9bc6fa91432324b8ef025`。固定新清单相对旧 C07 只更新 native 三项源引用及三条测试 facade 派生引用、Kotlin 两项源引用与审查元数据；其余 KSP、原生对象、编译和链接约束不放宽。

最终四路线测试候选与四份普通对照均完成实际生成、编译、链接和 Release HAR。C/C++/Rust 各执行 7 个新窗口/异常场景及 5 个既有适用回归，Kotlin 执行 7 个新场景及 4 个适用回归，另四条普通候选分别通过既有公开 API 基线。它们是 47 次测试伴随场景执行及 4 次普通基线执行，不是新增问题组或 C07/C09 全矩阵通过。Kotlin 按 main-env-only 契约检查真实线程、settlement 与关闭故障，未声称 Worker-env cleanup。目标仍为前述 API 23 Arm64 Release 模拟器与社区调试签名。

正常 hook 场景证明 ACK 已到、receipt=0 时精确拒绝重开，回调返回后 receipt=1、原关闭完成并允许新 Runtime 实际工作。六类负例验证 Error/getter、undefined、Proxy、Promise/thenable 的错误边界；它们保留 close rejection 与 receipt=0，并复查两个 owner turn 的 timer=0、callback 仅一次。负例的通过仅表示预期拒绝被观察到，SDK cleanup 明记未确认；stop/uninstall 只算设备进程恢复。

[最终目标证据索引](chief-architect-second-batch-receipt-target-review.json)绑定所有候选/报告摘要、目标、场景与清理层次。独立审查重放全部命名场景和普通原始日志，核对实际 HAP 中的 native/bytecode、归档消费者、源输入及固定配对，再以变异验证关键反例拒绝。ACK 前证据仍以旧 18256fc 候选及其报告为限，receipt 消费前证据以新 a31 候选为限；没有把两份证据偷换成同一候选的全矩阵认证。

## I01 与未变决定

I01 的[模板与 InitResult 草案](2026-10-26-project-init-format-draft.md)及 E01/E03 的[组合格式草案](2026-10-26-extension-composition-format-draft.md)已做交叉审查：显式版本准入、最终提交留 app/host、摘要按实际依赖失效。尚未采用或写入 schema；组合投影/feature 身份及普通/增强责任实例仍须具体收敛后进入既有决定流程。已有 L04/C08/L06/L07 分责及 doctor→I02、verify→C09、lock --update→C10、explain→L02 的批准继续有效，不再次申请同一批准。

本轮不改任务阶段、依赖、既有 acceptance 或 C06/C07 catalog 对象。远程 Actions 仍待项目完成后独立补跑，未执行不计通过。最终检查与原根集成摘要由本记录的配套验证文件绑定，不以生成 HTML、DOM 替身或本地检查授予发布资格。

后续实施先收敛 E01/E03 公共组合投影与具体责任实例、I01 模板/结果字段，再按既有决定流程接入；已有普通 API 和 CLI 分责不重议。L02 公共声明/诊断及 explain、L03 Kotlin、工程 I 链、L04/L05 的业务声明生成与便利层、C08/L06、C09/C10 联合验收继续按目录依赖推进。第二业务真实增删改生成仍是后续实现/集成输入，不能由本轮 L01 契约与固定 Snapshot 页面推导完成；子范围可开工也不解锁整包验收。

## 最终集成与检查

两仓库实际根的 HEAD/分支保持交接值；原有文件先逐项校验，差异守护式合入，未改动旧工作树、旧报告或发布。修复后实际产品根完整 scripts/check.py、独立 Lab 完整门禁和显式锁定 TypeScript 的 18 项 hook 检查通过；产品非文档源码与最终 1012 文件冻结输入相符，Lab 450 文件与最终冻结输入相符。最终报告/catalog/风险文本另做静态文档、目录和生成视图检查；具体命令、日志摘要、保留失败和范围见[本轮验证记录](chief-architect-second-batch-validation.json)。只将 L01 的契约包登记为 implemented/passed；E02/L05/C09 等整包状态保持原边界。
