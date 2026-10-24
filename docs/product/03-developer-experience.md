# MadB 开发者体验与目标使用流程

本页定义待实现的 CLI 与 SDK 使用契约。下面命令和 API 用于驱动实现与验收，当前尚无安装包。发布时必须把这些流程转为 MadB-Lab 的可运行旅程。

## SDK 消费者

消费者通过 Gradle、SwiftPM 或对应 ArkTS 包管理方式安装版本化 SDK，在 IDE 中调用普通类型接口。包中携带所需原生库、语言包装和最低平台说明。消费者不需要安装 MadB CLI 才能使用预生成 SDK；安装说明明确源码或二进制分发、最低工具链和必要编译开关，不能把 producer 的完整工具链要求转嫁给普通消费者；SDK 作者也可以选择在自己的构建中运行锁定版本的生成任务。

消费端示意：

```swift
// 目标 API，签名由具体后端能力与契约生成。
let engine = try SampleEngine.open()
let result = try await engine.checksum(bytes)
let observation = engine.observeState { state in render(state) }
// 页面退出只停止观察；engine 的 owner 在自己的生命周期中关闭它。
observation.requestStop()
await observation.awaitStopped()
```

`requestStop` 表示停止请求；`awaitStopped` 确认该观察实例已满足停止后条件。引擎关闭单独暴露，不混入页面对象析构。Swift、Kotlin、ArkTS 的表面语法可以不同，效果相同。

## 熟悉的语言入口

SDK 作者优先复用后端注解和宏，MadB 生成重复接线；消费者通过普通 API 或可选的属性、状态和作用域封装使用能力。源代码分发或主动使用宏时需要的编译依赖必须明确，普通预生成 SDK 的消费者不强制安装 MadB 自定义插件。

例如 Android 页面采用已有生态 API：

```kotlin
// 目标消费示例；engine.state 是生成 SDK 暴露的 StateFlow。
val state by engine.state.collectAsStateWithLifecycle()
Text(state.title)
```

页面停止收集不一定关闭热流上游；共享策略和 owner 决定原生观察何时停止。属性、宏和装饰器不自动提供线程或所有权保证。完整语言选择、示例与诊断手段见[低门槛 API 设计](../architecture/15-developer-ergonomics.md)。

## SDK 作者

```sh
madb init --template rust-swift --out ./sample-sdk --verify structure
cd sample-sdk
./madbw doctor
./madbw build --task metadata --frozen  # 需要编译元数据的后端先准备快照
./madbw inspect
./madbw check
./madbw plan
./madbw generate
./madbw build --frozen
./madbw package --frozen
```

模板必须包含最小业务实现、导出声明、行为策略、消费示例和验证入口。作者主要补充不能由签名推导的释放、线程、取消与交付语义。真实 Rust 函数仍需要所选后端的导出标记和构建设置；使用 UniFFI 不等于任意普通函数会自动导出。

这条作者旅程显式选择先生成结构，再逐项检查和构建；普通新工程旅程默认由 init 完成目标构建。

## 新工程开发者

```sh
madb init --template kmp-rust-app --out ./MyApp --open android-studio
```

在满足模板要求的开发环境中，一次得到应用外壳、共享实现、绑定、工具锁和 IDE 入口。开发者随后在 Android Studio 或 Xcode 中修改共享代码并直接运行；原生依赖和绑定更新由构建任务接管。鸿蒙组合通过对应 profile 创建 DevEco 工程，不推导任意语言/平台组合都能工作。

生成工程会明确哪个模块提供业务 API、哪个模块包装 native 库、哪个模块是应用；用户正常添加公开依赖即可消费。Android 不要求手工移动 `.so`，官方/CPF 依赖由模板选择各自坐标，Swift 二进制包按已验证的分发配置消费。

CLI 与将来的 IDE 向导使用同一初始化用例。默认验证构建；仅生成、目标运行及离线模式见[完整使用规范](04-project-initialization.md)。工程创建后构建失败时保留目录，使用项目 Wrapper 重试，不覆盖重建。

## CLI 职责

