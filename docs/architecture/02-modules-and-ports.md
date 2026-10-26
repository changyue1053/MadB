# MadB 最终工程骨架、模块与端口

本规范是 MadB 工具工程目录、模块职责和源码依赖的唯一维护入口。MadB-Lab 的目录由[独立验证工程](../validation/02-lab-architecture.md)维护；扩展、计划与 HTML 引用或生成阅读视图，不另写一套目录。该骨架承接 0.9 设计，表示最终工程目标；实际构建成员和任务证据决定当前实施范围，不能由目录存在推导功能完成。

模块按变化原因划分。核心处理值、规则和计划，外层处理文件、进程、编译器与设备。工具宿主的任务调度与应用中 SDK 的资源/线程协议分别实现，不合并成跨语言万能运行时。

## 控制实现复杂度

保持现有六个核心 crate 的依赖边界；目录中的职责不自动对应新 crate、独立进程或发布包。只有实际依赖隔离、复用或分发需求才增加物理单元，先用模块和纯函数承接变化。扩展也无需重新实现全部五个端口。

ContractIR 承担边界语义，ProjectPlan 承担工程结构，TaskGraph 承担执行依赖；组合根完成具体实现装配。三种模型分别有可检验的职责，不合并成万能配置对象。生成、编译和候选提交继续复用宿主机制，SDK 不加载工具核心。普通使用者不需要理解这些内部模型，角色入口见[开发者流程](../product/03-developer-experience.md)。

## 仓库落点与文档权威

本工作区的 madb 是工具 Git 根，madb_lab 是独立 Lab Git 根；上级仅为工作区容器，不建立第三个嵌套仓库。现有资料统一放入工具仓库 docs。旧工作区 docs 路径只允许作为指向该目录的兼容链接，不维护另一份规范。

Lab 的根入口在独立检出时可用；所需共同规范采用固定 MadB 提交与摘要的只读引用资料，不能以相邻源码目录、机器绝对路径或可变 main 分支充当基线。任务与认证状态只在工具仓库的现行 catalog 中修改。

## 工具工程的目标目录

