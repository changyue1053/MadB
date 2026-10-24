# C05 Rust ↔ C 与独立 Lab 整包交付

任务：C05 整包；依据 MadB `8fb8dbe00b2c9787b5ce9977c1c2524c7f7a0c1c` 与 Lab `6721ce40ef226cf2466faf0d00681b7bc074600b`、C05 acceptance、DEC-03/07/08/10 及 CRIT-03/08/11/12。两个仓库使用 `codex/c05-rust-c-first-route` 分支。

范围：真实 cbindgen/bindgen 与 Clang 声明读取、受限 ABI lowering、C04 阶段编排、Rust engine 的资源/回调协议、独立 C 提供方与 Rust 安全包装、公开候选和独立 Lab 双向消费。首链执行起点为固定 macOS Arm64；Linux CI 保存自己的实际目标结果，不推导其他设备能力。不包含其他语言、初始化、远程缓存、通用扩展平台或正式版本发布。

## 实施约定

- 延续 C02 状态码、最大 65536 字节输入、SDK 实例登记令牌、request/await/release 三阶段关闭，不改变已有 ABI 行为。
- 补齐 DEC-10 的固定 bootstrap 形状，见[首链规范](../examples/first-route/README.md)。ABI/profile 身份先于资源创建和业务调用检查；不把 candidate_digest 编进库造成循环。
- 每个直接方向使用独立的 `madb.candidate/1` 清单；`candidate.json` 本身是 manifest。默认 Release 与 test-support 候选分开，测试专用导出不能污染 Release 公开接口。
- 观察 v1 发布同步初始快照 sequence=1/state=0，没有计时器、消费者队列和历史回放。测试源驱动同一个真实租约/回调实现，验证源已持有 context 后停止的交错，不建立第二套测试状态机。CRIT-11 只登记适用的重入/退出与清理用例，队列/env 能力明确拒绝。
- 元数据从真实 Clang AST 和 cbindgen 的 C 可见产物取得；策略来自 project/profile。既有规则负责策略解析与摘要，不从测试 IR 复制或由生成器自报通过。
- 工具源码与 SDK 构建单元分离；SDK 不依赖 CLI、app 或工具私有 crate。Lab 只接收公开候选，在独立目录与空开发缓存中构建两个直接消费者。

## 验收

整包完成需：MadB 统一门禁、真实工具生成/再生成及拒绝测试、完整资源/回调交错、两个独立 Lab 消费者、错库/错身份与未插桩 Release 验证、最终提交远程 CI。所有实际命令、候选摘要和运行结果在完成后记录；当前尚未验收，不将基础设施或单向运行计为 C05 完成。
