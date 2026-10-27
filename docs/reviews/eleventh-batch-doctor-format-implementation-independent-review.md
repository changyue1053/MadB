# I02 doctor 格式实现独立审查

范围：隔离仓 `/private/tmp/madb-chief-round10/repo` 的 doctor-result schema、DocumentKind 注册、Rust/Python 共享格式向量及 doctor 格式测试。依据已批准提案 SHA-256 `60bdba6ebb41197a4d7623dcdfe4d22a150126621f746b195675cd641c96c17d` 和 `i01-doctor-independent-review.md`。本审只读产品，不运行 Cargo、不修改 schema/fixture；独立 Python 探针只写 round11。

结论：本次发现的格式与样例问题已修正，当前格式实现没有剩余阻断。该结论不涵盖 doctor app/host/CLI 行为，也不登记 I02 整包通过。

## 发现与修正实证

1. **禁止控制字符的正则曾放过末尾 LF。** 原 root、subject、host.version、host.build 使用 `$` 结束锚点。以正式项目 venv 的独立 JSON Schema 实现测试，四个末尾 LF 输入全部误接受。原结果保存在 `doctor-format-independent-probes-001.json`，不覆盖。现改为 `not + 控制字符 pattern`，root 另约束 `^/`；同四例及末尾 CR/NUL/DEL/TAB 均拒绝。没有放宽 schema，也没有修改 common 旧定义。
2. **原超预算样例没有所声称保留的 Conflict。** 原 `doctor-limit-preserves-conflict` 对应 exit 4、空 checks；只能测试宽泛形状，不能体现提案要求保留最高错误行。现模型含 tool.ninja Conflict 行、原首诊断、null expected/observed。独立复核通过。它仍是显式 design fixture，不能当成实际超预算汇总执行证据。
3. **格式向量入口与词法失败必须分开。** 原向量索引嵌入 1.5，Rust strict_json 在读取索引时先失败；现将 1.5 移到 doctor_formats 的 raw 字节拒绝测试，保留 FMT006。缺 schema 的共享向量现按既有 decoder 入口使用 FMT003；没有把 decoder 改成适应错误样例。早期 FileRef `encoding` 也已从错误 fixture 删除，common.file_ref 保持原定义。

## 独立验证

使用 `/Users/changyue/Desktop/MadB/madb/.venv/bin/python -B` 和独立 jsonschema/referencing 引擎，读取实际 schema 与 fixtures：

- 26 个定向输入均符合预期，包括四字段 × 五控制字符、合法中文空格根、未知字段/域、null 工具成功、truncated 成功拒绝。
- 对四份模型遍历全部对象，49 次添加未知字段均拒绝；256 次删除已有必需字段均拒绝。
- ToolObservation 七字段及限制与 report 完全相同；Diagnostic 同字段、同限制，nullable location 的 anyOf/oneOf 两分支互斥，接受集合一致。
- 新 schema SHA-256：`210f3fdd643e4d15be59d07ce78c7c7d846702bec3bbf7b9af7e01561c86d75c`。
- 独立结果 `doctor-format-independent-probes-002.json` SHA-256：`33b231e6b567926e036b314cb825634b8c9935611bf9907c7dc70ced61da0881`。

另实际读取主协调日志：`doctor-schema-python-003.log` 为 4 tests OK；`doctor-schema-rust-003.log` 为 doctor_formats 3/3、formats 6/6。它们是主协调执行证据，本审没有再次启动 Cargo。Python 001 的错误 FileRef fixture、Rust 001 的非法索引数值、Rust 002 的错误预期码均保留，不能把前三次失败改写为通过。

## 与批准契约的对应

顶层字段、显式 nullable、scope=preflight-only、0–4 退出范围、封闭 Fact union、独立 root、project File 与其 JSON/TOML 格式、其余 FileRef、Check 状态与 error_kind 对应、blocked 行空事实及非空 blocker、错误行诊断、成功必需 envelope、truncated 的 DOCTOR001 与非零约束均符合提案。没有新 input wire、extensions 逃生槽、report/candidate/Lab 伪身份或旧 schema 兼容变化。

四份数据均为设计模型，非实际工具、工程或资源观察；complete-model 的固定占位摘要与 design-only 根不能作为预检通过证据。带调整 status/error_kind 的正向向量只是字段组合测试，不证明其示例 overall exit 已由应用正确汇总。

应用层仍须独立证明：受信配置与等级派生完整必需行；ID 唯一与依赖顺序、blocked_by 引用实际前置失败且无环；真实 status/error_kind 与最高优先级汇总；snapshot/recipe/工具身份关系；错字节不伪造 tool fact；输出预算保留原错误且不返回 0；全程重验、只读、无子进程。上述语义不变量不要求硬塞进 JSON Schema；已向 app 实现者明确分责。

下一停止点：格式审查完成，待主协调安排 doctor 生产实现及冻结候选完整独审。
