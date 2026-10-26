# L02 声明来源、解释查询与首链诊断

本页描述 ArkTS 声明来源复用、ADR-0020 的只读解释命令和 ADR-0021 的 C→Rust 首链诊断。ArkTS 适用输入仍为 [L01 ArkTS 契约](arkts-contract.md)及 C07 受限 profile；新增 SDK 配置绑定自己的候选。任务状态只见 [catalog](../../catalog/implementation-plan.json)，历史首片证据不自动覆盖新实现。

## 输入与责任

`adapters/generators/arkts/read_facade.cjs` 使用锁定 TypeScript 编译器 API 读取真实声明，输出既有私有 descriptor；其 `source` 保留实际输入的逻辑路径、UTF-8 字节摘要及大小。各声明位置包含一基行列、UTF-16 offset 和范围。`lowering.lower_public` 将实际函数/方法的 path、line、column 和 `compiler-derived` 精度写入既有 ContractIR，不另创声明、符号身份或来源格式。

`declaration_diagnostics.py` 从 CLI 共用的固定说明目录提取下表两种稳定代码，输入为调用方已捕获的源字节、对应 FileRef 和该次读取器的原始错误。模块导入时只加载已冻结的固定说明目录；解释与来源绑定函数不读取作者源文件、不执行工具、不推导所有权，不把泛化类型失败临时编号成稳定代码。内部 Python 返回值不是 CLI JSON、公共 wire 或可序列化协议；`explain(code)` 对未知码抛出 `UnknownDiagnosticCode`，无码错误和不匹配消息为 `UnsupportedReaderDiagnostic`，来源不符为 `SourceBindingError`。

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

## 正式 explain 查询

[ADR-0020](../../adr/0020-explain-query-contracts.md) 固定命令及 [explain-result/1](../../schemas/explain-result.schema.json)、[diagnostic-context/1](../../schemas/diagnostic-context.schema.json)。例如：

```sh
madb explain ARKTS_DECL_CALLBACK_RESULT
madb explain TYPE004 --json
madb explain ARKTS_DECL_UNSUPPORTED_MODIFIERS --context diagnostic.json --root ./project --json
```

纯码查询从内置目录解释47个既有码及4个查询码，不读取工作区。未知码在打开 context 前失败；context 只能是显式逻辑文件路径，不接受 stdin。指定 context 后核对同次诊断码、实际源字节及位置，拒绝符号链接、路径别名、错误摘要和越界位置。来源精度仍是输入声明，不认证外来诊断出自哪个编译器。实际 reader 到 CLI 的验证入口为 [check_explain_cli.py](../../../tests/integration/l02_diagnostics/check_explain_cli.py)。

退出0表示查询完成；2表示未知码或非法输入，3表示实际宿主 I/O 失败，4表示捕获的输入在本次复核时变化。JSON 结果只写 stdout；无法写出结果时退出3。文本成功写 stdout，错误写 stderr。修复建议供调用者执行，命令本身不修改声明、不运行探测或构建。既有读取器的 stderr note 保持原失败身份；它与正式查询复用说明目录，仍是两个有明确边界的入口。

## C→Rust 普通构造与可选诊断

[ADR-0021](../../adr/0021-native-diagnostics-and-checked-constructor.md) 规定新生成候选的 `include/madb_rust_client.h`。消费者调用 `madb_engine_create_checked()` 即可核对该头文件的 interface/profile 身份，再调用原 create；无需手写64字节身份常量。结果区分 bootstrap 失败、接口错配、配置错配及 create 失败；失败不交付句柄，成功只取得一个 owned handle。后续仍显式执行原 request_close、await_closed、release_handle，helper 不建立另一套资源或订阅状态机。

默认库保持7个原生导出；测试伴随14个。显式 diagnostics 配置增加 configure/drain 两个入口，分别为9/16个导出，普通候选不传播诊断依赖。诊断使用每个 loaded image 的固定64槽，默认关闭；业务完成边界记录纯值，满载丢新，单次 try_lock 不等待，不调用外部日志 sink。消费者在 native 调用之外拉取最多一条 Event 和 Stats，再自行导出。关闭采集不关闭句柄、线程、SourceLease、准入或停止排水保护；诊断计数不替代资源守恒观察。

该首链的静态声明来自真实 cbindgen/Clang surface，lowering 仍用现有受检 IR 与策略。运行记录只描述已声明的调用/回调边界，不验证任意裸指针有效性、用户回调无 unwind 或消费者 exporter 的线程安全。默认 Release、诊断 Release、测试伴随及独立消费分别记录，编译和临时 mock 不当作实际候选运行。

## 逐符号责任及验证边界

固定首链从精确七/九项 ABI role、真实 IR、effective policies 与冻结 reader/SDK 来源派生责任。核心纯规则校验唯一且完整的 key/owner 集合；Python 生成 `responsibilities.md`，宿主按核心已核对的期望 FileRef 再核实际输出字节，随正常 Source 和候选文件清单交付。附表不新增共同 wire，不把其他 reader 或 SDK 误纳入本有限配方。

普通配置27行、诊断配置33行，区分读取、ABI lowering、同步投影和适用的订阅、投递、停止、排水、释放。checked constructor 仅有身份分支的 helper 责任，bootstrap/create 委派原入口；不存在第二次转换、订阅或资源 owner。source-caller 同步投递不声称固定 OS 线程、队列或线程池；不适用的 async/cancel 不伪造 owner。物理装配与运行语义仍由真实 Build/Candidate/Lab 证据证明。

现行验收保留声明/来源、独立普通消费、关闭诊断的保护、首链资源/回调、责任表和解释命令全部条件。具体实现、编译、运行、独审及尚缺环境见 [本批记录](../../reviews/2026-10-26-chief-architect-sixth-batch.md)；不能据本文将所有语言、平台、正式分发或 L02 整包记为完成。
