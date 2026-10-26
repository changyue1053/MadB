# ADR-0020 只读 explain 查询及来源核对

状态：项目所有者已确认“采用推荐契约，进入实现”。日期：2026-10-26（America/New_York）。关联 L02；不改变阶段、依赖或整包验收条件。

采用[具体提案](../reviews/l02-explain-proposal.md)第2–8节的 CLI、两种闭合格式、有限说明目录与拒绝边界；独立复核见[审查](../reviews/l02-explain-independent-review.md)。批准版本 SHA-256 为 `6bc0f329a0e3c7e0c537b235ede7efede61bf58366c414cce95e97b6e07bf821`。提案与审查保留批准前的设计结论，设计片段检查不计实现或验收通过。

- 新增 `madb explain CODE [--context PATH [--root DIRECTORY]] [--json]`。纯码查询不读取工作区；未知码在读取 context 前明确失败。既有六命令保持原行为，不启动工具、网络、设备或写入。
- 新增 `madb.explain-result/1` 与 `madb.diagnostic-context/1`。复用既有诊断、File/FileRef/Location 原子，不伪造 Report 的候选和运行事实，不新增摘要域。
- 可选 context 只核对显式源字节、摘要及 byte-lf/utf16-js 点位置；精度是输入声明，不认证编译器确实产生该诊断。BOM、UTF-16、源变动和缺失来源分别处理，不制造位置。
- 结果退出0只表示本次解释成功，2为非法输入或未知码，3为真实宿主 I/O 失败，4为已捕获输入在本次复核时变动。JSON 正常结果只写 stdout；输出失败不能声称已交付成功对象。
- 说明目录覆盖提案列明的47个现有码及4个 EXPLAIN 查询码，每码的实际产生分支分别核对。实施核对发现提案简表把 TYPE004 扩写为“重复常量”；实际只有符号参数、record 字段和 callback 参数的重复名称。以实际产生方定义纠正文案，不新增语义或支持能力。CAP003 的精度含义已在批准前独审修正。

实现仍须交付共享格式向量、真实 CLI/来源失败正反例、只读检查和独立审查。普通 SDK 生产诊断及 checked constructor 见 ADR-0021；两项决定不能替代 L02 的其他原有验收条件。实时任务状态只在 catalog 维护。
