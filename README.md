# MadB

跨语言 SDK 工程工具。当前提供契约检查、受控执行、输入追踪及三层输出事务；实际工作包状态以[实施目录](docs/catalog/implementation-plan.json)为准，尚无已认证的跨语言路线。

## 开发环境

使用 Python 3.14.4、Node.js 26.8.2、Rust 1.99.1 和 Clang 工具链（用于 C11 声明检查、真实依赖发现及消费测试）。Python/Node 版本分别记录在 .python-version/.node-version，Rust 由 rust-toolchain.toml 固定；不需要修改全局 Rust 默认版本。

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

`init`、`verify` 尚未交付；当前 `generate/build/package` 必须提供显式锁定配方，不会为任意项目自动推导后端。

## 当前可运行的只读查询

在 macOS/Linux 上，读取已有的 C02 格式文件（路径均相对 `--root`）：

```sh
cargo run --locked -p madb-cli -- inspect --root /path/to/workspace --project madb.toml --request read.json --snapshot contract.json
cargo run --locked -p madb-cli -- check --root /path/to/workspace --project madb.toml --request read.json --snapshot contract.json
cargo run --locked -p madb-cli -- plan --root /path/to/workspace --project madb.toml --request read.json --snapshot contract.json --task-file metadata.json --task-file compile.json
```

项目配置可用 TOML 或 JSON；request 为 `madb.read-request/1`，其引用固定 lock/profile 的路径、大小和字节摘要。`plan` 的任务文件使用既有 `madb.build-task/1`，可重复 `--task-file` 并用 `--task <id>` 选择依赖子图；它不运行任务。没有任务文件时返回准备要求，不能把空计划报为成功。

成功只表示记录的契约校验及**已声明输入字节匹配**，输出 `TrackedInputsMatch`；尚不证明编译器依赖闭包完整、工具已安装或 SDK 可用。缺失/过期快照给出 `MetadataRequired`，没有隐式编译或缓存修复。显式 metadata 执行及生成/构建/打包见下方；查询命令不会自动执行这些步骤。复现与限制见[只读查询验证](tests/integration/c04/README.md)。

## 显式执行与三层输出

C04 提供受检执行基础设施。每个命令消费一份 `madb.execution/1` 文件，引用精确的配置、工具和请求；它不自动猜测后端或生成语言绑定。

```sh
madb build --root /path/to/workspace --task metadata --execution metadata.execution.json --frozen
# 按需显式准备受限原生依赖；工具可用 --tool-root 指向固定安装根
madb build --root /path/to/workspace --task dependencies --execution generate.execution.json --frozen
madb generate --root /path/to/workspace --execution generate.execution.json --frozen
madb generate --root /path/to/workspace --execution generate.execution.json --frozen --check
madb build --root /path/to/workspace --execution build.execution.json --frozen
madb package --root /path/to/workspace --execution package.execution.json --frozen
```

metadata 在缺少 IR 时执行真实读取配方，返回固定 IR 路径；生成执行 `GenerateRequest→OutputSet` 并提交源码批次；构建固定该批次，只消费受检依赖输出；打包只接受固定源码和构建批次中的真实证据。未知输出身份先计算、再显式绑定下一阶段，不能在执行中改写任务摘要。

构建子图缺少 compile/link/run 任一阶段时，只报告任务已执行，不提交完整构建批次。`package` 产生不可变候选，不能据此宣布 SDK 已认证。

输出以不可变物理批次存储。短提交锁内复核输入和索引代次；旧结果不能覆盖新 current，同身份复用仍校验实际文件。消费者持有固定批次和祖先租约；源码更新清空旧构建与候选的 current，旧消费者继续读取自己的旧批次。

`generate --check` 在系统临时目录真实复算，对比文件与登记身份；不写源码、锁、有效输出或依赖缓存。缺材料、漂移、损坏和工具失败均非零。运行目录按保留策略留下并报告路径：进程组收尾不能证明所有逃逸后代已停止，因此不会由 Drop 自动删除现场。

## 依赖与缓存边界

显式缺失路径、目录成员和内容观察用于发现变化；它们本身不证明完整依赖。受限 freestanding Clang 配置进一步执行真实传递依赖发现，固定源和搜索范围，记录工具、参数、宏、环境及目录/缺失条件，再形成可缓存身份。无法证明完整性的配置返回准备要求或禁用缓存；普通脚本生成器不能借一份 Clang 清单继承缓存资格。

已准备的输入清单必须与后续配方逐项一致，不静默扩充锁定请求。受限原生准备缓存逐项校验清单和文件；损坏、缺失和身份漂移保守重建。缓存不是输出提交许可，也不是测试或认证证据。

## 验证与支持范围

当前执行宿主为 Linux/macOS 本地文件系统。macOS 需要 Xcode Command Line Tools 和 libclang，供固定 libproc 依赖构建系统声明。工具、系统运行库与命名工作区根由调用方保持稳定；私有目录和进程组不构成同权限恶意进程、网络或完整进程树沙箱。

| 验证范围 | 入口 |
|---|---|
| 配方、取消、背压、超时和进程收尾 | [受控进程](tests/integration/c04_process/README.md) |
| 固定输入、结果字节和 DAG | [输入](tests/integration/c04_inputs/README.md)、[单任务](tests/integration/c04_tasks/README.md)、[执行图](tests/integration/c04_dag/README.md) |
| 缺失路径、成员集合和最终复核 | [输入观察](tests/integration/c04_observations/README.md) |
| 三层事务、CLI、故障恢复和缓存 | [整包验证](tests/integration/c04_package/README.md) |

完整检查仍使用 `scripts/check.py`。本地进程终止/恢复不等于断电、网络文件系统、所有目标设备或 SDK 生命周期验收。

## 工程与依据

六个核心 crate 保持既定依赖方向。app 拥有用例、端口、输入/结果与提交控制；contracts 转换 wire 数据并绑定语义；host 拥有进程、目录、缓存及原子存储；CLI bootstrap 只装配这些边界。规范、实际接口及验收分别维护，不以目录或 schema 存在推导能力。

格式入口见[契约格式 v1](docs/schemas/README.md)和[首链声明设计](docs/examples/first-route/README.md)。共同格式向量由 Rust/Python 验证，摘要向量由 Rust/Python/Node 验证；这些检查不运行原生 SDK。

- [工程骨架与依赖](docs/architecture/02-modules-and-ports.md)
- [任务目录](docs/catalog/implementation-plan.json)与[实施计划](docs/delivery/01-implementation-plan.md)
- [Agent 入口](AGENTS.md)与[贡献方法](CONTRIBUTING.md)
- [当前设计文档](docs/README.md)

MadB-Lab 是独立仓库，使用公开候选和独立预期；不能链接本仓库私有 crate。C03 已提供固定工具、实际生成及同步 FFI 双向运行探针，见[复现入口](tests/integration/c03/README.md)和[验收记录](docs/reviews/2026-10-24-c03-native-probes.md)。资源/并发、设备和公开 SDK 验收仍由后续任务提供。

## Quick start for contributors

Install the exact versions above, create the Python virtual environment, install the hash-locked development dependencies, then run `scripts/check.py` with that environment's Python. The CLI supports explicit read-only queries and frozen metadata/generation/build/package recipes. Execution infrastructure does not certify a language SDK; see the guides above. Read the task's linked specification before changing code.

See [LICENSE](LICENSE) and [third-party scope](THIRD_PARTY.md). This repository does not yet provide a production SDK.