| 命令 | 行为 | 副作用 |
|---|---|---|
| init | 解析模板，创建原生工程并按级别验证；可选打开 IDE | 新目录无覆盖提交；依赖/工具写约定缓存；构建失败保留工程 |
| init --dry-run | 显示文件、工具、下载和检查计划 | 不创建工程、不下载、不执行构建 |
| doctor | 检查环境、锁与前提 | 默认只读；给出安装指导 |
| inspect | 读取新鲜契约快照，输出结构和语义来源 | 只读；缺快照时返回准备任务，不隐式编译 |
| check | 对新鲜快照执行纯规则，产生结构化诊断 | 只写显式报告目录；动态探针由 build/verify 执行 |
| plan | 给出生成、编译、验证任务及摘要；缺快照时给出 metadata 前置计划 | 不执行任务，不暗中运行 Reader 的编译依赖 |
| generate | 生成受管文件及来源清单 | 只写受管目录，事务提交 |
| generate --check | 在临时目录复算并比较生成物 | 不改源码、锁和有效输出；写自有临时目录，漂移返回非零 |
| build / package | 构建或封装锁定产物；build --task 仅执行命名任务子图 | 写构建输出和缓存，不递归调用所属应用构建 |
| verify | 从指定制品运行指定 Lab 套件 | 仅使用隔离消费目录 |
| lock --update | 明确更新依赖解析和锁 | 输出锁变化，不隐式发布 |
| explain | 解释诊断编号和修复路径 | 只读 |

退出码：0 成功，1 检查或测试未通过，2 输入/配置/环境不满足，3 工具内部错误，4 输出冲突。自动化读取 JSON 报告字段，不解析面向人的描述文本。

## C04 当前只读子集

当前源码 CLI 接受显式 `--root`、`--project`、`--request`、`--snapshot`，不隐式发现或更新工程。project 可为 TOML/JSON；其余使用 C02 的 read-request、lock、profile、contract-ir JSON。`plan` 另外接受重复的 `--task-file`，可用 `--task` 选子图、`--active-task` 声明所属调用，拒绝递归。

查询验证请求固定的配置字节、锁内声明的源码/schema 文件、快照身份、结构及项目/profile 策略来源。返回前再次核对读过的字节和缺失条件；变化返回冲突。输出 `TrackedInputsMatch` 只描述已列清单，不把它称作完整依赖闭包的新鲜性证明。编译器来源的注解需要真实 reader 通道，当前不从 IR 自行反推并信任这些注解。

`inspect` 显示符号位置和策略来源；`check` 返回契约检查结果；`plan` 只校验调用者提供的任务摘要、profile/工具身份、无环依赖和输出归属，未提供任务时返回非零。locked components 是显式组件通道；编译工具必须属于所选 profile.tools，不能从全局 lock.tools 借用其他 profile/target 的工具。静态图不验证二进制批次和实际输入产物，不作为执行或发布许可。

此子集的退出码沿用 0/1/2/3；输入在查询期间变化用 4 表示冲突，后续输出提交也使用该类别。输出目前是面向人的文本，没有新增 JSON 公共格式；自动化仅判断退出码。`MetadataRequired` 同时给出既定 metadata 前置任务，明确其执行器/reader 尚待 C04/C05，不声称该命令现已可运行。完整目标命令与 JSON 报告仍按上表实施。

本地文件读取当前限 Unix：选定根目录后，以目录描述符逐段打开，拒绝后代符号链接、硬链接和非普通文件。文档上限 1 MiB、单个跟踪文件 32 MiB、一次查询累计读取 128 MiB；任务文件路径去重。静态受管输出暂限 ASCII 路径并拒绝大小写别名及文件/目录冲突。读取本身不建立操作系统级原子快照；执行器仍须固定不可变输入并实现完整失效协议。

## 配置与错误体验

[设计配置](../examples/madb.example.toml)把结构来源、行为策略和目标路线分开。选择器必须解析到可追踪的符号，改名或失配产生错误，不静默遗漏。

诊断至少包含：稳定编号、问题、源位置、边界和目标、失败依据、下一步操作。例如“此返回缓冲声明为转移所有权，但未指定释放器”，应定位声明及策略，并列出允许的显式处理方式。

例如 SKIE 的某个声明只能生成不可调用占位，或当前 SwiftPM 配置不支持请求的反向直接调用，`check` 对已知限制在生成前解释并给出可用配方；只有消费编译才能确认的可调用性由 build/verify 探针判定；不能等用户在 IDE 中遇到不可用方法。

不支持某个类型时建议收敛导出面、显式适配或选择已验证后端；不能悄悄转成字符串或 JSON。用户数据、凭证、地址等不会默认进入报告。

## 文档与示例入口

快速开始分别覆盖消费 SDK、创建 SDK、初始化跨端工程和添加后端。场景手册围绕“传字节”“取消任务”“订阅状态”“接入宿主能力”等任务组织。高级实现原理放在架构目录，普通旅程只解释会影响业务正确性的语义。

首次使用验收由未参与实现的人在干净环境执行；记录手工接线数量、失败原因和恢复步骤。发布承诺中的每一步都必须在至少一个对应消费工程验证。
