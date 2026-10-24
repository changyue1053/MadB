# MadB 架构与实施视图

这些 HTML 是设计文档的离线阅读入口。样式、脚本和目录数据嵌入文件，无 CDN、外部字体或网络请求；跳转到 Markdown 规范需保留 `docs` 相邻目录。未使用 archify。

## 十三个阅读入口

| 文件 | 用途 | 交互 |
|---|---|---|
| [全部路线与扩展](route-atlas.html) | 九组边界、十六条主线及扩展方案的统一导航 | 搜索路线与配置；跳转专题与任务 |
| [架构全景](architecture-overview.html) | MadB 与独立 MadB-Lab 的职责、调用路线、技术决策与认证范围 | 选择链路与模块；展开两个工程目录；筛选路线；跳转工作包 |
| [实施计划与进度](implementation-progress.html) | 阶段、工作线、前置条件、验收与真实状态 | 按阶段 / 范围 / 状态搜索；查看依赖；导入导出本地进度草稿 |
| [Kotlin/Native ↔ Swift](kotlin-swift-architecture.html) | SKIE 主线、NativeCoroutines 扩展、分发配置与真实消费者检查 | 切换配置、Async / Combine / RxSwift；查看构建、订阅、取消、反向宿主与类型路径 |
| [Kotlin/Native ↔ ArkTS](kotlin-arkts-architecture.html) | CPF 工具链 + akinterop 主线、KNOI 独立扩展、载荷、env 与发布身份 | 切换后端与载荷；查看工具链、注册、互调、资源与环境退出时序 |
| [Kotlin ↔ Rust](kotlin-rust-architecture.html) | UniFFI Kotlin/JVM、Gobley Kotlin/Native 与独立 CPF 约束 | 切换配置、链路和节点；查看异步模板及 SDK 装配门禁 |
| [Swift ↔ Rust](swift-rust-architecture.html) | UniFFI Swift、显式边界适配与 Apple SDK 分发 | 查看生成、调用、宿主回调、关闭与交付 |
| [ArkTS ↔ Rust](arkts-rust-architecture.html) | napi-ohos 主线与 UniFFI-ArkTS 实验 | 比较后端、env/线程和多库交付要求 |
| [ArkTS ↔ C/C++](arkts-c-cpp-architecture.html) | Node-API 的 C/C++ 适配、线程与资源治理 | 比较配置，查看回调投递、停止与环境退出 |
| [Swift ↔ C/C++](swift-c-cpp-architecture.html) | Clang、直接 C++、C ABI 和 Objective-C++ 配置 | 比较调用方向、构建与分发条件 |
| [Kotlin ↔ C/C++](kotlin-c-cpp-architecture.html) | JVM JNI 与 Native cinterop / C 适配器 | 比较运行时、注册、引用、线程与 native 打包 |
| [Rust ↔ C/C++](rust-c-cpp-architecture.html) | bindgen 输入、cbindgen 输出、CXX 及显式 C ABI | 区分生成方向、unsafe 义务与构建边界 |
| [扩展方案](extensions-architecture.html) | 既有增强、实验后端、新语言和工具工程扩展 | 选择扩展配置、查看生命周期、接入门禁与关联任务 |

十三页提供明暗主题、打印和键盘入口；窄屏图形采用局部滚动。图形和阶段矩阵在自己的容器内横向滚动，正文纵向阅读；总览的技术决策链接可打开计划中的具体工作包。工作包深链接形如 `implementation-progress.html#task-C06`。

## 视图范围与阅读方法

- Swift：默认 SKIE；NativeCoroutines 按声明分工并核对 stream identity。SKIE 分发配置、包含 cinterop 类型的原始 / 增强入口、反向宿主可见性均需真实消费探针。不同方向和构建系统的支持范围分别说明。
- ArkTS：图形、默认配置和节点详情均采用 CPF + akinterop 主线。CPF 是工具链，akinterop 是调用桥，CMP 为可选 UI；KNOI 独立扩展，不串接到主线。官方与 CPF 构建根、插件、缓存、发布坐标及根元数据隔离。
- 总览与计划：显式展示 Gobley 修复候选门禁、Android KMP 插件及 native 打包、双根独立发布、Swift 消费检查和 UniFFI ArkTS 实验扩展。实验配置不能继承主线认证。

路线专题使用八段结构：内容依次为完整全景、构建交付、运行时序、类型错误、生命周期、模块原则、验收矩阵和依据。每个配置有实际机制节点与具名连线，默认展示完整关系；调用、反向和关闭另用时序图解释。类型路径、所有权树、失败交错和语义示例独立呈现。不同方向的箭头只表示声明范围内的调用或协议关系，不能推导任意语言对象均可互传。扩展页的工程工具链路按自身职责解释，不冒充跨语言运行时。

具体版本和选型以[技术栈规范](../architecture/16-technology-stack-and-versions.md)为准。专题页使用目录数据展示关联工作包与认证状态；图中的 API 和运行协议是设计，不表示已经有可安装的产品实现。

## 数据与模板的单一来源

