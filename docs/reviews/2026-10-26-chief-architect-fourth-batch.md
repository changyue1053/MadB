# 接管第四批：已批准组合格式的实现

记录日期：2026-10-26。任务为 E01/E03 公共格式、纯输入绑定与责任比较子范围，依据 [ADR-0018](../adr/0018-extension-composition-formats.md)、提案修订3、现行模块/格式/扩展与质量门禁规范；不改变整包验收、路线或发布要求。

## 输入与责任

第三批完整门禁和独立复核后，39 项差异已守护合入产品根。本批从实际 MadB 根完整捕获 1036 个 tracked/非忽略 untracked 源文件；Lab 450 个源文件保持原样。复制前后核对文件集合、SHA-256、HEAD、分支和状态。MadB HEAD 为 `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`，Lab HEAD 为 `a466501275bee1786d171199390f228f55eb38a0`，分支均为 `codex/c07-arkts`。

实施副本为 `/private/tmp/madb-chief-round4/madb`，完整保留所有继承的未提交输入。公共格式线单写 schema、protocol 登记和结构向量；语义线单写新的 composition 适配模块及其测试。主协调负责共享导出、旧投影的窄提取、新 feature 常量、旧执行路径的明确拒绝和文档/catalog。不存在并行写同一共享文件的安排。

## 范围与检查

保持 `DocumentKind::Profile` 对旧 profile/1 的精确处理；新版本独立登记。新 feature 不因字符串已知就进入旧核心的无条件能力集合。源字节、清单、包与组件身份分别校验，生成位置声明不冒充已绑定产物。受检结果不得变成未经宿主检查的执行或验收令牌。

执行共享 Rust/Python 格式向量、独立摘要与旧 v1 向量、输入/责任正反例、依赖边界及完整本地门禁。集成前后重新绑定实际文件。无原生生命周期修改、设备占用、push/PR、远程 Actions 或发布；Lab 本批保持只读。

## 结果

新增两个闭合 schema、显式文档种类与独立摘要域，Rust/Python 共用 52 项新增结构向量。旧 profile schema、夹具和原格式/规范化向量逐字节保持；旧 profile 摘要回归及旧扩展入口拒绝新格式的测试通过。新测试沿用已有 `InputInvalid` 包装，并检查内层 `CapabilityUnsupported`，没有修改生产诊断来迁就测试。具体文件与命令见[格式及旧入口复核](chief-architect-fourth-batch-format-review.json)。

纯 composition 适配器绑定项目、profile、源锁、包清单、七份实际解码 schema 和冻结字节，校验宿主固定选择、组件/生成位置、来源别名和责任集合。生成文件只登记未来位置；返回值为只读观察。15 组 Rust 检查和 Clippy 通过，见[最终模块检查](chief-architect-fourth-batch-composition-final-checks.json)。

独立复核使用单独构造的输入和 Python 摘要投影：27 项输入/责任检查（3 项接受、24 项准确拒绝）及 11 项摘要对照通过。复核发现同一 profile 同时在 profiles/inputs 固定时被重复登记路径的误拒；修复后先比对实际字节，再合并同一路径登记，生成输出的同名、大小写和目录冲突仍拒绝。旧失败和初次模块检查保留，最终结论见[独立复核](chief-architect-fourth-batch-independent-review.json)。

完整 `scripts/check.py` 本地门禁通过，包含工作区、进程会话、路线适配、SDK 边界、架构与文档检查。该次 Rust 检查针对首次冻结字节；其后仅上述修复涉及 pins.rs 与对应测试，已对最终字节重新运行全工作区 fmt/check/Clippy/all-features tests。未变路线与 SDK 的证据仍绑定其原检查字节，不把旧运行记录标为修复后的重新执行。最终文档、集成清单与实际根复验结果见[验证记录](chief-architect-fourth-batch-validation.json)。

HTML 从 catalog 重新生成。全部 41 个工作包的 status、verification、阶段、依赖和 acceptance 保持；E01/E03 只追加本批范围与证据，C06/C07 完整任务对象不变。本批没有设备候选、独立 Lab 消费、远程 Actions、push/PR 或发布结果。

## 剩余交付与下一段

E01/E03 仍须把真实声明、SDK 导出及接线转换成宿主固定配方和独立必需责任集合，完成执行镜像准入、包依赖闭包与生成后 OutputSet 字节/血缘绑定，再复用 E02 会话和现有组合事务。当前纯比较接口不能用申请者自己的数组证明完整性；SDK 不透明内容、任意跨包 re-export、真实运行时及环境授权均未由本片认证。

下一段先贯通有限普通/增强/禁用组合的真实配方及生成结果，按接口推进 I01 和 L04/L05，再由独立 Lab 消费候选。I01 的具体公共格式草案仍待其自身决定；L02/L03、工程 I 链、C08/L06 及 C09/C10 联合验收保留现行目录的完整退出条件。本批没有新增整包验收通过。
