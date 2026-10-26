# MadB

跨语言 SDK 工程工具。当前提供契约检查、受控执行、输入追踪及三层输出事务；Rust ↔ C 首链已在 macOS Arm64、Linux x86_64 的实际配置通过独立 Lab 验收。C06 四个 Kotlin/Swift/Rust 配置也已按完整本地验证验收收口，GitHub Actions 留作项目完成后的独立专项。支持范围与证据见[实施目录](docs/catalog/implementation-plan.json)、[C05 记录](docs/reviews/2026-10-24-c05-rust-c-first-route.md)和[C06 记录](docs/reviews/2026-10-24-c06-kotlin-swift-rust.md)。

## 开发环境

使用 Python 3.14.4、Node.js 26.8.2、Rust 1.99.1 和 Clang 工具链（用于 C11 声明检查、真实依赖发现及消费测试；C07 宿主检查还要求 C17/C++17 与 ASan/UBSan 运行库）。Python/Node 版本分别记录在 .python-version/.node-version，Rust 由 rust-toolchain.toml 固定；不需要修改全局 Rust 默认版本。

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
cargo run --locked -p madb-cli -- build --root /path/to/workspace --task metadata --execution metadata.execution.json --frozen
# 按需显式准备受限原生依赖；工具可用 --tool-root 指向固定安装根
cargo run --locked -p madb-cli -- build --root /path/to/workspace --task dependencies --execution generate.execution.json --frozen
cargo run --locked -p madb-cli -- generate --root /path/to/workspace --execution generate.execution.json --frozen
cargo run --locked -p madb-cli -- generate --root /path/to/workspace --execution generate.execution.json --frozen --check
cargo run --locked -p madb-cli -- build --root /path/to/workspace --execution build.execution.json --frozen
cargo run --locked -p madb-cli -- package --root /path/to/workspace --execution package.execution.json --frozen
```

metadata 在缺少 IR 时执行真实读取配方，返回固定 IR 路径；生成执行 `GenerateRequest→OutputSet` 并提交源码批次；构建固定该批次，只消费受检依赖输出；打包只接受固定源码和构建批次中的真实证据。未知输出身份先计算、再显式绑定下一阶段，不能在执行中改写任务摘要。

默认构建需要 compile/link/run 三阶段；profile 显式要求并支持 `core.external-runtime-verification@1` 时，构建完成策略改为 compile/link，运行验收交给独立消费者。缺少所选策略的必要阶段时，只报告任务已执行，不提交完整构建批次；参见[执行策略](docs/adr/0010-external-runtime-verification.md)。`package` 产生不可变候选，不能据此宣布 SDK 已认证。

输出以不可变物理批次存储。短提交锁内复核输入和索引代次；旧结果不能覆盖新 current，同身份复用仍校验实际文件。消费者持有固定批次和祖先租约；源码更新清空旧构建与候选的 current，旧消费者继续读取自己的旧批次。

`generate --check` 在系统临时目录真实复算，对比文件与登记身份；不写源码、锁、有效输出或依赖缓存。缺材料、漂移、损坏和工具失败均非零。运行目录按保留策略留下并报告路径：进程组收尾不能证明所有逃逸后代已停止，因此不会由 Drop 自动删除现场。

## Rust ↔ C 首链候选

C05 的真实读取器、生成器和 SDK 已接入上述执行端口，整包验收仍以任务目录与绑定候选的报告为准。先安装固定生成器，再从干净的生产者检出构建三个公开候选；工具安装根与输出目录都必须是新目录：

```sh
.venv/bin/python scripts/install_native_tools.py --root /absolute/path/to/native-tools
.venv/bin/python scripts/build_first_route.py \
  --output /absolute/path/to/candidates \
  --bindgen /absolute/path/to/native-tools/bin/bindgen \
  --cbindgen /absolute/path/to/native-tools/bin/cbindgen
```

默认拒绝有未提交修改的生产者仓库。仅开发验证可加 `--allow-dirty`；脏树产物标记为 `provisional`，不能用于正式 Lab 验收。安装器验证上游归档、内含 Cargo.lock 和实际工具二进制，既有 C03 receipt 可继续使用。

| 候选目录 | 公开消费与范围 |
|---|---|
| `c-to-rust` | C 消费 Rust engine 动态库；未插桩 Release、资源关闭及同步观察回调 |
| `c-to-rust-test` | 单独 profile 的测试候选；提供 barrier、故障注入和资源计数接口 |
| `rust-to-c` | Rust 消费独立 C 静态库；公开 `madb-c-client` 安全包装与真实 bindgen 声明 |

读取器以真实 cbindgen 产物或 C 作者头为输入，由 Clang 提取声明与布局，再按首链已知 roles 降低到契约；范围以[读取器说明](adapters/generators/rust_c/README.md)为准，不接受任意 Rust/C 接口。元数据辅助程序 `madb-native-finalize` 复用工具端核心规则；候选 SDK 不依赖它、MadB CLI 或工具私有 crate。

构建配方先显式 prepare 实际输出身份，再由 C04 静态 DAG 重新编译、链接、运行并核对字节；prepare 不是验收证据。Apple 使用冻结 Xcode/SDK 配方，Linux 为实际宿主记录独立 observed profile；二者的运行与认证分别取证。更多输入、输出与平台边界见[首链工具链](adapters/toolchains/rust_c/README.md)。

把完整候选目录交给独立 [MadB-Lab](https://github.com/changyue1053/MadB-Lab)，在 Lab 检出中执行：

```sh
python3 scripts/check.py --first-route \
  --candidates /absolute/path/to/candidates \
  --output /absolute/path/to/lab-reports