| 内容 | 修改位置 | 边界 |
|---|---|---|
| 基线、工作包、依赖、责任和状态 | [implementation-plan.json](../catalog/implementation-plan.json) | 任务计数不代表工时百分比 |
| 工具 / Lab 目录树 | [模块规范](../architecture/02-modules-and-ports.md) / [Lab 规范](../validation/02-lab-architecture.md)的具名 text 块 | 总览直接抽取并转义；缺失、重复或非法块使生成失败 |
| 系统模块与链路 | [architecture-map.json](../catalog/architecture-map.json) | 行为以链接的架构规范为准 |
| 主线路线、方向与认证引用 | [routes.json](../catalog/routes.json) | 只有真实证据支持认证 |
| 问题与验收组统计 | `catalog/problems.json`、`catalog/case-groups.json` | 总览显示目录口径 |
| 路线专题的配置、完整拓扑、时序与案例 | `catalog/route-views/*.json` | 展示用数据，引用既有规范；不是新的产品 IR/schema，不能增加认证 |
| 页面文字、样式与交互 | `tools/templates/` | 通用专题共用 `route-panorama.html/.css/.js`；不手改输出 HTML |

页面从目录读取基线与可变计数；增减工作包后无需手改统计数字。工作包依赖是验收前置，允许提前探针和并行实现；C09 / C10 从 S1 开展、S5 收口。主线工作包、主线路线、扩展工作包与认证配置是不同口径，不相加成“完成度”。

## 更新与验证

1. 将真实交付、阻塞原因与证据写入目录；责任角色落实为具体 `owner` 后再安排日期，没有依据时日期保留 null。
2. 实施状态使用 `not_started / in_progress / blocked / implemented`；验证状态使用 `missing / running / failed / passed / stale`。
3. 仅 `implemented + passed + 有效证据` 计入验收。证据必须对应候选、工具、目标、Lab revision、用例与报告；填写文本不产生认证。
4. 路线认证另更新 `routes.json`。不能从某个工作包完成推导整条路线的所有目标均支持。
5. 在文档根执行 `python3 tools/render_visuals.py`，再执行 `python3 tools/render_visuals.py --check`；生成后的十三个 HTML、展示数据与模板一起提交；生成器检查图节点/连线、当前配置的生命线与场景消息、任务/路线引用、图形边界和十六条主线覆盖。
6. 浏览器检查桌面 / 窄屏、主题、节点详情、路线 / 工作包筛选、深链接、专题配置与时序，以及控制台错误。计划草稿功能另检查导入导出和基线隔离。

## 浏览器进度草稿

计划页将草稿存入浏览器 `localStorage`，键包含目录文件摘要。导出格式为 `madb.progress-draft/1`；导入仅接受同一基线摘要和已知任务，并合并对应草稿。基线改变后需要人工复核，旧草稿不会覆盖新计划。

草稿不能编辑验证通过、认证数量或发布权限。维护者审核后更新目录并重新生成页面；多人协作看板可以用同一工作包 ID 对接 GitHub Issues / Projects，避免第二套编号。

## 验证与维护边界

文档页面检查与产品验收分开。静态检查确认引用、结构、目录和生成一致性；DOM 替身检查指定脚本行为；真实浏览器另验证桌面/窄屏、字体、滚动、交互和控制台。未执行项目必须如实记录，旧浏览器结果不覆盖新页面。

在 docs 目录执行：

```sh
python3 tools/check_docs.py
python3 -m unittest discover -s tools/tests
node tools/tests/panorama_smoke.cjs
```

静态检查需要 Python jsonschema 和 Node.js，可通过 `--node` 指定 Node。C01 在工具仓库根提供 .python-version、.node-version、requirements-dev.lock 和环境安装说明；使用仓库虚拟环境执行上述命令，不能依赖作者机器临时目录。研究阅读版与原始快照均在 docs 内，当前检查无需 --reference-root 或旧项目目录；该参数仅供旧文档快照兼容。缺校验依赖应报告未执行，不静默跳过。

专题展示数据使用 `madb.route-detail-view/3`。`detail.graph` 表达机制拓扑，`detail.sequences` 表达消息时序，models/owners/modules 保存相应语义。风险行必须显式关联问题和工作包；不按关键词猜任务，不要求各页固定节点、卡片或代码行数。

实施计划页的 DEC-01 至 DEC-12 表从 `implementation-plan.json` 的 `decision_gates` 生成，显示采用的决定、准入时点和关联任务。它不产生独立完成率，产品进度仍来自工作包和真实证据。P03/P05 是对应配置的风险门禁，不是 Rust/C 首链的统一前置。

当前变化与历史检查通过[复核索引](../reviews/README.md)查阅；生成 HTML 不直接编辑，规范变更先更新模板/目录再运行 render/check。

架构总览的“工程骨架”区通过 `#project-skeleton` 定位；目录保持原始层级并可独立滚动/折叠。进度页链接 C01、C04、C05；完整验收仍由目录任务数据生成，不增加骨架完成百分比。
