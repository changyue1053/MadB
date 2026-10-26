# L02 声明来源与诊断解释首片

本页描述 ArkTS 既有声明读取器的内部解释接线，适用输入仍为 [L01 ArkTS 契约](arkts-contract.md)及 C07 当前受限 profile。任务状态只见 [catalog](../../catalog/implementation-plan.json)；本首片不构成 L02 整包验收。

## 输入与责任

`adapters/generators/arkts/read_facade.cjs` 使用锁定 TypeScript 编译器 API 读取真实声明，输出既有私有 descriptor；其 `source` 保留实际输入的逻辑路径、UTF-8 字节摘要及大小。各声明位置包含一基行列、UTF-16 offset 和范围。`lowering.lower_public` 将实际函数/方法的 path、line、column 和 `compiler-derived` 精度写入既有 ContractIR，不另创声明、符号身份或来源格式。

`declaration_diagnostics.py` 只解释下表已有的两种稳定代码，输入为调用方已捕获的源字节、对应 FileRef 和该次读取器的原始错误。它不读文件、不执行工具、不推导所有权，不把泛化类型失败临时编号成稳定代码。内部 Python 返回值不是 CLI JSON、公共 wire 或可序列化协议；`explain(code)` 对未知码抛出 `UnknownDiagnosticCode`，无码错误和不匹配消息为 `UnsupportedReaderDiagnostic`，来源不符为 `SourceBindingError`。

| 既有码 | 当前真实触发条件 | 修复与普通替代 |
|---|---|---|
| `ARKTS_DECL_UNSUPPORTED_MODIFIERS` | 当前 AST 节点的修饰符集合与 source/declaration 模式在该节点允许的集合不符；不只指 async | 在作者输入中采用该节点支持的修饰符，不编辑生成输出；仍使用普通声明方法。异步工作保留明确 Promise/Operation 契约，不能仅删去 async 便声称效果等价或具备取消 |
| `ARKTS_DECL_CALLBACK_RESULT` | 已成功解析的回调类型返回值不是同步 void，包括可解析的 Promise；更早的未知类型失败不保证产生此码 | 将作者声明及实现改为同步 void listener，异步工作移出本同步交付契约；沿普通 Subscription 的 stop/close 管理观察，返回 Promise 不会让回调交付自动等待 |

实际 `driver._read` 在 `support.command` 读取公共声明失败时，使用发送给读取器的同一份字节和冻结引用尝试解释，仅对匹配结果追加人类可读 note。原 RuntimeError 对象、原文字、失败阶段和退出结果保留，顶层 CLI 将 note 写到 stderr；没有更改成功 stdout、ContractIR、生成输出或机器字段。超时、未知码、无码解析失败、改变的错误文案及来源不符不添加解释，也不吞掉原失败。

`adapters/generators/arkts/diagnostics.py` 仍负责 C07 测试伴随协议的权威字段投影和预算核对；它不是声明码词典。关闭运行诊断仍必须保留资源、线程、代次、准入和状态机保护，本首片没有为这些保护增加开关。

## 来源精度与信任边界

解释器先核对逻辑路径、源大小及 SHA-256，再按编译器的 LF、CRLF、CR、U+2028/U+2029 换行和 UTF-16 列计算位置。读取器的 TextDecoder 默认消耗一个首部 UTF-8 BOM，因此内部 `utf16_offset` 与 AST 的 `start` 一样，指去除该 BOM 后的逻辑文本；`byte_offset` 始终指向原始捕获字节，包含首 BOM 的三个字节。双 BOM 只去除第一个，后续 BOM 仍参与列与偏移计算。拒绝源外位置、落在代理对内部的列、错误摘要及错误源名。错误 stderr 只有点位置，不能从它捏造结束范围、符号 ID 或原模板映射。生成时替换字面值的编译输入和作者模板是不同字节；本首片定位前者，不承诺两者列号相同。