<!-- madb-project-tree:start -->
```text
MadB/
├── Cargo.toml                         # 工具宿主 workspace 与成员
├── Cargo.lock                         # 宿主依赖；不代替各目标的工具锁
├── rust-toolchain.toml
├── .python-version / .node-version  # 仓库检查环境
├── requirements-dev.lock             # 文档校验依赖及摘要
├── scripts/check.py                  # C01 本地与 CI 共用检查入口
├── README.md
├── AGENTS.md                         # 任务、规范和检查的简短入口；C01 落地
├── LICENSE
├── NOTICE
├── CONTRIBUTING.md
├── SECURITY.md
├── crates/
│   ├── madb-cli/src/
│   │   ├── main.rs                    # 启动与退出
│   │   ├── commands/                  # 参数解析、结果展示
│   │   └── bootstrap.rs               # 组合根：绑定具体实现
│   ├── madb-app/src/
│   │   ├── use_cases/                 # 初始化、检查、生成、构建、验证
│   │   ├── ports/                     # 应用所需的小型能力接口
│   │   ├── execution/                 # 任务执行、取消、结果汇总
│   │   ├── outputs/                   # 生成/产物/候选提交与恢复编排
│   │   └── extensions/                # 解析、协商与装配请求
│   ├── madb-model/src/
│   │   ├── identity/                  # 符号、路线、配置与制品身份
│   │   ├── contract/                  # 结构快照、类型、接口及来源
│   │   ├── policy/                    # 所有权、线程、取消与错误语义
│   │   ├── capability/                # 能力、限制与需求
│   │   ├── profile/                   # 已解析配置
│   │   ├── artifacts/                 # 输出、候选、证据的领域描述
│   │   └── findings/                  # 问题、缺口与来源位置
│   ├── madb-rules/src/                # 纯类型、资源、并发、兼容规则
│   ├── madb-plan/src/                 # TaskGraph、ProjectPlan 与输入身份
│   └── madb-protocol/src/             # schema 对应 DTO、编解码与格式验证
├── adapters/
│   ├── contracts/                    # 源码、明确桥接声明与元数据读取
│   ├── generators/                   # bindgen/cbindgen/UniFFI 等及受限接线
│   ├── toolchains/                   # Cargo/Gradle/CMake/Xcode/Hvigor
│   ├── verification/                 # 探针执行、Lab 调用与观察转换
│   ├── artifacts/                    # 各生态的打包准备实现
│   ├── templates/                    # 模板目录、资产读取与渲染接入
│   ├── ide/                          # 原生 IDE 配方、启动与探测
│   └── host/                         # 宿主控制的 IO、进程、缓存与最终提交
├── helpers/
│   ├── kotlin-metadata/              # 官方/CPF、KSP1/KSP2 配套前端
│   └── uniffi-metadata/               # 与所选后端版本配套的辅助工具
├── sdk/
│   ├── kotlin/                       # 按能力分包；官方/CPF 独立构建与坐标
│   ├── swift/                        # 普通 API、异步、状态及可选便利封装
│   ├── arkts/                        # Promise、观察、env 与生命周期接入
│   ├── rust/                         # 受限安全包装与资源原语
│   ├── c/                            # 必要的公开 ABI 支持
│   ├── cpp/                          # RAII、移动语义与受限包装
│   └── dart/                         # E04 同步 FFI 扩展探针
├── extensions/
│   └── <extension-id>/               # 清单、组件引用与资产索引
├── templates/
│   ├── projects/                     # 完整工程配方
│   └── modules/                      # 可组合模块配方
├── tests/
│   ├── architecture/                 # 依赖方向、可见性与隔离检查
│   ├── contract/                     # schema、协议与能力协商
│   ├── integration/                  # 应用用例、生成事务及故障恢复
│   ├── fixtures/                     # 工具内部测试输入
│   └── golden/                       # 生成形状基线
├── docs/
│   ├── architecture/                 # 当前规范
│   ├── adr/                          # 技术取舍
│   ├── schemas/                      # 对外格式权威
│   ├── catalog/                      # 路线、问题、案例、计划与图示映射
│   ├── engineering/                  # 设计与质量规则
│   ├── validation/                   # 验收规范
│   ├── delivery/                     # 实施和公开交付
│   ├── product/                      # 产品要求及开发者旅程
│   ├── examples/                     # 格式与声明示例
│   ├── references/                   # 一手依据、legacy-boundaries 阅读版与封存快照
│   ├── reviews/                      # 指定快照的审查记录
│   ├── tools/                        # 文档检查与 HTML 生成
│   └── visuals/                      # 从权威资料生成的阅读视图
└── .github/workflows/                # 检查、候选构建与发布
```
<!-- madb-project-tree:end -->

六个 `madb-*` 核心 crate 是工具 workspace 的初始构建边界。各自的 Cargo.toml、lib.rs 和测试随建仓建立；目录树省略这些重复文件。其他目录是职责分组，不能据此推导必须增加同名 crate。Rust 适配器按依赖隔离和复用需要成为实际 workspace 成员，同一后端的紧密协作实现可以放在一个包内，通过公开端口暴露。

帮助程序和 SDK 使用所在语言的原生构建与发布单元；不强制纳入宿主 Cargo workspace。Kotlin 官方线与 CPF 使用独立构建根、插件 classpath、缓存和发布坐标，共享兼容的源码。具体工具版本与目标组合由[技术栈规范](16-technology-stack-and-versions.md)及 profile 锁确定。

ArkTS 的声明读取与绑定生成属于 helpers/generators，env、操作和订阅协议属于相应后端与 SDK 运行时，ArkUI 页面状态适配属于可选 UI 集成，HAR 装配与独立消费属于 artifacts/templates。必要能力与扩展的逐项选择分别以 [ADR-0016](../adr/0016-arkts-production-baseline.md)和 [ADR-0017](../adr/0017-arkts-extension-delivery.md)为准；本目录分工不要求每项能力另建一个包，也不把作者侧编译工具带入普通 SDK 消费流程。

