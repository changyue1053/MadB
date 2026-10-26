# 接管第五批：生成资产绑定与真实页面消费

任务范围：E01/E03 的固定生成请求、实际 OutputSet 与完整生成槽绑定；并行推进 L05 的真实页面销毁/重建及共享 Engine 隔离。依据 ADR-0018、现行任务目录、扩展组合、代码生成和语言体验规范。整包条件与原 C06/C07 证据范围不变。

## 输入、隔离与责任

从实际产品根完整捕获 1052 个源文件，独立 Lab 捕获 450 个；产品逐字节匹配第四批最终集成快照。MadB HEAD 为 `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`，Lab HEAD 为 `a466501275bee1786d171199390f228f55eb38a0`，分支均为 `codex/c07-arkts`。完整脏文件副本分别位于 `/private/tmp/madb-chief-round5/madb` 与 `madb_lab`，基线只读保留。原目录和旧 detached 工作树不作为并行写入区。

生成绑定作者单写 composition 输出模块、必要的已绑定输入保留字段及专用测试；真实配方线先对照实际 reader/generator/finalizer 接口，明确可执行子范围后再实施。L05 作者只新增独立 Lab 页面生命周期消费者与 harness，并独占本轮 ArkTS 模拟器。主协调单写共享导出、既有 helper 的 crate 内可见性、catalog 和文档；独立审查者不修改实现。

## 实施边界与检查

复用既有 FrozenInputs、OutputSet、generate 任务投影、来源校验和生成身份。新增绑定结果只保存不可变数据及来源，不授予进程执行、Source 提交或认证资格。旧 profile/1 路径仍明确拒绝 profile/2；不以改名、替换摘要或伪造成功回执绕过准入。C07 的私有 backend 配置是显式源输入；现行 profile.backend_config 为 null，其真正衔接限制在旧 finalizer/C04 对 profile/1 的精确处理。

生成资产检查绑定实际字节、请求、生产者和任务身份，只有必需槽完整覆盖时返回结果；不回写本轮 composition/profile/源锁。真实声明探针与完整受控组合执行分开记录。L05 使用冻结普通 HAR 的公开 API，不重建或扩大原 SDK 的认证结论；页面销毁、新模型身份、其他消费者持续工作和停止确认必须有真实运行观察。

执行对应单元/准确负例、旧路径回归、独立反例、完整本地门禁和集成后复验；页面新增用例另绑定候选 HAR、新 HAP、SDK/设备及日志。项目所有者本轮追加授权：验证后的阶段提交可推送 GitHub，但不得触发 GitHub Actions；不创建 PR 或发布。推送前核对远端事件入口，提交包含 GitHub 支持的跳过标记，推送后另核对新 SHA 的工作流运行。未执行、跳过或 cancelled 均不记作通过。

## 结果

生成后绑定复用实际 schema、固定配方投影、源引用、任务摘要与生成身份，逐一核对请求、安装执行镜像、OutputSet 和完整槽覆盖；保留各步骤原始清单和冻结字节，不合成一个虚构 OutputSet。13 项新增测试通过；独立构造夹具的 29 项检查（4 接受、25 准确拒绝）及最终相关 64 项 Rust 回归通过。见[实现检查](chief-architect-fifth-batch-output-validation.json)和[独立复核](chief-architect-fifth-batch-output-review.json)。任意 ArtifactRecipe 及其 ArtifactResult 不具有天然信任，绑定器独立重验实际输入；返回值仍不授予执行或提交资格。

真实 C07 来源观察使用固定 Node 26.8.2 与正式工具分发中的 TypeScript 4.9.5、真实 Facade AST、生产声明投影及公开观察源码，派生普通/增强的 5/8 项有限责任及 2/4 份声明输出，禁用时重新取得普通闭包。29 项实际检查通过。独审发现的父路径及祖先符号链接逃逸已修复，原三项攻击复验准确拒绝且未写入源目录；旧失败与 003 观察保持。见[配方检查与限制](chief-architect-fifth-batch-recipe-validation.json)。本片未调用完整 driver.generate、未注册扩展包、未产生 profile/2 OutputSet，也未完成 C04 受控组合执行。

独立 Lab 以既有冻结普通 C HAR（候选 `sha256:7fce7dc8b8befbb985694ca08dfecff0b769fef5d0443608b61cc79edf1d921a`）和固定公开观察源码构建新的 Release HAP `sha256:aa676dcca3759bd6a52983cadf2393812c492d8e591b5e70f95a8c2b4161db77`。最终 004 报告绑定 46 项 Lab 输入；三轮 Router.replaceUrl 真实触发页面销毁回调，排水后创建新状态实例，其他观察和共享 Engine 持续工作。19 个原始事件、39 项实际 UI 断言、18 项宿主测试及独立控制流/证据复核通过。运行目标仅 API 23 Arm64、OpenHarmony 6.1.0.115 模拟器、社区签名；正常 SDK 关闭先于 force-stop，卸载与 PID 清理另有记录。见[独立复核](chief-architect-fifth-batch-l05-review.json)。

L05 初次连接失败、编译失败以及 003 的来源登记缺口均保留原报告。编译阶段误分类在后续实现中修复并新增反例；旧报告另有[对账记录](chief-architect-fifth-batch-l05-reconciliation.json)，不改写原结论。旧模型被强引用只用于观察，不证明 GC；确认排水后发布不证明已排队迟到回调交错。后台、旋转、关闭失败重试、其他路线和正式扩展包继续按原完整出口验收。

产品首次完整门禁在 E02 cancel-ok 探针失败，后续检查未执行。确定的缺陷是测试 watcher 等待失败覆盖真实执行结果；修复仅保留诊断并在执行结束时通知 watcher，不改变 app/host 取消实现。受控 exit23 反例证实旧探针丢失诊断并等到超时，新探针保留退出码/诊断/清理，真实成功退出但缺 readiness 也不向下游传值。原始运行提前结束的底层原因无法从已被覆盖的旧报告还原，不把后续未复现当成根因证明。见[实际修复验证](chief-architect-fifth-batch-e02-validation.json)及[独立复核](chief-architect-fifth-batch-e02-review.json)。

最终冻结实现的完整 `scripts/check.py` 本地门禁通过；Lab 副本与集成后实际根的完整本地门禁均通过。新来源观察、目标 HAP 和证据独审另有专门检查，不因总门禁通过扩大其覆盖。后续仅决定/报告文档变动，最后再执行文档与生成一致性检查。具体命令、源字节与失败记录见[验证记录](chief-architect-fifth-batch-validation.json)。

Lab 阶段源码已提交并推送 `341088a122cb36f468367f49b0f3d45d01d86a24`，继承的已验收首片源码一并保留。提交含 `[skip ci]`；已核对远端 SHA，并在 2026-10-26 09:09:11 UTC 查询到新提交的 Actions 运行数为 0。该观察不表示远程 CI 通过。产品阶段推送在最终集成后执行，不创建 PR 或发布。

## 整包出口与后续

本批只收上述子范围。41 个工作包的 status、verification、阶段、依赖和 acceptance 保持；C06/C07 完整对象不变。下一段贯通真实 profile/2 生成 finalizer、执行镜像准入、真实包注册及普通/增强/禁用组合的独立消费。I01 四项格式修订已独审并由项目所有者采用，见 [ADR-0019](../adr/0019-project-init-incremental-contracts.md)；其余 envelope、包摘要与有限配方表尚须定稿，未批准完整生产 decoder。L02/L03、工程 I 链、C08/L06 与 C09/C10 保留现行目录的联合退出条件。
