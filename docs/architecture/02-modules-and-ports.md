# MadB 最终工程骨架、模块与端口

本规范是 MadB 工具工程目录、模块职责和源码依赖的唯一维护入口。MadB-Lab 的目录由[独立验证工程](../validation/02-lab-architecture.md)维护；扩展、计划与 HTML 引用或生成阅读视图，不另写一套目录。该骨架承接 0.9 设计，表示建仓目标，尚未创建产品源码或完成相应工作包。

模块按变化原因划分。核心处理值、规则和计划，外层处理文件、进程、编译器与设备。工具宿主的任务调度与应用中 SDK 的资源/线程协议分别实现，不合并成跨语言万能运行时。

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
| adapters | app 的端口/公开请求、model、plan 的必要公开值、protocol 及对应工具依赖 | 其他适配器的私有实现；越过宿主修改正式输出 |
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
| 工具锁与目标探针 | toolchains 与独立探针资产；固定编译器、头、库和目标布局 | C03 |
| 流程与输出事务 | cli/bootstrap、app、plan、host；固定批次、输入追踪与失败恢复 | C01、C04 |
| 双向接线 | contracts/generators/toolchains 的实际适配器；bindgen 导入 C，cbindgen 从 Rust C 可见声明生成头；导出实现另有明确责任方 | C05 |
| 公开包和真实消费 | artifacts 准备候选，host 提交；Lab fixtures 与 C/Rust consumers 验证两个直接方向 | C05、C09、C10 |
| 新工程与 IDE 接入 | projects/modules 模板、初始化用例及对应 IDE 配方 | I01–I03 |

源码中的最小安全边界、回调、关闭与等待遵循现有 DEC/CRIT。普通 API 先贯通，便利封装复用同一协议。首链通过不替代其他路线与目标的验收，C09/C10 从 S1 提供首链所需部分并在 S5 完成全矩阵。

## 扩展与工程初始化

作者注解/宏由 helpers 和 contracts adapters 提取；普通 API 与受限外观由 generators 输出；属性封装器、状态、协程和 UI 入口归对应语言 SDK。诊断 sink 可关闭，资源准入和线程保护仍需执行，详见[语言体验](15-developer-ergonomics.md)与[运行时职责](08-runtime-and-idiomatic-api.md)。

`madb-app/src/use_cases/initialize` 编排模板解析、ProjectPlan、纯渲染、Workspace 无覆盖提交、既有构建验证及可选 IdeLauncher。ProjectPlan 与 ContractIR 分离，初始化复用既有 Generator/Toolchain；文件与 IDE 规则见[初始化架构](14-project-initialization.md)。

新增语言通常只增加适配器、清单、必要 SDK、模板和 Lab 消费资产；核心出现新公共语义时按版本演进。扩展清单、握手与接入门禁见[扩展规范](11-language-extension.md)。语言辅助进程崩溃或非法消息转换为明确工具错误，保留受限原始日志、身份与摘要，不能变成“无操作成功”。