C02 首个实际适配器包为 `adapters/contracts` 下的 `madb-contracts`：只做 schema 文档到领域值的显式转换、语义投影与校验编排，向内依赖 model、rules 和 protocol。它不读取编译器源码、不执行后端，不向核心反向暴露 DTO；C05 的实际来源读取与生成位于外部语言适配器。这一编译边界让格式检查、领域规则和转换各有单一维护位置，架构门禁同时拒绝核心导入该包及该包导入 CLI。

C04 在 `adapters/host` 实现 `madb-host`，依赖 app 的公开读取/进程端口；提供目录描述符相对、有界、拒绝链接的文件读取，以及 Linux/macOS 的受控子进程。`madb-contracts` 实现 app 的格式端口；CLI `bootstrap.rs` 装配只读查询与显式执行用例。app 不导入适配器或 wire 类型，plan 只依赖 model。contracts 在共同受检上下文内绑定执行图；app 负责输入来源、串行依赖调度、图级取消和结果预算；host 只拥有私有图/节点目录与具体进程。图不会公开可独立绕过调度的节点配方，单任务入口仍拒绝依赖。正式执行入口通过 pipeline 管理 metadata、生成、构建与打包；它们不替代 C05 的语言配方实现。

`app/process` 负责一次性执行准入、取消与结果接收的原子先后；`host/process` 负责启动、管道、信号和回收。`ProcessRunner` 返回观察与未回收资源的 owner，`PendingProcess` 允许继续非阻塞回收。app 不把进程零退出当成契约验证、E02 终态或输出提交；状态与预算见[受控进程规范](07-code-generation.md)。

`app/inputs` 通过 ReadWorkspace 捕获与声明大小/字节摘要一致的不可变 FrozenInputs，验证完整清单的路径与预算，不接触文件系统。`host/inputs` 独占创建私有 inputs/work 目录，受限物化并核对实际副本；PreparedRun 消费后成为 RetainedRun，继续持有目录与待回收进程。目录没有自动 Drop 删除；进程收尾不等于目录可安全回收。该层不新增 wire 摘要、不解析任意命令参数，也不替代完整依赖追踪。

`contracts/task` 将完整上下文和 BuildTask 绑定到 app 的 TaskInvocation/BuildRecipe；wire DTO 留在适配器。`app/task` 校验声明输出和日志、捕获不可变结果，`app/process` 把结果校验纳入同一次取消准入。`host/task` 组合固定输入、受控进程和精确目录验证，返回带结果字节的 RetainedRun；无输出提交权限。测试驱动放在 CLI composition root 的 examples，不让 host 依赖 contracts。

`app/input_observation` 拥有 InputWatchSet、InputObservation 和 ObserveWorkspace 端口，声明缺失路径及目录直接成员；host 以描述符相对的元数据/枚举实现。`execute_graph_observed` 在同一图准入内捕获和复核，最后检查已固定的根源字节。原 execute_graph 的固定字节语义保持不变；观察没有新增持久化格式或编译器依赖发现职责。

`app/pipeline` 持有操作级取消控制、父批次租约、阶段来源和复算比较；`app/transaction` 定义三层批次、索引和提交端口，host 实现短锁、持久化、CAS 与恢复。生成、构建和候选不共用一个成功布尔值，存储身份不代表 SDK 认证。

`app/dependencies` 定义发现结果、预备清单和缓存身份；contracts 校验版本化格式，host 通过受限真实 Clang 发现取得可信快照并实现原生准备缓存。加载磁盘清单本身只允许输入复核，不能伪造新的完整性证明。`host/locations` 负责受管目录解析和自有临时位置；CLI 不自行写文件或实现缓存规则。

物理包的 Cargo manifest 或原生构建文件是实际成员与依赖的依据。C01 建立首期可编译的最小 workspace 和包职责说明；后续包随对应工作包加入，不批量创建成功返回的空适配器或无用途的占位包。内部类名与纯函数布局可调整，职责移动或依赖越界必须同步规范与检查。

Agent 入口遵循[开发规范](../engineering/07-agent-development.md)，只链接现有计划、架构和真实检查。目录中列出 AGENTS.md 是 C01 的交付目标，不引入独立任务管理平台，也不表示文件或 CI 已部署。

