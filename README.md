# MadB

跨语言 SDK 工程工具。当前已建立工程基线、C02 契约/格式库和 C03 首链探针；C04 已接入只读契约查询和静态任务规划，尚无已认证的跨语言路线。

## 开发环境

使用 Python 3.14.4、Node.js 26.8.2、Rust 1.99.1 和一个 C11 编译器（clang 或 cc，用于 ABI 声明检查）。Python/Node 版本分别记录在 .python-version/.node-version，Rust 由 rust-toolchain.toml 固定；不需要修改全局 Rust 默认版本。

在仓库根执行：

```sh
python3 -m venv .venv
.venv/bin/python -m pip install --require-hashes --only-binary=:all: -r requirements-dev.lock
.venv/bin/python scripts/check.py
```

CI 使用同一入口。依赖缺失、工具版本不匹配和任一必需检查失败都会退出非零；文档检查不再引用本机临时目录。

```sh
cargo run --locked -p madb-cli -- --help
cargo run --locked -p madb-cli -- --version
```

`init`、`generate`、`build`、`verify` 尚未交付，调用会失败；功能按现有工作包逐步加入。

## 当前可运行的只读查询

在 macOS/Linux 上，读取已有的 C02 格式文件（路径均相对 `--root`）：

```sh
cargo run --locked -p madb-cli -- inspect --root /path/to/workspace --project madb.toml --request read.json --snapshot contract.json
cargo run --locked -p madb-cli -- check --root /path/to/workspace --project madb.toml --request read.json --snapshot contract.json
cargo run --locked -p madb-cli -- plan --root /path/to/workspace --project madb.toml --request read.json --snapshot contract.json --task-file metadata.json --task-file compile.json
```

项目配置可用 TOML 或 JSON；request 为 `madb.read-request/1`，其引用固定 lock/profile 的路径、大小和字节摘要。`plan` 的任务文件使用既有 `madb.build-task/1`，可重复 `--task-file` 并用 `--task <id>` 选择依赖子图；它不运行任务。没有任务文件时返回准备要求，不能把空计划报为成功。

成功只表示记录的契约校验及**已声明输入字节匹配**，输出 `TrackedInputsMatch`；尚不证明编译器依赖闭包完整、工具已安装或 SDK 可用。缺失/过期快照给出 `MetadataRequired`，没有隐式编译或缓存修复。完整 metadata 执行、`generate --check`、三层输出事务和缓存仍属于 C04 后续工作。复现与限制见[只读查询验证](tests/integration/c04/README.md)。

## 工程与依据

六个核心 crate 保持既定依赖方向。C02 在 model/rules/protocol 实现身份、策略、格式和证据检查，外层 `adapters/contracts` 显式转换并检查跨文档一致性。C04 的 app/plan 已提供只读用例和任务 DAG，host 负责有界读取，CLI bootstrap 装配适配器；进程执行、事务和缓存仍待完成。

格式入口见[契约格式 v1](docs/schemas/README.md)和[首链声明设计](docs/examples/first-route/README.md)。共同格式向量由 Rust/Python 验证，摘要向量由 Rust/Python/Node 验证；这些检查不运行原生 SDK。

- [工程骨架与依赖](docs/architecture/02-modules-and-ports.md)
- [任务目录](docs/catalog/implementation-plan.json)与[实施计划](docs/delivery/01-implementation-plan.md)
- [Agent 入口](AGENTS.md)与[贡献方法](CONTRIBUTING.md)
- [当前设计文档](docs/README.md)

MadB-Lab 是独立仓库，使用公开候选和独立预期；不能链接本仓库私有 crate。C03 已提供固定工具、实际生成及同步 FFI 双向运行探针，见[复现入口](tests/integration/c03/README.md)和[验收记录](docs/reviews/2026-10-24-c03-native-probes.md)。资源/并发、设备和公开 SDK 验收仍由后续任务提供。

## Quick start for contributors

Install the exact versions above, create the Python virtual environment, install the hash-locked development dependencies, then run `scripts/check.py` with that environment's Python. The CLI supports help/version and explicit read-only snapshot queries (`inspect`, `check`, `plan`); see the query guide below. Read the task's linked specification before changing code.

See [LICENSE](LICENSE) and [third-party scope](THIRD_PARTY.md). This repository does not yet provide a production SDK.
