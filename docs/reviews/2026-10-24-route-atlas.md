# 剩余路线与扩展架构全景：交付记录

日期：2026-10-24。沿用设计基线 0.7。本轮新增展示数据和 HTML，不改变主线目录、任务状态、版本候选或认证。

新增 **9 个 HTML**：7 组语言边界专题、1 个扩展专题、1 个统一导航页；加上既有 4 页，当前共 **13 页**。新增专题包含 **29 个配置、348 个节点、145 条流程**。

## 阅读入口

- [全部路线与扩展导航](../visuals/route-atlas.html)
- [整体架构](../visuals/architecture-overview.html)
- [实施计划](../visuals/implementation-progress.html)

| 新增专题 | 配置数 | 内容 |
|---|---|---|
| [ArkTS ↔ C / C++ 架构全景](../visuals/arkts-c-cpp-architecture.html) | 2 | Node-API → C ABI；Node-API → C++ 资源模型 |
| [ArkTS ↔ Rust 架构全景](../visuals/arkts-rust-architecture.html) | 2 | ArkTS · napi-ohos 直连；ArkTS · UniFFI 实验生成 |
| [扩展路线与工程增强全景](../visuals/extensions-architecture.html) | 11 | SKIE + NativeCoroutines；官方 Swift export；KNOI 独立互通后端；ANI → Rust / C++；桌面 JVM FFM / JNA；Crubit Rust ↔ C++；Dart Native → C ABI 探针；UniFFI-ArkTS beta；远程制品存储；IDE 向导与编辑器集成；深度插桩与编译器插件 |
| [Kotlin ↔ C / C++ 架构全景](../visuals/kotlin-c-cpp-architecture.html) | 2 | Kotlin/JVM · JNI C/C++；Kotlin/Native · C ABI / cinterop |
| [Kotlin ↔ Rust 架构全景](../visuals/kotlin-rust-architecture.html) | 3 | Kotlin/JVM · UniFFI + JNA；Kotlin/Native · Gobley 配套；CPF/OHOS · Gobley 独立门禁 |
| [Rust ↔ C / C++ 架构全景](../visuals/rust-c-cpp-architecture.html) | 3 | Rust 消费 C：bindgen；C 消费 Rust：cbindgen；Rust ↔ C++：CXX |
| [Swift ↔ C / C++ 架构全景](../visuals/swift-c-cpp-architecture.html) | 4 | C API · Clang 导入；C++ · 源码直接互操作；C++ · C ABI 分发；C++ · Objective-C++ 适配 |
| [Swift ↔ Rust 架构全景](../visuals/swift-rust-architecture.html) | 2 | Swift · 应用内构建；Swift · 独立 SDK 分发 |

## 内容组织

每个配置统一展示三层节点：声明/生成/装配，调用/适配/实现，所有权/执行器/生命周期/验证。五条链路分别解释构建、调用、反向回调、关闭与交付；箭头表示架构关系，具体动作顺序和前提另以步骤说明。节点详情包含职责和具体接口或产物。

生成与手写职责、风险和实际案例、平台制品、独立消费者门禁均独立列出。配置切换不会修改实施状态；实验或可选配置不继承关联主线的认证。

扩展页覆盖现有扩展目录全部方向，并展开 Dart、IDE 与插桩边界。远程制品存储尚无独立工作包，图中明确标注待拆分，C10 仅关联共有发布规则。Dart 探针不扩展为全语言能力承诺。

## 维护方式

- `catalog/route-views/*.json` 是展示数据，引用既有规范，不是产品 IR 或支持认证清单。
- 新专题共用 `tools/templates/route-panorama.html/.css/.js`；导航使用 `route-atlas.html` 模板。
- `tools/render_visuals.py` 统一生成全部 13 页，并检查 16 条主线覆盖、任务和路线引用、节点顺序与流程引用。
- 两份 README 和既有四页导航已同步；原有六份 canonical catalog 的字节摘要保持不变。

## 验证结果

详见[机器记录](route-atlas-validation.json)。