## 模块职责与状态归属

| 模块 | 输入 → 输出 | 状态与边界 |
|---|---|---|
| cli | 参数 → 应用请求、退出码与展示 | 不实现规则或具体生成流程；bootstrap 是当前宿主组合根 |
| app/use_cases | 请求与端口 → 用例结果 | 管理一次用例的阶段和失败，不直接操作编译器或文件系统 |
| app/execution | TaskGraph → 执行记录 | 拥有工具任务的取消、并发与结果汇总；不管理业务应用的线程池 |
| app/outputs | 已完成输出 → 提交请求/结果 | 编排输入复核、批次固定及恢复；原子 IO 由宿主端口实现 |
| app/extensions | 清单、锁 → 组件解析与协商结果 | 申请受控组件装配，不向所有模块暴露全局 Registry |
| model | 结构、策略、观察 → 领域值 | 不可变快照与受检身份；无文件、进程和全局日志状态 |
| rules | ContractView + Capability + Observations → Findings/Gaps | 纯校验，不扫盘、修源码或执行工具 |
| plan | 受检需求 + 锁 → TaskGraph / ProjectPlan | 计算依赖、任务身份和文件归属，不实际执行任务 |
| protocol | 格式消息 → DTO、编解码及格式错误 | 使用 schema 权威，不承担领域规则和认证判定 |
| contracts / generators adapters | 权威输入 → 快照或受管输出 | 保留来源与精度；不推断未知语义、不静默降级 |
| toolchains / verification adapters | 指定任务/候选 → 构建结果或观察 | 不修改被测候选，不自行授予整条路线认证 |
| artifacts adapters | 已完成输出 → 候选内容 | 只准备包，不持有最终提交或发布权限 |
| templates / IDE adapters | ProjectPlan → 资产、原生配置或启动结果 | 不重新定义边界语义，不覆盖用户文件或虚报未执行的 IDE 验证 |
| host adapters | 受控请求 → 文件、进程、网络、缓存与提交结果 | 实现权限范围、锁/租约、原子 IO 和进程边界；不决定业务类型映射 |
| sdk | 明确协议 → 语言惯用 API | 语言资源、观察和执行器适配；不拥有全局业务账户或业务事务 |

`ContractIR` 描述边界语义，`ProjectPlan` 描述工程模块和文件归属，`TaskGraph` 描述任务依赖。三者通过明确输入输出协作，不合并成包含任意平台属性的万能模型。

## 源码依赖与组合根

以下约束针对生产代码和构建依赖；测试替身可通过公开测试接口组装。目标/feature 组合均须检查，不能只检查默认编译路径。

| 消费模块 | 允许的内部依赖 | 禁止的依赖与行为 |
|---|---|---|
| model | 基础值类型库 | app、protocol、适配器、SDK、IO/进程入口 |
| rules、plan | model | 具体后端、宿主服务；plan 不执行 rules，只消费受检结果 |
| protocol | 序列化与格式校验库 | app、适配器和领域规则；不把 wire DTO 当领域模型 |
| app | model、rules、plan；自身定义的 ports | 具体适配器、具体编译器；不直接读取 wire DTO 绕过转换 |
| adapters | app 的端口/公开请求、model、rules/plan 的必要公开值、protocol 及对应工具依赖 | 其他适配器的私有实现；越过宿主修改正式输出 |
| cli/bootstrap | app 与明确启用的适配器 | 把全局服务容器传进纯规则或 SDK |
| helpers | 对应生态工具与公开协议 | 宿主私有 app 对象、私有 Rust ABI |
| sdk | 对应语言运行库、绑定与必要公开原语 | CLI、app 或工具宿主运行时 |
| MadB-Lab | 公开工具/SDK 候选、公开格式与测试接口 | MadB 私有 crate、开发机缓存或生产规则充当 oracle |

`crates/madb-cli/src/bootstrap.rs` 创建受控宿主服务、端口实现和用例依赖；CLI 命令只调用已装配用例。适配器通过注入的 ProcessRunner、Workspace 等小型端口复用宿主能力，不直接依赖 `adapters/host` 的内部实现。未来 IDE 向导调用既有 CLI 或同一应用用例，不能复制组合规则。

