# MadB 设计模式使用指南

设计模式用于封装已存在的变化点。每次引入模式都记录问题、替代方案、复杂度和验证方式；不按“使用模式数量”评价设计质量。

## 模式目录

| 模式或方法 | 使用场景 | MadB 示例 | 约束 |
|---|---|---|---|
| Adapter | 外部机制与公共协议不同 | UniFFI Reader、Clang Toolchain、Node-API Executor | 不把不支持能力伪装为已实现 |
| Strategy | 存在相同目的的不同算法或表示 | 整数映射、错误映射、包输出 | 选择依据来自 profile，避免隐式全局开关 |
| Facade | 用户只需一个稳定的使用面 | SampleEngine 的语言 SDK | 保留取消、关闭和成功阶段语义 |
| Pipeline/任务图 | 多阶段有输入输出依赖 | 读取、检查、生成、编译、封装 | 节点显式声明依赖和产物，失败不执行不安全下游 |
| 状态机 | 有效状态及转换受限 | Operation、Subscription、Resource | 先定义转换表，再决定是否需要类层级 |
| Factory/Registry | 按受控配置创建后端 | 组合根注册 capabilities 与实现 | 不演变成任意位置访问的 Service Locator |
| RAII/Scope Guard | 同步资源需要确定性回收 | 文件、临时目录、调用许可 | 异步排水不能仅靠析构完成 |
| Observer/Subscription | 长期推送数据 | 状态与事件订阅 | 令牌、容量、停止和回调上下文必须明确 |
| Anti-corruption Layer | 外部模型与公共语义不同 | JNI 错误或 N-API 状态转领域错误 | 不丢失错误原因和精度信息 |

[工程初始化](../architecture/14-project-initialization.md)复用这些模式：Adapter 隔离 IDE 工程格式，声明式组合表达模板模块，任务图驱动依赖构建，状态机区分 source-committed 与 verified。模板渲染保持纯函数；不为每个模板建立继承层级，也不让 IDE 向导重写初始化业务逻辑。

## Adapter 与 Strategy 的区别

Adapter 隔离外部接口：把 clang、UniFFI 的输出变为 ContractSnapshot。Strategy 在已知契约内选择行为：例如特定整数是直接映射、范围检查还是明确的大整数包装。把两者混在一个万能 Backend 中，会导致读结构、启动编译和运行时策略相互耦合。

## 状态机示例

订阅通常用枚举加受保护状态足够：Active、Stopping、Stopped。requestStop 从 Active 切换为 Stopping；停止准入、注销和回调排水完成后转 Stopped。Repeated stop 返回同一停止结果。只在状态对应行为确实复杂且持续变化时使用 State 对象模式。

## RAII 与异步协议配合

调用许可可由 RAII 保证退出时归还；整个引擎关闭则需要先禁止新许可，再等待已有许可归还。析构不在 UI 线程无限等待，也不从当前回调同步 join 自己。以“智能指针已经存在”为理由省略关闭协议是不成立的。

## 组合与替代

优先组合 Reader、Generator、Verifier、Toolchain。单个纯函数即可表达的规则保留为函数；只有需要运行时替换、隔离外部依赖或独立发布时才引入端口。重复出现的真实变化点用于验证抽象是否合适。

新增语言沿用这些组合：Adapter 隔离上游工具，Strategy 实现受检映射，Registry 只在组合根绑定实现。扩展清单描述组件关系，不成为可执行脚本；ArtifactPackager 准备包，宿主 ArtifactStore 提交包。完整约束见[扩展设计](../architecture/11-language-extension.md)。

可接受的技术重复优于错误的语义合并。例如 JNI 与 Node-API 都有“附着环境”的表象，但不能共用一个假定线程规则相同的清理函数。

后端选择采用受检配置策略：同一语言对允许源码共同构建、独立二进制 SDK、既有 C ABI 等不同配方。规则返回选择依据和缺失条件，不能在失败时自动切换后端。具体 Android 模块、Swift 包装和双根发布由适配器组合完成，不为每种技术栈再建一个全功能工厂。

## 常见反模式

- 大量平台 if 分散在模型、规则与 CLI。
- 每个规则都配置工厂、单例、基类与注册钩子。
- 构造函数执行网络下载、创建全局线程或生成文件。
- 装饰器/属性封装器隐式拥有并关闭共享引擎。
- 用异常吞噬、无界队列或“永远重试”修饰失败路径。
- 一个类同时扫描源码、决定语义、生成、编译、发布并渲染日志。

评审以更改成本、理解成本与可验证性判断模式价值，相关决策写入 ADR。
