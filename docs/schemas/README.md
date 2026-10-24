# MadB 扩展设计 Schema

本目录是设计期扩展清单和控制协议的机器结构权威源。采用 JSON Schema Draft 2020-12；宿主、帮助程序和文档校验应引用同一份 schema，不手工维护多份相互独立的字段定义。

| 文件 | 作用 | 仍需额外验证 |
|---|---|---|
| [extension-manifest.schema.json](extension-manifest.schema.json) | 扩展身份、组件、路线、资产与执行需求的结构 | 身份和组件唯一性、引用、依赖环、权限、版本交集、能力含义和真实文件 |
| [extension-wire.schema.json](extension-wire.schema.json) | hello、ack、reject、invoke、cancel、result 的帧格式 | 身份与摘要一致性、状态机、锁、取消时序、文件内容和派生进程 |

schema 中可表达的约束尽量直接阻断，例如未知顶层字段、错误的端口名、结果与错误同时存在。路径的结构校验只是一层检查；文件打开时的规范化、链接检查与受管目录约束仍由宿主执行。

开放标识不代表允许任意语义。语言、运行时和路线可以登记新的 ID；core 功能是否已定义、所需规则是否存在，应由[模型演进规范](../architecture/13-contract-evolution.md)及核心功能目录判断。

当前交付范围是扩展清单和控制帧。各端口 payload、完整 IR、项目锁、案例报告的 schema 在对应实施任务中完成；本目录不代表这些格式已经实现。

样本与结构正反例见[扩展示例说明](../examples/extensions/README.md)。未来调整权威文件位置必须同步引用及校验入口，不能保留两份可独立修改的 schema。

## 结构与语义的实现约束

首链 payload 清单、唯一 schema 权威和语义摘要由[契约规范](../architecture/03-contract-and-ir.md)定义。C02 交付这些 schema、共同编码向量及宿主/帮助程序验证；现有两个 schema 的通过不能代替它们。现有文件清单与协议样本继续使用字节摘要，不因 IR 的 JCS 语义投影改变算法。