这些核对绑定调用方给出的读取结果和源字节，不独立认证任意外来 stderr 确实由编译器产生。正式驱动继续负责工具、冻结输入、预算与调用来源；helper 不启动第二次解析来“证明”诊断。成功 descriptor 的完整来源范围仍由实际 reader 提供；解释器只复用点位置，未宣称验证所有 descriptor 字段。

新增 helper 进入生产驱动的冻结 `HELPERS` 闭包，组件及后续候选身份随代码变化。既有 C06/C07 或 C09 候选报告继续绑定原身份，不自动覆盖这次接线。

## 检查入口与证据层级

普通宿主门禁不依赖本机 SDK 路径，也不跳过工具检查：

```sh
.venv/bin/python -B -m unittest discover -s tests/integration/l02_diagnostics -v
```

这组验证纯绑定、未知与无码分类、只读/无进程副作用，以及真实 CLI 的失败展示分支；展示分支注入失败，不能代替真实编译器检查。

真实 reader/driver 检查单独显式提供既有精确工具观察文件和一个尚不存在的报告文件。该文件沿用既有工具观察中的 `config.tools.node` 与 `config.tools.typescript`，不下载工具、不读取全局 PATH；两者执行前后按记录核对字节，不把任意新装工具的临时摘要当成供应链认证。

```sh
.venv/bin/python -B tests/integration/l02_diagnostics/test_declaration_diagnostics.py \
  --tool-observation "$MADB_L02_TOOL_OBSERVATION" \
  --report "$MADB_L02_REPORT"
```

显式命令运行普通门禁，并调用实际 reader 检查当前普通声明、三个已有负例、非 BMP、首部单/双 BOM 与六种换行输入、第二业务拒绝；将成功 descriptor 经过实际 `lower_public` 后核对来源。驱动失败测试在冻结引用入口注入受控记录，实际执行原 `support.command` 和 reader，检查原异常身份及 note；来源错配负例故意绕过上游引用验证，以验证解释层也不会误标。这不是完整冻结项目生成或候选消费验收。

报告绑定实际 helper/reader/driver/测试、策略及输入摘要、工具记录和运行结果。失败报告保留；修复后使用新报告路径重跑。无 SDK 生成、HAR/HAP 编译、native 运行、设备调用、独立 Lab 验收或发布含义。

## 与 L02 整包验收的差距

| catalog 条件 | 本首片提供 | 仍须交付 |
|---|---|---|
| 复用后端声明/来源，普通入口独立消费 | 当前 ArkTS 普通 descriptor 与既有 IR 点来源复用 | 其他后端覆盖、完整普通 SDK 独立消费与第二业务生成 |
| 诊断关闭仍保留资源/线程/状态机保护 | 未修改这些保护，解释器只在工具失败路径运行 | 诊断开关/失败 sink 与各实际候选资源后条件对照 |
| 首链资源/回调与生成诊断贯通，分别声明覆盖 | 当前两种声明拒绝到实际驱动 stderr；运行协议投影保持独立 | 完整首链语义诊断、运行 hooks 与适用静态属性覆盖 |
| DEC-07 每符号生成语义阶段责任表 | reader 担任结构/来源权威，lowering 担任既有 IR 映射，解释器只附说明；普通/便利共享相同声明机制 | 由 resolved profile/lowering 实际产生并检查完整每符号/阶段/实例责任表，不能用本页静态分工替代 |
| explain 稳定解释、未知失败、机器输出/退出正反例、只读 | 两个既有码的内部说明/条件/来源/可执行修复指导，未知码明确拒绝，查询无探测与构建 | 正式 `explain` CLI、经格式审查的机器输出、命令级正反例与所承诺代码覆盖；人类 stderr note 不替代该入口 |

后续沿 [已批准职责](../../reviews/2026-10-25-chief-architect-first-batch.md#目标-cli-的归属与可见行为)接入正式 explain；新增公共字段或兼容语义仍按格式流程处理。不得把本页两个代码的实现扩大成所有语言、所有声明或 L02 整包已通过。
