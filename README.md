# MadB

跨语言 SDK 工程工具。当前已建立工程基线、C02 契约/格式库和 C03 首链探针；C04 已接入只读契约查询、静态任务规划、受控进程、已声明输入副本、单任务结果验证和静态串行 DAG，尚无已认证的跨语言路线。

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

## 受控进程验证

`.venv/bin/python -m unittest discover -s tests/integration/c04_process` 调用真实 app/host 端口，验证取消准入、参数/环境、背压、超时、组终止与清理缺口。详见[复现说明](tests/integration/c04_process/README.md)。零退出只允许进入后续结果验证，不代表候选已提交或 SDK 已认证。

当前执行宿主支持 Linux/macOS；macOS 构建还需要 Xcode Command Line Tools 和可用的 libclang，供锁定的 libproc 构建脚本生成系统声明。调用方保持工具/工作目录稳定并独占子进程回收；进程超时不覆盖启动前后文件 IO 和 OS 启动本身。它不执行网络隔离或完整进程树沙箱。

## 固定输入验证

`.venv/bin/python -m unittest discover -s tests/integration/c04_inputs` 通过内部 app/host API 捕获已声明文件、创建独占 `inputs/work` 目录，让真实工具读取固定副本，并在执行前后检查副本。源文件在捕获后变化不会改变已捕获字节；修改副本或增加/移除路径会拒收结果。目录在成功、失败、取消和 Drop 后均保留，未自动发布或回收。复现见[固定输入回归](tests/integration/c04_inputs/README.md)。

这只覆盖声明清单，不证明完整依赖、原子文件系统快照或阻止同权限写入。是否允许旧快照结果提交仍由后续输入/CAS 协议判定。尚无用户可用的 build/generate 命令。

## 单任务配方验证

`.venv/bin/python -m unittest discover -s tests/integration/c04_tasks` 绑定受检 project/request/IR/profile/lock 和无依赖 BuildTask，启动实际受控配方，并核对 stdout 的 BuildEvidence 与 work 中的输出/日志字节。结果解析、捕获和最终副本检查均在一次性取消门禁内；被取消或验证失败不会暴露结果值。成功值保留独立字节，磁盘后来变化不改变它。详见[单任务回归](tests/integration/c04_tasks/README.md)。

配方通过 stdin/stdout 使用既有格式；普通编译器由配方适配。仅支持所选 profile 中精确锁定的工具 image，不把组件包摘要当可执行文件。单任务入口只接受无依赖配方；多任务由下面的图入口调度，尚无提交能力，成功只表示受检观察；不证明关联生成批次已存在或可发布。

## 静态串行 DAG

`.venv/bin/python -m unittest discover -s tests/integration/c04_dag` 通过内部图入口绑定同一配置上下文的多份 BuildTask，执行 producer→consumer、分叉/汇合或命名子图。首节点前冻结外部源；下游只消费直接依赖已验证的 outputs，不读取上游活跃 work，也不以同名源文件兜底。节点失败或图取消后不再调度，返回节点观察与保留目录，图结果集不可用。

本子集要求输入大小/摘要预先已知，最多执行 64 节点；根源、节点输入副本与结果文件合计最多 256 MiB。未知产物需要先发现身份、重新绑定下一张图。独立回归及接口见[图执行验证](tests/integration/c04_dag/README.md)。成功图仍是受检观察，没有批次提交、缓存或完整依赖证明；正式 build/generate CLI 尚未开放。

## 工程与依据

六个核心 crate 保持既定依赖方向。C02 在 model/rules/protocol 实现身份、策略、格式和证据检查，外层 `adapters/contracts` 显式转换并检查跨文档一致性。C04 的 app/plan 已提供只读用例和任务 DAG，app 捕获不可变输入值，host 负责有界读取、私有副本与受控进程，CLI bootstrap 装配查询适配器。进程底层通过内部 app 端口调用，尚未开放 build/generate；静态串行图可验证已知输入的依赖传递；动态元数据接入、事务和缓存仍待完成。

格式入口见[契约格式 v1](docs/schemas/README.md)和[首链声明设计](docs/examples/first-route/README.md)。共同格式向量由 Rust/Python 验证，摘要向量由 Rust/Python/Node 验证；这些检查不运行原生 SDK。

- [工程骨架与依赖](docs/architecture/02-modules-and-ports.md)
- [任务目录](docs/catalog/implementation-plan.json)与[实施计划](docs/delivery/01-implementation-plan.md)
- [Agent 入口](AGENTS.md)与[贡献方法](CONTRIBUTING.md)
- [当前设计文档](docs/README.md)

MadB-Lab 是独立仓库，使用公开候选和独立预期；不能链接本仓库私有 crate。C03 已提供固定工具、实际生成及同步 FFI 双向运行探针，见[复现入口](tests/integration/c03/README.md)和[验收记录](docs/reviews/2026-10-24-c03-native-probes.md)。资源/并发、设备和公开 SDK 验收仍由后续任务提供。

## Quick start for contributors

Install the exact versions above, create the Python virtual environment, install the hash-locked development dependencies, then run `scripts/check.py` with that environment's Python. The CLI supports help/version and explicit read-only snapshot queries (`inspect`, `check`, `plan`); see the query guide below. Read the task's linked specification before changing code.

See [LICENSE](LICENSE) and [third-party scope](THIRD_PARTY.md). This repository does not yet provide a production SDK.
