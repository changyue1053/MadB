# 接管第六批：L02 收口与真实组合接线

任务：以 L02 五项既定验收为主线，推进正式 explain、普通资源/回调、诊断开关与责任表；并行实施 ADR-0019 的 I01 内部纯选择规则和 ADR-0018 的真实 profile/2 生成接线。L04 延续先核对真实 Apple 工具、候选和最小可验收消费范围。依据当前 catalog、适用 architecture/validation、ADR-0018/0019 和共同质量规则。

## 输入与责任

实际 MadB HEAD `bc32a61e0dd0fa4f35e57421c309619304b97381`，Lab HEAD `341088a122cb36f468367f49b0f3d45d01d86a24`；分支均为 `codex/c07-arkts`，捕获时工作区干净。完整捕获 1076/460 个源文件，复制前后核对 HEAD、分支、状态与逐文件摘要；隔离工作副本位于 `/private/tmp/madb-chief-round6/madb` 和 `madb_lab`，基线只读保留。旧 detached 工作树不写入；上轮 agents 已完成，不复用它们的旧写入范围。

主协调单写 catalog、共享导出、共享契约检查和审查记录。I01 作者独占新增 project/selection.rs 与其专用测试；E 作者先负责 native finalizer 的窄独立 /2 接线及专用测试；L02 格式与原生契约调查先只读，新增公开格式/API 必须形成具体差异并经既有流程决定。独立 Lab 与 UI 源码在实际接口和候选明确后另定范围，设备独占，不并行争用。

## 范围与检查

已有四项 I01 决定不重批；内部纯选择复用现有 ProjectPlan，不引入第二套 planner、文件系统提交许可或完整未批准 wire。profile/2 不降成 /1，不把输入观察当执行令牌；真实生成、会话、事务及独立消费分别记录。原生生命周期或并发改变独立复核，首次公开接口有编译及消费正反例。

L02 目标按 catalog 全部适用 acceptance 检查；当前旧 C05 测试专用 sink 不能冒充生产 hooks，旧样例的显式内部身份接线不能自动当作普通构造体验。新增代码复用既有稳定诊断和来源，关闭可选采集不关闭资源/线程/状态机保护。缺项不删、不放宽。

按风险执行模块/生成/真实消费者/独审与集成后检查；最终源守护合回原根。状态只写 catalog，HTML 改目录/模板后生成，历史报告保留。阶段完成可提交并推送，须不触发 GitHub Actions；无 PR、标签或 SDK 发布。

## 实际结果

解释命令按已批准 [ADR-0020](../adr/0020-explain-query-contracts.md) 实现，共用51码目录和两种闭合格式。纯码查询不读取工作区，来源查询核实际字节及 byte-lf/utf16-js 位置，未知码和0/2/3/4退出有正反例；旧六命令回归保留。独审实际执行21项 CLI 场景、逐个51码及14项读取分类检查；真实 reader 两种拒绝到10次文本/JSON查询也已重放。见[独审记录](sixth-batch-explain-review.json)。首次格式向量的未知版本期望误用 FMT004，按实际既有 FMT003 修正并保留失败记录；未放宽 decoder。

SDK 按已批准 [ADR-0021](../adr/0021-native-diagnostics-and-checked-constructor.md) 增加固定64槽生产诊断与生成 checked constructor。默认/test/diag/combined 的实际导出为7/14/9/16；默认 ABI 投影与原基线相同，诊断不是旧测试 sink 的改名。原生独审覆盖输出清零、满载、并发开关/拉取、回调内控制及 self-wait，私有确定交错另证明关闭入口不采集、争用丢弃及 poison 失败保留。见[原生独审](sixth-batch-native-diagnostics-review.json)。这些是绑定源和临时候选的实现证据，正式冻结候选及独立运行另记。

DEC-07 的有限责任比较下沉到现有 madb-rules 纯函数，保留旧 composition wrapper 的错误顺序；适配层从七/九个真实 ABI role、受检 IR、effective policy 和冻结 SDK 来源分别建立 required/provided 集合。Source `responsibilities.md` 的27/33行必须匹配核心检查后的字节，宿主再核实际输出；旧 finalizer envelope 与公共 wire 不增加字段。五配置的005开发候选已完整经过 metadata/generate/compile/link/run/package，四个 Rust-provider 候选都带同一 Source 附表；反向 C 配置不套此表。见[005生成记录](sixth-batch-responsibility-generation-005.json)。该记录之后的 Package 防绕过修复及正式候选将单独验证，005不冒充最终冻结验收。

