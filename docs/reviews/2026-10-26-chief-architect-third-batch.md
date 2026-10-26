# 接管第三批：声明诊断与扩展准入

记录日期：2026-10-26。本文记录本批实施范围与实际检查；当前状态只在任务目录维护。依据为本次完整捕获的根 AGENTS、共同开发方法、模块边界、质量门禁，以及各任务在目录中引用的现行规范。第二批与 C06/C07 历史报告保持原结论。

## 输入、隔离与分工

MadB 根为 `/Users/changyue/Desktop/MadB/madb`，HEAD 为 `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`；独立 Lab 根为 `/Users/changyue/Desktop/MadB/madb_lab`，HEAD 为 `a466501275bee1786d171199390f228f55eb38a0`，分支均为 `codex/c07-arkts`。旧 detached 工作树不参与本批。

本批完整捕获产品 1017、Lab 450 个 tracked/非忽略 untracked 源文件，复制前后核对文件集合、SHA-256、HEAD、分支和 Git 状态。工作副本为 `/private/tmp/madb-chief-round3/{madb,madb_lab}`，原根、基线与旧报告保留；集成前重新核对捕获摘要，不以 HEAD 克隆替代未提交输入。

主协调入口负责共享导出、公共契约、检查入口、catalog 和生成视图。E01 线只负责既有 v1 清单/锁准入；L02 线负责声明来源与诊断复用；I01 线负责现有纯 ProjectPlan 的准确拒绝；L04/L05 线先核对第二业务声明生成的最小接入。各线在确认文件范围后修改工作副本，独立复核集成差异。Lab 本批先只读，设备未分配。

## 实施边界与检查条件

- E01/E03：保持既有控制协议及旧 profile/1 摘要；新组合格式先形成字段、来源、投影、责任与拒绝向量的具体方案。未采用的字段不进入生产解码器。已有 v1 可以表达的精确来源准入继续实现并测试。
- L02：复用现有声明和诊断，不改变运行期资源、线程或关闭保护；新 explain 机器输出字段另走格式决定。首片不等于 L02 整包。
- I01：针对现有纯计划的实际缺口补正反例，不将模板/InitResult 草案当作已采用格式，不新增覆盖更新入口。
- L04/L05：以现有受支持声明的实际生成、来源定位和普通消费为检查对象；不以固定 Snapshot 包装或结构解析宣称第二业务端到端完成。

源码检查按变更运行模块测试、生成/消费检查及集成门禁；涉及生命周期的新增变更须独立复核。文档修改运行静态检查与视图重生成。缺失环境、未执行平台、未完成公共格式明确保留。无 push、PR、远程 Actions 或发布。

## 结果

### E01 既有格式的精确准入

新增 `adapters/contracts/src/extension_lock.rs`，复用现有 Project/Profile/Lock、清单注册、FileRef 和 ProcessPolicy。该窄入口核对一个明确的 component=image=tool 映射：清单原字节、实际执行镜像、组件版本与完整工具元组、项目选择、host/target/source、参数和环境申请。现有策略没有网络授权字段，网络请求在此入口准确拒绝。没有新增 wire、修改旧注册入口或给予执行/最终提交权限。

新增 7 项测试与 registration/task_binding/execution_binding 的 47 项回归通过，clippy/rustfmt 通过。[独立审查](chief-architect-third-batch-e01-review.json)另验证 16 组拒绝/身份场景。单镜像映射不是包来源认证，也不证明全部依赖锁、协议版本协商或路由适配。多组件包源与实际执行镜像的不同身份仍须由新组合格式明确处理。

### L02 声明来源与诊断

新增只读 `declaration_diagnostics.py`，复用现有 `ARKTS_DECL_UNSUPPORTED_MODIFIERS` 与 `ARKTS_DECL_CALLBACK_RESULT`。解析器真实失败经来源路径、大小、SHA-256 和 UTF-16 位置核对后，驱动仅附加人类可读提示；原异常、错误文本、机器输出及退出语义保持。未知码、未编码失败或来源不符不伪造解释。新 helper 进入实际生成输入清单。覆盖说明见[声明诊断](../examples/language-api/l02-diagnostics.md)。

独立检查发现首个 UTF-8 BOM 被 reader 的 TextDecoder 去除，而提示层最初仍按原文计算，导致第一行位置偏移。修复保持 reader 行为：逻辑 UTF-16 偏移对应去单个首 BOM 后的编译器输入，字节偏移对应含 BOM 的冻结原始源。旧失败保留；修复后真实 reader/driver 10 项、独立位置 30 组及来源不符拒绝 90 项通过；既有 ArkTS adapter 82 项回归通过。详见[复核记录](chief-architect-third-batch-l02-review.json)。

这不是完整 `madb explain` 命令或新机器结果格式，也不补造原始模板范围、符号级来源或运行期覆盖。资源、线程和关闭保护未改。L02 仅开始交付本子范围，整包仍缺首链资源/回调贯通、责任表和正式 explain 入口。

### L05 第二业务的声明投影前端

新增独立纯值读取入口和 `value_declarations.py`，复用已有 AST 读取；默认 C07 完整 profile 的拒绝门禁保留。受支持输入为必需 number/string 字段的记录，以及使用这些值的同步自由函数；资源、异步、回调和导入等超出首片的语义准确拒绝。生成实际作者实现的导出面、声明视图和来源映射，没有固定业务名称表，也没有猜测数值 ABI。

以第二业务的原始、加方法、加字段、改名、删除五轮真实源码，完成 5 份 Release HAR 和 5 个普通 HAP 编译；第六个消费者使用已删除导出时，真实 SDK 给出对应源码位置的 missing-export 失败。生成声明的类型检查、实际 HAR 导出声明与独立期望对账通过。[最终独立复核](chief-architect-third-batch-l05-review.json)核对 25 个输入、88 个唯一附件，另比较旧/新默认 reader 的 30 种输入/模式组合；实际 HAP 含 ABC、无 native 库。

上述是正式 SDK 上的生成前端与编译证据，未安装或运行 HAP，未生成跨语言 ContractIR/数值与所有权策略，未完成 C04 Source/Candidate 提交或独立 Lab 验收。不能由此将“第二业务跨语言完整生成”、L05 或任意 Native 路线登记通过。原有 L04 Swift 首片保持；后续仍需对应后端的真实业务接线。

### I01 与公共格式审查

现有纯 ProjectPlan 未找到可证明缺陷，不为本批强加新更新模型；其 TaskGraph 4 项、ProjectPlan 9 项通过。[I01 独立复核](2026-10-26-project-init-format-review.md)要求下一版草案修正请求摘要失败分类、成功必须真实创建、FileRef 基准和显式 profile 的默认目标选择。原草案保留，尚未成为采用格式。

E01/E03 的[具体格式提案](composition-decision.md)及[独立复核](composition-review.md)完成修订：49 项设计检查、7 项独立反例和现行 Rust source_digest 核验通过。旧设计校验器的漏拒绝、订阅/Runtime 责任混淆和包内路径到锁命名空间的缺口均已修正，原版本保留。该结果只支持提交格式决定，未将新 schema/feature 写入生产，也不计 E01/E03 验收。

## 集成检查

集成前后均核对完整源集合与摘要，仅复制本批差异。完整产品门禁第一次被沙箱阻止既有 Unix socket 测试；原日志保留，使用所需权限重跑同一入口，不改断言。所需权限下完整 scripts/check.py 通过；最终命令、日志摘要与未执行项见[验证记录](chief-architect-third-batch-validation.json)。
