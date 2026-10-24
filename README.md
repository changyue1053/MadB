# MadB

跨语言 SDK 工程工具。当前处于 C01 工程基线阶段，CLI 仅提供帮助和版本；没有已经实现或认证的跨语言路线。

## 开发环境

使用 Python 3.14.4、Node.js 26.8.2 和 Rust 1.99.1。Python/Node 版本分别记录在 .python-version/.node-version，Rust 由 rust-toolchain.toml 固定；不需要修改全局 Rust 默认版本。

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

六个核心 crate 是已建立的构建边界。madb-cli 有上述入口；其余五个 crate 暂无公开 API，具体模型/协议属于 C02、应用与规则属于 C04。没有创建成功返回的空后端。

- [工程骨架与依赖](docs/architecture/02-modules-and-ports.md)
- [任务目录](docs/catalog/implementation-plan.json)与[实施计划](docs/delivery/01-implementation-plan.md)
- [Agent 入口](AGENTS.md)与[贡献方法](CONTRIBUTING.md)
- [当前设计文档](docs/README.md)

MadB-Lab 是独立仓库，使用公开候选和独立预期；不能链接本仓库私有 crate。C01 的检查只证明工程基线，FFI、设备和 SDK 验收由后续任务提供。

## Quick start for contributors

Install the exact versions above, create the Python virtual environment, install the hash-locked development dependencies, then run `scripts/check.py` with that environment's Python. The CLI currently supports `--help` and `--version` only. Read the task's linked specification before changing code.

See [LICENSE](LICENSE) and [third-party scope](THIRD_PARTY.md). This repository does not yet provide a production SDK.