I01 按 ADR-0019 实现显式 profile/targets 的内部纯选择，不引入完整未批准的 template/InitResult decoder；新增12项与原9项检查通过，并由独立输入复验。见[实现](sixth-batch-i01-validation.json)和[独审](sixth-batch-i01-review.json)。包摘要、有限配方及其余工程初始化出口仍按当前目录执行。

E01/E03 普通 ArkTS 配方现在经过真实 AST、profile/2 finalizer、ContractIR、task digest、生产生成体和生成后槽绑定。006的22项及独立007的16项检查通过；131份输入、10份锁定 schema、1槽15份 payload 与 OutputSet 两次逐字一致。新说明目录已纳入真实 helper 闭包，改字节准确拒绝；20种内置格式注册另有向量检查，不把10份当前锁误写成20份。旧 /1 入口继续拒绝 /2。见[最终生成验证](sixth-batch-composition-generation-validation.json)、[独立复验](sixth-batch-composition-generation-review.json)及[native finalizer 原独审](sixth-batch-native-composition-review.json)。本片仍不包含真实包注册、E02组合会话或 C04组合提交。

L04 使用固定公开 MadBObservation 源包，独立 Lab 在真实 iOS 模拟器运行 SwiftUI/UIKit 各三轮进出、新实例及共享源消费。001通过后复核发现停止完成提前发布风险，修复只在清理结束后通知等待者；002编译失败保留，003的确定三等待者屏障、故意提前通知反例及 XCTest 1项/0失败/0跳过均通过，62帧观察和清理结果已独审。见[冻结输入](sixth-batch-l04-ui-inputs.json)和[独审](sixth-batch-l04-ui-review.json)。环境为 Xcode27.0/27A266a、Swift6.4、iOS26.5/23F77 模拟器；使用单独创建的目标并在完成后删除，没有操作已启动的 MadDemo 设备。此证据只覆盖本 Swift 源包/UI 配置，不代表 C06 native、所有 Swift 便利层或完整 L04。

L02 Lab 的 Mac 消费者经过27项独立 Python oracle 检查及13项 Lab 解析/拒绝/覆盖单测；精确字段漏拒和诊断候选借用其他头文件错库证据的缺口已修复，已实现两种头文件各自的两对真实错库用例，实际运行仍待正式候选。见[源码冻结](sixth-batch-l02-lab-inputs.json)、[独审](sixth-batch-l02-lab-review.json)。源码已守护集成，实际根门禁通过，干净 Lab 提交为 `a6ec8585f61fa4fc83a534b6611b38260c13661b`；该提交包含 `[skip ci]`，本记录时尚未推送。源码/开发编译不计为正式 SDK 消费通过。


独审实际复现两处打包漏洞：重算自洽 Candidate 后可省略责任表；第一处修复后，切换当前 reader 标签仍能绕过。最终以 Source input_digest 内固定 backend 的 provider/semantics/source 事实锚定保留条件，必须保留既有表的 path/hash/size；历史无表 Source 和其他 SDK 不被授予新保证。修复通过普通/诊断的真实 Source 与完整 Source/Build 父批次检查，新增三个永久回归；核心312项、实际 Source 30项检查及候选附表复核见[最终独审](sixth-batch-responsibility-review.json)。不靠改输出期望或放宽宿主校验消除缺陷。


新增责任拒绝分支落地后，说明目录仅补充 NATIVE006 的 triggers/repairs，其他50码和格式不变；最终目录 SHA-256 为 `d27b38cf41be605c3834e4ad45a9a2e9f0e6c7af04ce1459005f94e7bdb17b4b`。51码、10项 reader 及10次真实 reader→CLI查询复验通过，见[补充独审](sixth-batch-explain-responsibility-review.json)。因此 ArkTS helper 源身份随之重冻；最终008的22项与独立009的13项检查通过，真实15份 payload、OutputSet及槽绑定逐字一致，见[最后生成验证](sixth-batch-composition-generation-validation-002.json)与[复核](sixth-batch-composition-generation-review-002.json)。006/007原报告不改写。

产品完整本地门禁001因新增 host 测试中的两处 Clippy 写法失败，修复保留相同断言；002通过后完成上述打包修复，003完整门禁再次通过，并将 SDK 默认/test/diag/combined 四种配置纳入现有 native unit gate。003启动后仅说明目录两个字段及文档变化：说明数据已另行执行上述CLI/reader/真实生成复验，文档另跑一致性检查。Lab隔离副本及实际集成根的完整门禁均通过。DOM形状单测不计为真实浏览器验证；正式SDK候选与独立消费仍分开执行。
