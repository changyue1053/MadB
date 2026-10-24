# C02 共同格式与摘要向量

这些文件是**合成的检查器测试输入**，不是发布候选、真实工具锁或 SDK 运行证据。`fixture-1`、`fixture://toolchain`、全零 Lab revision 和若干 `fixture` 字节摘要都明确用于测试。不可拿本目录作为 `build --frozen` 的真实工作目录。

- 15 份 payload 正例覆盖 project/IR/lock/profile/case/coverage/report 及五个端口。
- `format-vectors.json` 定义共享正反变更；Rust `SchemaRegistry` 与 Python jsonschema 读取同一份 schema 和向量。
- `parse-vectors.json` 保留原始文本，以测试重复键、精度、Unicode 与尾随值，避免先解析后丢失问题。
- `canonical-vectors.json` 固定 JCS 字节及域隔离 SHA-256，覆盖 UTF-16 键序、Unicode 不归一、数组顺序、精确数值和安全整数边界；Rust、Python、Node 分别验证。
- IR/profile/project 与摘要由独立 Python/hashlib 参考过程按[投影规范](../../../docs/schemas/README.md)制作并人工审查后保存；测试不会调用产品函数重写期望值。修改规范时显式审查向量差异，不允许随实现失败自动刷新。

[适配器测试](../../../adapters/contracts/tests/)还验证来源/策略冲突、释放签名、异步借用、布局身份、锁、文件字节、五组端口配对、六种执行状态和证据失效。真实 48 项问题与 96 个基础组只被实例化以验证覆盖完整性，不生成任何执行通过记录或修改问题目录进度。

运行入口为仓库根 `scripts/check.py`。编译型 ABI 设计见[首链资产](../../../docs/examples/first-route/README.md)，其真实生成、链接和双向运行由 C03/C05 完成。