```

Lab 只使用公开头、库、包装和身份记录，在新工程与空开发缓存中编译两个直接消费者；不需要 MadB 源码或生成器。缺工具、目标不符、provisional 候选和必需用例失败均不能算通过。

## Kotlin / Swift / Rust 四路线候选

C06 已提供 `kotlin-jvm-rust`、`kotlin-native-rust`、`swift-rust`、`swift-kotlin` 四配置的公开安装与构建入口。本机正式构建、独立 Lab 的 54 项必需案例和 92 项基础检查通过；按项目所有者确认，本包已验收收口；GitHub Actions 后续独立补跑，不阻塞本地验收。精确范围和证据见[任务目录](docs/catalog/implementation-plan.json)与[C06 记录](docs/reviews/2026-10-24-c06-kotlin-swift-rust.md)。

按照[完整复现指南](docs/examples/managed-routes/README.md)，从固定作者检出依次安装受管编译器/绑定后端、准备离线依赖、构建四个候选，再在独立 MadB-Lab 执行 `scripts/check.py --c06`。指南包含实际命令、候选文件布局、受检 `openEngine` / Swift 门面以及取消和显式关闭责任。

| 环节 | 公开入口 |
|---|---|
| 编译器与锁定 Maven 依赖 | `scripts/install_managed_tools.py` |
| UniFFI / Gobley 源码、补丁与工具 | `scripts/install_managed_backends.py` |
| Rust 离线依赖闭包 | `scripts/prepare_managed_dependencies.py` |
| 冻结读取、生成、构建与候选打包 | `scripts/build_managed_routes.py` |

当前范围为锁定工具链的 macOS Arm64 Release；deployment target 14.0 不代表已在 macOS 14 实跑，桌面 JVM 也不代表 Android。消费方从独立、已验证的候选 `identity.json` 取得 `expectedProfile`，不能用已加载库自报的 profile 代替预期身份。

## Swift 可选观察适配

L04 已启动独立的 [MadBObservation 源码包](sdk/swift/observation/README.md)，将普通订阅的拥有型状态快照投递到 MainActor 本地模型，提供显式启动、请求停止和排水等待。当前是 macOS 开发子范围；真实 SKIE 组合检查使用固定 C06 公开候选，不改变该候选身份，也不代表 SwiftUI/UIKit、iOS 或正式发布包已经验收。九项扩展的完整范围与安排见[实施计划](docs/delivery/01-implementation-plan.md)。

在具备所述 Apple 工具链的开发机运行 `.venv/bin/python scripts/check_swift_observation.py --output /absolute/path/to/new-check-output`；真实 native 组合需要显式指定候选和独立预期摘要，命令见[检查说明](tests/integration/l04_swift_observation/README.md)。通用仓库检查与这一 Apple 目标检查分别执行，不把缺少候选或工具计为通过。

## ArkTS 可选观察适配

L05 已启动 [StateObservation 与 ArkUIStateModel](sdk/arkts/observation/README.md)：复用 C07 状态订阅，提供显式接入、本地停止和可重试异步关闭，页面只管理自己的观察。首片包含独立普通/ArkUI 源码及生命周期检查；正式 SDK 编译与目标运行、完整 native HAR 认证分开记录。十二项必要能力和十二项扩展的选型、交付次序及剩余验收见[实施计划](docs/delivery/01-implementation-plan.md)和[本次记录](docs/reviews/2026-10-25-arkts-extension-delivery.md)。

## ArkTS 环境预检

C07 已提供 `scripts/check_arkts_environment.py`，检查显式选择的应用 SDK 元数据、工具和已连接目标。用法见[预检说明](tests/integration/c07_environment/README.md)。该命令不安装或启动设备；输出是基础环境观察，不代表四路线实现、正式工具组合或 P05/P06 验收通过。当前发现与实施要求见[C07 记录](docs/reviews/2026-10-25-c07-arkts.md)。

C07 四路线已完成既定 8 个候选配置的本地验证并验收收口，涵盖生成、状态/异步/关闭、Release/HAR 与独立消费。适用目标、配置和证据见[C07 记录](docs/reviews/2026-10-25-c07-arkts.md)；GitHub Actions 留待项目完成后的独立专项补跑。

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

MadB-Lab 是独立仓库，使用公开候选和独立预期；不能链接本仓库私有 crate。C03 已提供固定工具、实际生成及同步 FFI 双向运行探针，见[复现入口](tests/integration/c03/README.md)和[验收记录](docs/reviews/2026-10-24-c03-native-probes.md)。C05 与 C06 的资源、并发和公开 SDK 消费均使用独立候选；各自实际结果见对应验收记录，设备和其他配置继续按对应任务验收。

## Quick start for contributors

Install the exact versions above, create the Python virtual environment, install the hash-locked development dependencies, then run `scripts/check.py` with that environment's Python. The CLI supports explicit read-only queries and frozen metadata/generation/build/package recipes. Execution infrastructure does not certify a language SDK; see the guides above. Read the task's linked specification before changing code.

See [LICENSE](LICENSE) and [third-party scope](THIRD_PARTY.md). This repository does not yet provide a production SDK.
