# MadB

跨语言 SDK 工程工具。当前已建立工程基线及 C02 契约/格式库；CLI 仍只提供帮助和版本，尚无已认证的跨语言路线。

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

## 工程与依据

六个核心 crate 保持既定依赖方向。C02 在 model/rules/protocol 实现身份、策略、格式和证据检查，外层 `adapters/contracts` 显式转换并检查跨文档一致性。app/plan 的实际用例和执行器由 C04 完成；不会用空后端报告成功。

格式入口见[契约格式 v1](docs/schemas/README.md)和[首链声明设计](docs/examples/first-route/README.md)。共同格式向量由 Rust/Python 验证，摘要向量由 Rust/Python/Node 验证；这些检查不运行原生 SDK。

- [工程骨架与依赖](docs/architecture/02-modules-and-ports.md)
- [任务目录](docs/catalog/implementation-plan.json)与[实施计划](docs/delivery/01-implementation-plan.md)
- [Agent 入口](AGENTS.md)与[贡献方法](CONTRIBUTING.md)
- [当前设计文档](docs/README.md)

MadB-Lab 是独立仓库，使用公开候选和独立预期；不能链接本仓库私有 crate。当前检查证明工程结构、契约规则和声明形状，FFI、设备和 SDK 验收由后续任务提供。

## Quick start for contributors

Install the exact versions above, create the Python virtual environment, install the hash-locked development dependencies, then run `scripts/check.py` with that environment's Python. The CLI currently supports `--help` and `--version` only. Read the task's linked specification before changing code.

See [LICENSE](LICENSE) and [third-party scope](THIRD_PARTY.md). This repository does not yet provide a production SDK.