扩展解析可在一次请求内产生确定的组件配置；组合根提供的受控工厂据此创建类型明确的端口代理。注册表不成为运行期间随处查找服务的容器。默认通过版本化进程协议调用独立辅助程序，不加载未知原生插件进入宿主。进程隔离不等于安全沙箱，来源、环境、路径与网络仍需按策略控制。

这些边界落实单一职责、依赖倒置与接口隔离；后端选择采用策略与适配器，绑定集中在组合根。原语优先复用语言和后端已有机制。模式只服务实际变化点，不为每个函数建立接口与工厂。具体门禁见[评审与质量检查](../engineering/05-review-and-quality-gates.md)。

## 端口与协议位置

应用能力接口放在 `madb-app/src/ports/`，请求、结果和领域错误优先使用稳定的模型/计划值。用于单一用例的类型可留在 app；仅供传输的 DTO 放在 `madb-protocol`。适配器负责 wire 与领域值之间的显式转换，格式通过后仍需身份、能力和状态语义检查。

接口草图表达职责，不固定最终 Rust 类型布局，也不宣称下面已是可编译库：

```rust
trait ContractReader {
    fn read(&self, request: ReadRequest) -> Result<ContractSnapshot, ReadError>;
}
trait Generator {
    fn generate(&self, request: GenerateRequest) -> Result<OutputSet, GenerateError>;
}
trait Toolchain {
    fn execute(&self, task: BuildTask) -> Result<BuildEvidence, ToolError>;
}
trait Verifier {
    fn verify(&self, request: VerifyRequest) -> VerificationResult;
}
trait ArtifactPackager {
    fn package(&self, request: PackageRequest) -> Result<Candidate, PackageError>;
}
trait ArtifactStore {
    fn commit(&self, candidate: Candidate) -> Result<ArtifactRef, CommitError>;
}
```

Workspace、ProcessRunner、Clock、NetworkResolver 等副作用端口按需要注入。ArtifactPackager 的实现属于 `adapters/artifacts/`，只写被授予的 staging；ArtifactStore 的实现归 `adapters/host/`，最终提交权由宿主控制。生成源码批次与构建产物的提交复用宿主存储原语，但保持各自清单和验收状态，不把源码批次假称为已认证发布候选。

ContractSnapshot 保留输入摘要、符号、位置和精度；OutputSet 保留 generation_id、任务/profile 身份和文件清单。VerificationResult 的六类结果遵循[测试策略](../validation/01-testing-strategy.md)，与实现状态和证据新鲜度分开。wire 字段、schema 和确定性摘要遵循[契约规范](03-contract-and-ir.md)。内部领域类型可以手写；对外字段只由同一 schema 与共同格式向量约束，不复制维护另一份消息结构。

## 文件所有权与生成位置

| 内容 | 权威与维护方 | 写入方式 |
|---|---|---|
| 工具、适配器、SDK 原语源码 | 对应维护者；SDK 按语言/能力组织 | 代码评审；生成工具不得覆写手写实现 |
| extensions 清单与资产索引 | 扩展维护者 | 引用组件和资产，不复制 adapters 或 SDK 源码 |
| templates 资产 | 模板维护者 | 初始化时交付给新工程；后续升级不得无条件覆写用户文件 |
| 对外格式 | docs/schemas 与共同格式向量 | 生成或核对协议 DTO；其他目录不另建格式权威 |
| 调用方生成绑定 | 固定输入的生成任务 | 受管工作区的不可变批次；不写回 MadB/sdk 手写源目录 |
| 构建产物、候选与缓存 | 宿主存储与对应锁 | 与手写区隔离；消费者固定批次，失败保留旧完整结果 |
| 发布与验证记录 | 固定候选、profile 与 Lab revision | 追加可追溯记录；不得将普通运行缓存当认证证据 |

