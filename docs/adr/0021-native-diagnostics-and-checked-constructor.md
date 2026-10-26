# ADR-0021 首链生产诊断与普通 checked constructor

状态：项目所有者已确认“采用修订2，进入实现”。日期：2026-10-26（America/New_York）。关联 L02、现有 C→Rust 首链；不新增平台承诺，不改变 C05/C06/C07 历史验收范围。

采用[SDK 修订2](../reviews/l02-native-sdk-decision.md)及其[验证向量](../reviews/l02-native-sdk-vectors.json)，完整 ABI、逻辑投影、失败清零、身份及所有权边界以该冻结文本为准。批准提案 SHA-256 为 `5489d3b10f0164f3a7b73bade3e74f5b814d575d7350c41f05b6ee724d45e036`；[最终独审](../reviews/l02-native-independent-review.md)无设计阻断。原数组方案被现行 IR 的标量 buffer 限制否决，修订2只拉取一条固定记录，不扩共同 IR。

- 显式 diagnostics feature/profile 提供 configure/drain 两个生产入口。默认保持7个 native 导出，旧 test-support 为14，新诊断为9，诊断伴随为16；分别固定声明、工具、身份和候选，不能互借通过结果。
- 每个 loaded image 拥有静态64槽、默认关闭的采集及独立诊断计数；单次 try_lock、FIFO、满载丢新、不增线程或外部 sink。诊断不改变业务返回、所有权、SourceLease、准入与停止排水。
- drain 每次复制一条 Event 和 Stats 纯值，明确存在标志；四输出独立清零与拒绝语义按修订2。SDK 不持有消费者 exporter 或 context，不把计数当资源守恒证明。
- 本次以后新生成的 C→Rust 候选提供独立 `madb_rust_client.h`。checked constructor 自动带入同次受检 identity 的 interface/profile 字节，先 bootstrap 核对，再且仅再 create 一次；不匹配不 create。旧 native ABI 保留，成功句柄仍执行原关闭/等待/释放协议。
- 原首链 profile/1 不加入跨组件责任 wire。逐符号阶段责任仍按既有 DEC-07 交付，不能拿组件叙述代替；本决定不授权另一套共同协议或绕过 profile/2 的既有准入。

必须复核真实 compiler/reader/lowering/finalizer、默认与诊断的精确导出、关闭采集下的正确性、预算/争用/重入/失败及候选身份。原生并发独审与干净独立 Lab 消费是必要检查；临时 mock、声明解析和 Rust 单测各按其范围记录。实现和验证由当前主协调安排，状态只写 catalog；不因此授权 Actions、PR、标签或发布。
