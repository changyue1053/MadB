# C02 契约、配置、锁与证据格式验收

日期：2026-10-24。设计依据：基线 0.9、DEC-01/02/03、C02 六项 acceptance。两份 C01 PR 已按用户授权合入：MadB `4057552d930a2741e298efc0551469faa9a44647`，MadB-Lab `6721ce40ef226cf2466faf0d00681b7bc074600b`。

本次实现 C02 库与首链可执行设计，不开发 C03 的实际工具链探针、C04 的 CLI/任务执行器或 C05 的 FFI 运行时。MadB-Lab 的独立基线没有修改。当前本地门禁通过，远端 CI 结果由本记录后续的提交说明及任务目录关联；在远端检查通过前不标记整项验收完成。

## 实施与验收对应

| C02 验收 | 本次交付 | 证据入口 |
|---|---|---|
| 权威、稳定身份、完整选择器与默认展开 | 稳定 SourceKey、显式/派生 public ID、来源与冲突规则；无重载语言的签名不混入 source ID | `madb-model`、`madb-rules`；适配器 semantics 测试 |
| 配置及业务 payload 完整字段 | 15 类 schema 和 common 定义、严格解析、共同正反向量；保留原扩展控制帧分工 | [格式说明](../schemas/README.md)、Rust/Python 格式测试 |
| 摘要与证据身份跨实现一致 | JCS 受限数值编码、域隔离、语义投影和原始文件摘要；位置/顺序/工具失效反例 | 7 组固定向量经 Rust/Python/Node 验证；identities 测试 |
| 首链 ABI 与来源诊断 | 固定状态值、`repr(C)`/C 声明、句柄、关闭、观察 context、双方向入口与诊断位置 | [首链 ABI](../examples/first-route/README.md)、C11/Rust metadata 编译、SEM008/POL004 来源断言 |
| 48 问题按组合实例化并分离状态 | 适用性要求完整性；六种执行状态与 missing/current/stale 独立；独立预期、清理及资源守恒 | 真实 48 问题/96 基础组的实例化测试、evidence_and_ports 测试；不产生路线通过记录 |
| domain/app/wire 分工 | 六个核心边界保持；增加纯格式适配器 `madb-contracts`，schema 到领域值显式转换 | Cargo 依赖门禁及 7 个正反例；app 用例仍归 C04 |

## 全部修改范围

- `crates/madb-model`：受检身份、结构/策略、案例与证据领域值。
- `crates/madb-rules`：身份、精确策略、类型/数值/资源/取消必要条件、布局观察、覆盖完整性与证据规则。
- `crates/madb-protocol`：预算与重复键检查、离线 schema、JSON/TOML 解码、JCS/SHA-256。
- `adapters/contracts`：明确转换、语义投影、校验编排、冻结锁关系、实际文件字节和五组端口配对检查。
- `docs/schemas`：16 个新 schema，共用字段统一引用；补齐格式、投影、诊断及支持限制。
- `tests/fixtures/contracts`、`tests/contract`、相应 Rust integration tests：合成正反样本、跨实现摘要和 ABI 声明编译。
- `docs/examples/first-route` 和 `madb.example.toml`：可编译预期声明及通过当前 schema 的配置写法。
- `Cargo.toml/Cargo.lock`、子包 manifest、`THIRD_PARTY.md`、[依赖清单](c02-dependency-licenses.json)：冻结必要依赖并记录许可证元数据；没有复制上游生成器代码。
- `tests/architecture`、`scripts/check.py`：把 C02 接入原检查入口，阻断核心导入适配器以及适配器导入 CLI；binary-only 依赖声明同样受检。
- README、模块/契约/生命周期规范、首链准入、任务目录与 13 份 HTML：同步真实实施边界与进度。历史 C01 审查原文保留。

## 实际验证

本机为 macOS Arm64，Rust 1.99.1、Python 3.14.4、Node 26.8.2；ABI 声明编译使用 Apple clang 21.0.0（clang-2100.3.34.2）。这不是 C03 的可复现 SDK 工具锁。

执行 `.venv/bin/python scripts/check.py` 通过：

- fmt、workspace check、全目标/全 feature Clippy（warnings 为错误）。
- **37 个 Rust 测试**：包含既有 3 个 CLI 测试；新增身份/策略/摘要/锁/文件/证据/格式反例。
- **5 个 Python 测试**：69 组共享格式正反向量、解析负例、摘要向量和 C/Rust ABI 声明编译。
- **7 个依赖架构测试**、**21 个文档测试**。
- **7 组 Node 摘要向量**和既有全景图 DOM 单元检查。
- 文档链接/schema/catalog 与 13 个 HTML 的生成一致性检查；`git diff --check` 通过。

重复键、整数舍入、非法路径、未知格式主版本、冲突权威、错 profile、异步 call-frame 借用、错误释放器、布局/工具/源不匹配、旧证据、假运行成功、缺输出、错误负例和清理计数矛盾都进入负例。当前没有跳过的必需检查。

## 准确边界与下一步

测试中的 profile/文件引用是合成样本，不是可消费 SDK；本机声明编译不是实际绑定生成、链接或双向运行。Python 参考实现核对共同结构向量，RFC 3339 format 校验保留在 Rust 主解码器。没有重新执行浏览器视觉验收，DOM-shaped 单元检查不冒充浏览器。

C03 应继续 P01/P02：真实 Rust/Clang/libclang/bindgen/cbindgen/SDK 锁、macOS Arm64 与 x86_64 布局/编译、至少一个目标的独立 C→Rust 与 Rust→C 运行。C04 接入纯应用请求、任务图、文件隔离、真实端口与缓存；C05/C09 再验证生成结果、所有权、并发关闭、候选包及独立 Lab。C10 发布编译产物前补齐实际分发依赖的许可证文本；本次依赖元数据清单不代替 NOTICE 包。
