# 第十一批：扩展注册与语义验收出口

承接[第十批](2026-10-26-chief-architect-tenth-batch.md)。本记录只登记本批范围、实现与实际证据；任务状态仅在 catalog 维护。

## 输入及隔离

实际 MadB 为 `9a72a6417858b7ee1a141ce8b13fa47ba7ef2f94`，独立 Lab 为 `d270293f50c04ebdca31a4a1eaa7335b1e869cd1`，两仓均无未提交文件。完整核对1365/531项源码字节与模式，旧C07 detached工作树保留。复用 `/private/tmp/madb-chief-round10/repo` 干净隔离检出，在 `codex/round11-extensions` 分支开发；新证据写 `/private/tmp/madb-chief-round11`，不覆盖上批工具、候选和归档。实际产品根只由主协调在重核基线后集成。

## 本批范围与责任

- E03：按当前四条验收，对账模型版本、旧读取器拒绝、证据失效和按符号/阶段/实例的责任与能力前提。实施先复用现有模型、纯规则和已批准composition/1、profile/2；不以有限Source协议证据扩为全部SDK能力。
- E01：并行核对真实包注册、字节来源、依赖闭包/作用域、权限/锁和有限组合消费；具体缺口在实现前确定，独审不代替实现。
- I01/I02/I03：准备doctor具体输出契约及既有工程Wrapper/增量的最小可执行子范围。已批准责任不重问；新增公开字段、行为或兼容承诺先按既有决定流程处理。
- 主协调独占共享格式、导出、catalog、规范及集成；子任务修改边界另在本记录具体追加。原生与并发变更须独审。Swift/ArkTS沿现有L04/L05队列推进，设备和编译资源由主协调安排。

依据为各任务catalog的documents、ADR-0018/0019/0023/0024及共同工程规范。保留C06/C07/L01/L02/E02既有冻结范围，不改历史报告；不新增工作包或降低整包退出条件。

## 检查条件

先运行影响模块的真实正反例，再审查最终实现并冻结候选；涉及组合生成的改动须复验实际输入、生成、旧批次保持和独立消费。集成后重验受影响路径，缺环境或尚未执行明确记录。状态仅在全部适用验收齐备后更新catalog，HTML由源生成。阶段推送使用skip-ci，不创建PR/标签、发布或启动Actions。

## 已定位的准入差异与实施边界

独审使用第十批原冻结工具及新隔离输入，实际复现四项漏拒绝：普通配置清空依赖、增强配置将运行依赖改成验证用途、清单新增无映射toolchain依赖、清单新增无映射generator依赖，均进入双会话并提交Source。原工具、历史输入和报告保持原样。新增回归测试在修复前因“意外接受输入”真实失败，退出101。

主协调负责composition/pins、ownership与内部RecipeRequirements依赖接线：依赖用途由固定宿主配方给出，结构合法后仍须核对完整集合；复用composition/1已有六种scope，不新增wire。当前没有受信任映射的外部toolchain/generator依赖明确拒绝，锁里出现同名工具不能代替映射。该正确性修复会拒绝此前错误接受的缺依赖输入，历史候选身份及结论不重写。

E03实施者负责composition/semantics、新语义/演进检查和独立source输入组装；原CLI example仅接入推导调用。原@1配方保留其5/8项局部观察责任，新本地`source-semantics-{mode}@2`配方绑定16个实际调用及各自SDK机制来源。它复用已批准composition/1与profile/2，只覆盖Source配方声明的语义阶段；没有物理库装配执行者时不虚授该owner，不认证Native运行、HAR或页面。额外要求或责任必须准确拒绝。主协调负责配方选择和生成入口映射，测试生成输入与生产推导分别实现。

## 已批准doctor实现范围

项目所有者采用[ADR-0025](../adr/0025-doctor-preflight-contract.md)及其精确提案，进入I02现有冻结上下文的只读doctor实现。主协调负责schema、格式向量、决定和catalog；I02实施者负责app只读编排、contracts受检投影、Host部分事实观察及CLI组合，独占相关导出。现有执行安装凭据仍要求完整验证，部分观察不转换为执行许可。新增输出预算码DOCTOR001随现有explain目录维护。

Swift另在独立Lab的`consumers/swift/l04_native_ui`、专用harness及检查脚本推进macOS真实SKIE/SwiftUI页面。公开C06候选和独立Observation源导出逐项锁定；不修改C06案例目录或历史报告。新页面运行、生命周期独审与冻结Lab复验完成前，不登记运行通过。该子范围不替代iOS/UIKit、其他配置或L04整包验收。

## E01/E03冻结候选与验收

本批以14文件选择提交`fe75241697469292d7af06ff6bb4e1ce47cc4018`单独冻结E01/E03；并行doctor内容完整保留在开发检出，不混入该候选。正式检出`/private/tmp/madb-chief-round11/extensions-source`保持干净，1371项tracked字节/模式与构建前一致。13工具镜像收据SHA-256为`e3da7d727d7b3bcde226c5b52eee9c96212af96d110b294980de6760657605c1`。

新三配置完成真实读取/生成双会话、相同冻结IR传递、一次Source提交和独立公开消费。最终31项分为3个真实正例、25个相干重绑metadata/语义拒绝、1个重复责任key摘要前拒绝、2个真实reader拒绝；后两项明确未启动generator或提交Source。旧双会话三配置和保留单generator的9项矩阵分别复验，未把旧5/8项观察扩大为新责任保证。E01四项历史漏拒、独立两项同名工具依赖和七项共同准入反例均准确拒绝。

完整本地门禁002实际退出0，原001因沙箱禁止`UnixListener::bind`创建临时Unix socket失败；原日志保留，提权后按原断言完整复跑。没有删除检查或将远程Actions写成通过。新模块只读取并校验现有生命周期责任，没有修改SDK原生协议。

[最终独审](eleventh-batch-extension-final-independent-review.md)逐条确认E01六项、E03四项现行验收齐备。当前结论已按固定候选及有限范围登记catalog；[精确证据索引](eleventh-batch-extensions-validation.json)与[作者验证](eleventh-batch-e03-final-validation-001.md)保留分层结果。Source机制与消费不授予SDK/HAR、真实目标/页面或实验后端认证，后续L/I/C/X及E04承诺不因此省略，也不反向追加为本两包前置。实际产品根集成及本批其他改动后的复验继续单独记录。