`app/outputs` 负责何时请求提交、冲突后如何重新计划；host 负责实际目录锁、索引代次比较、读取租约和原子替换。具体状态机、依赖输入和崩溃恢复规则以[代码生成规范](07-code-generation.md)为准，不在各后端分别实现一套。运行时资源关闭则由语言 SDK/绑定/原生实现按机制责任表执行，不能借用工具进程的生命周期代替应用资源管理。

## Rust ↔ C 首链的源码落点

| 资产 | 实施位置与职责 | 工作包 |
|---|---|---|
| 契约、身份与格式 | model、rules、docs/schemas、protocol；明确 Rust/C 声明的权威关系 | C02、E01/E02 |
| 工具锁与目标探针 | `adapters/toolchains/rust_c` 的锁配方、安装、实际工具观察与构建接线；C03 保留独立探针 | C03、C05 |
| 流程与输出事务 | cli/bootstrap、app、plan、host；固定批次、输入追踪与失败恢复 | C01、C04 |
| 声明读取与生成 | `adapters/generators/rust_c`；cbindgen 从 Rust 实现生成 C 可见头，Clang 读取头的声明/布局，bindgen 为 Rust 消费者生成 C 库 raw 声明 | C05 |
| 元数据完成 | `adapters/contracts/src/native.rs` 与 `crates/madb-cli/src/bin/madb-native-finalize.rs`；严格接收 seed/project/profile，复用 rules 解析策略和来源、重算身份/摘要并校验 | C05 |
| 原生 SDK | `sdk/rust/engine`、`sdk/c/first-route`、`sdk/rust/c-client`；独立原生构建单元，持有资源协议或公开安全包装，不依赖工具私有 crate | C05 |
| 公开包和真实消费 | `adapters/artifacts/rust_c/pack.py` 准备公开清单，host 提交；Lab 仅消费三个候选的公开头、库、包装与身份，独立验证两个直接方向 | C05、C09、C10 |
| 新工程与 IDE 接入 | projects/modules 模板、初始化用例及对应 IDE 配方 | I01–I03 |

首链 Python 适配器按明确的 ABI v1 roles 解释编译器声明，不是通用 Rust/C AST 映射器；未知导出、类型、布局或调用约定必须拒绝。`madb-native-finalize` 只在工具端运行，seed 不能自带已解析策略或伪造来源，策略解析与摘要规则仍由既有 Rust 层维护；它不进入候选 SDK。`inspect/check/plan` 仍只读，实际编译器调用由显式读取或构建配方触发。

L02 的自然 Rust 有限值配方按 [ADR-0022](../adr/0022-native-value-recipe.md)增加独立读取与生成分支：contracts 适配器中的纯 syn 读取不读文件或启动编译器，内部 CLI 桥接有界输入输出；外层 rust_c 配方冻结作者源与工具，执行 rustc 见证、生成 shim、cbindgen/Clang 物理验证及既有 C04 流程。新分支复用共同 IR 与宿主事务，SDK 不依赖工具；旧七／九角色读取器保持精确拒绝，不被改成通用映射器。

公开入口是 `scripts/install_native_tools.py` 和 `scripts/build_first_route.py`。后者通过 `adapters/toolchains/rust_c/pipeline.py` 接入 C04：先 prepare 实际编译/链接/运行输出身份以形成静态 DAG，再由受控执行重新构建并核验；prepare 不充当语义预期或验收证据。候选分别为 `c-to-rust` Release、`c-to-rust-test` 测试 profile 和 `rust-to-c`，测试导出不能进入 Release。具体操作见[首链工具链说明](../../adapters/toolchains/rust_c/README.md)。

首链 profile 的 `backend_config` 保持 `null`，内部 `backend.json` 由 ReadRequest 输入与 lock 固定，不放宽通用 backend-config 校验。公开 `toolchain.json` 记录工具镜像与 SDK 内容身份、逻辑名称和编译器语义，不发布宿主绝对路径；执行定位与完整参数留在内部锁定配置。Linux 记录实际 native host 的 observed profile，Apple 校验冻结 Xcode/SDK 配方，各自目标的运行证据分别用于验收。