- 独立架构审查覆盖 8 份数据、29 个配置，没有发现阻断性架构问题。
- 13 个 HTML 的静态结构、256 个静态链接和 13 段脚本语法检查通过；统一生成无漂移。
- 使用隔离的 DOM 形状单元环境执行新模板脚本：29 个配置、145 次流程切换、1740 次节点键盘选择，以及深链接、hashchange、主题和导航筛选通过。该检查不运行真实浏览器，不验证布局、渲染、滚动、浏览器 API 或完整可访问性。
- 检查中修复了 C/C++ 筛选与 C / C++ 标题空格不一致的问题。
- 本地 file:// 页面受到浏览器工具 URL 策略限制；未绕过策略，本轮未完成真实浏览器视觉与交互复验。
- 未运行跨语言 SDK、目标设备或编译器矩阵测试，产品状态未提升。

## 本轮文件变化

| 文件 | 处置 |
|---|---|
| [README.md](../README.md) | 修改 |
| [catalog/route-views/arkts-c-cpp-architecture.json](../catalog/route-views/arkts-c-cpp-architecture.json) | 新增 |
| [catalog/route-views/arkts-rust-architecture.json](../catalog/route-views/arkts-rust-architecture.json) | 新增 |
| [catalog/route-views/extensions-architecture.json](../catalog/route-views/extensions-architecture.json) | 新增 |
| [catalog/route-views/kotlin-c-cpp-architecture.json](../catalog/route-views/kotlin-c-cpp-architecture.json) | 新增 |
| [catalog/route-views/kotlin-rust-architecture.json](../catalog/route-views/kotlin-rust-architecture.json) | 新增 |
| [catalog/route-views/rust-c-cpp-architecture.json](../catalog/route-views/rust-c-cpp-architecture.json) | 新增 |
| [catalog/route-views/swift-c-cpp-architecture.json](../catalog/route-views/swift-c-cpp-architecture.json) | 新增 |
| [catalog/route-views/swift-rust-architecture.json](../catalog/route-views/swift-rust-architecture.json) | 新增 |
| [reviews/2026-10-24-route-atlas.md](../reviews/2026-10-24-route-atlas.md) | 新增 |
| [reviews/route-atlas-validation.json](../reviews/route-atlas-validation.json) | 新增 |
| [tools/render_visuals.py](../tools/render_visuals.py) | 修改 |
| [tools/templates/architecture-overview.html](../tools/templates/architecture-overview.html) | 修改 |
| [tools/templates/implementation-progress.html](../tools/templates/implementation-progress.html) | 修改 |
| [tools/templates/kotlin-arkts-architecture.html](../tools/templates/kotlin-arkts-architecture.html) | 修改 |
| [tools/templates/kotlin-swift-architecture.html](../tools/templates/kotlin-swift-architecture.html) | 修改 |
| [tools/templates/route-atlas.html](../tools/templates/route-atlas.html) | 新增 |
| [tools/templates/route-panorama.css](../tools/templates/route-panorama.css) | 新增 |
| [tools/templates/route-panorama.html](../tools/templates/route-panorama.html) | 新增 |
| [tools/templates/route-panorama.js](../tools/templates/route-panorama.js) | 新增 |
| [visuals/README.md](../visuals/README.md) | 修改 |
| [visuals/architecture-overview.html](../visuals/architecture-overview.html) | 修改 |
| [visuals/arkts-c-cpp-architecture.html](../visuals/arkts-c-cpp-architecture.html) | 新增 |
| [visuals/arkts-rust-architecture.html](../visuals/arkts-rust-architecture.html) | 新增 |
| [visuals/extensions-architecture.html](../visuals/extensions-architecture.html) | 新增 |
| [visuals/implementation-progress.html](../visuals/implementation-progress.html) | 修改 |
| [visuals/kotlin-arkts-architecture.html](../visuals/kotlin-arkts-architecture.html) | 修改 |
| [visuals/kotlin-c-cpp-architecture.html](../visuals/kotlin-c-cpp-architecture.html) | 新增 |
| [visuals/kotlin-rust-architecture.html](../visuals/kotlin-rust-architecture.html) | 新增 |
| [visuals/kotlin-swift-architecture.html](../visuals/kotlin-swift-architecture.html) | 修改 |
| [visuals/route-atlas.html](../visuals/route-atlas.html) | 新增 |
| [visuals/rust-c-cpp-architecture.html](../visuals/rust-c-cpp-architecture.html) | 新增 |
| [visuals/swift-c-cpp-architecture.html](../visuals/swift-c-cpp-architecture.html) | 新增 |
| [visuals/swift-rust-architecture.html](../visuals/swift-rust-architecture.html) | 新增 |