源码中的最小安全边界、回调、关闭与等待遵循现有 DEC/CRIT。普通 API 先贯通，便利封装复用同一协议。首链通过不替代其他路线与目标的验收，C09/C10 从 S1 提供首链所需部分并在 S5 完成全矩阵。

## C06 四路线的源码落点

| 模块 | 职责与边界 |
|---|---|
| `adapters/generators/kotlin_rust` | 真实声明降低、闭合类型/语义表、metadata/generate 端口；只将已审阅语义交给共同规则，不自建认证器 |
| `adapters/generators/uniffi`、`gobley`、`skie` | 对应上游工具的受管源、补丁、编译导出读取与准确诊断；不进入业务 SDK 依赖图 |
| `adapters/toolchains/common` | C05/C06 共用的冻结流程编排、文件身份和受管编译目录；宿主仍唯一拥有批次提交权 |
| `adapters/toolchains/kotlin_rust` | 四配置工具/依赖闭包、profile 与源码证明、编译/链接/运行配方及公开文件清单 |
| `sdk/rust/boundary-core` | 无第三方依赖的有界资源、同步回调、异步等待和关闭排水协议；不依赖工具核心或 CLI |
| `sdk/rust/uniffi`、`sdk/rust/gobley` | 独立 Cargo workspace，隔离 0.32.2/0.29.4 后端；依赖仅限审阅的精确注册包与共同 SDK 内核 |
| `sdk/kotlin`、`sdk/swift` | 平台语义实现与薄安全入口/并发 facade；保留后端资源所有权，不复制第二套引擎状态机 |
| MadB-Lab C06 | 四类真实消费、独立预期、54 个固定案例及候选/工具身份验证；不导入生产者工具源码 |

两版 UniFFI 的类型读取、运行时和生成器各自绑定，不能共用版本标签假设其 ABI 相同。Python 后端与 Rust 核心之间继续使用既有协议和 `madb-native-finalize`；共同规则只新增错误通道、后端句柄这类语言无关语义。公共打包仍复用既有清单准备器和宿主事务，不按每个语言再建认证/发布协议。

五个 Rust SDK workspace 的依赖由 `scripts/check_native_sdk.py` 检查：C05 两包与 C06 共同内核保持无依赖；两个绑定包只允许准确版本的后端/解析辅助依赖及内核路径。别名、其他目标、开发/构建依赖和额外本地包不能隐藏工具宿主依赖。实际生成 SDK 的独立消费另由 Lab 验证。

## 扩展与工程初始化

作者注解/宏由 helpers 和 contracts adapters 提取；普通 API 与受限外观由 generators 输出；属性封装器、状态、协程和 UI 入口归对应语言 SDK。诊断 sink 可关闭，资源准入和线程保护仍需执行，详见[语言体验](15-developer-ergonomics.md)与[运行时职责](08-runtime-and-idiomatic-api.md)。

`madb-app/src/use_cases/initialize` 编排模板解析、ProjectPlan、纯渲染、Workspace 无覆盖提交、既有构建验证及可选 IdeLauncher。ProjectPlan 与 ContractIR 分离，初始化复用既有 Generator/Toolchain；文件与 IDE 规则见[初始化架构](14-project-initialization.md)。

新增语言通常只增加适配器、清单、必要 SDK、模板和 Lab 消费资产；核心出现新公共语义时按版本演进。扩展清单、握手与接入门禁见[扩展规范](11-language-extension.md)。语言辅助进程崩溃或非法消息转换为明确工具错误，保留受限原始日志、身份与摘要，不能变成“无操作成功”。

## 扩展组合的归属

[组合契约](17-extension-composition.md)细化已有模块职责：组合根解析显式扩展选择，contracts 将受检 wire 转为领域值，rules/plan 检查能力、阶段责任及依赖，app/host 沿既有执行与三层事务提交。语言适配器只承担自己的端口或 SDK 机制，不取得最终提交权。多生成步骤的文件 owner、基础 API 与增强模块的依赖、升级与禁用沿同一契约检查。

该细化不增加平行插件内核、全局服务定位器或运行时对工具核心的依赖。文档中的目标端口名称不表示现行代码已完成同名公开 API；实际交付由 E/I/L 及对应路线验收。
