L04 新 macOS native SwiftUI 消费片独立审查，2026-10-27。

结论：最终清单 004 的九文件静态审查无未解决阻断。两项进程清理问题已在审查期间修正并复核。此结论只允许继续最终构建与真实 UI 验证，不代表 native UI、完整 L04、C06 或发布验收通过。最终 004 比已编译 003 多一行路径规范化，尚未取得该最终行的实际构建证据。

审查绑定：Lab `/private/tmp/madb-chief-round10/lab`，基线 `d270293f50c04ebdca31a4a1eaa7335b1e869cd1`；范围为 `consumers/swift/l04_native_ui/` 六文件、`harness/l04_native_ui.py`、`scripts/verify_l04_native_ui.py`、`scripts/test_l04_native_ui.py`。最终清单 `/private/tmp/madb-chief-round11/l04-files-004.json` SHA256 `29f360c0c76eec85e8cdf13973bf229e71d1d6c43963c90d87e7f5c472a2e230`，九项实际 SHA256/size 均一致。最终 App SHA256 `915c13a241f077dc74e84c85a68abb73416e1b6b46742536ae88ef677ef17fd9`，5515 字节。最后状态中的 `scripts/check.py` 由 root 单写登记专属测试，归其另行复核，不纳入本九文件结论。

实际读取依据：Lab 根 `AGENTS.md`、固定 `docs/reference/engineering/07-agent-development.md`、整洁代码、质量门禁、Lab 架构及引用元数据；产品根 `AGENTS.md`、共同方法、catalog L04 acceptance、`docs/architecture/05-lifecycle-and-concurrency.md`、`08-runtime-and-idiomatic-api.md`、`15-developer-ergonomics.md` 的 Swift/Observation 契约和 `docs/validation/07-developer-ergonomics.md` 的适用 DX 条件。读取公开 Observation 导出与公开 C06 facade 以检查接线，没有导入产品私有实现或开发缓存。遵守只读限制，没有生成图谱、编辑仓库、启动 GUI 或执行目标应用。

确认的代码事实：

- `NativeOwner.swift:52–62,106–122,139–142`：应用仅持有一个 `MadBEngineHandle`；普通入口直接使用 `subscribeState/collect`，页面 source factory 使用同一 handle。公开 handle 的复制共享 native engine 与 delivery owner；页面没有第二个业务 engine、Flow 桥或 payload 队列。
- `App.swift:101–105`、`NativeOwner.swift:125–160,186–210`：按钮只改变真实 SwiftUI 视图层级，`onAppear/onDisappear` 发起开始和停止；XCTest 通过可访问性点击操作，没有直接调用生命周期回调。稳定 session/model 身份跨刷新保留，重复 start 在 factory 前拒绝。
- `NativeOwner.swift:199–209,264–285`：应用保留 session 至 `stopAndWait` 成功，再移除强引用并观察弱引用释放；普通订阅仍须为 1、engine 仍开放，随后发布继续验证普通消费。等待使用异步挂起/yield，没有同步阻塞 MainActor 排水。发布、页面清理、释放观察及最终关闭任务均有明确 owner。
- `NativeOwner.swift:213–254`：最终显式 join 发布、每页清理/释放观察任务，停止并等待普通订阅与 collect，再 `awaitClosed`；全部 native 计数归零后才显示 finished/允许 Quit。终止回调继续核清理状态。析构和强杀没有替代 native close。
- `LifecycleUITests.swift:30–78`、`harness/l04_native_ui.py:13–24,112–203`：独立 literal 值 oracle 覆盖 ASCII、Unicode、NUL、空文本/bytes 和边界字节；实际 UI label、普通与页面快照、三次唯一身份、32 帧顺序、资源计数和正常退出分别受检。测试源码不是 app 值表或产品规则的导入者。
- `harness/l04_native_ui.py:52–81`、`verify_l04_native_ui.py:58–65,199–246`：候选复用完整 C06 公开准入，并核独立预期 digest；源码 supplement 核独立 pin、完整文件清单、字节和 mode。构建前后复核消费拷贝、公开输入、Lab 源码、工具镜像及 revision；成功 UI 需真实 XCTest 结果和 nonce/PID 绑定 native 帧，build-only 单独报告。

已解决发现：

- `verify_l04_native_ui.py:148–151`：原先未知/截断 ps 可执行路径可能被当成已退出；现在明确抛错，既不 signal 未确认身份，也不据此宣称退出。
- `App.swift:9–11,30–52`、`verify_l04_native_ui.py:130–167,253–262`：首条 native 日志之前若初始化挂起，原实现没有可归属 PID。现在先以 `O_EXCL|O_NOFOLLOW` 独占创建并同步 nonce/PID/executablePath receipt，再构造 native owner；runner 核限定大小普通文件、当前 nonce、PID 和精确可执行路径。缺失/错 receipt、未知 ps 身份均不构成清理成功；强制终止只记进程清理，最终成功还要求退出未被强制且 receipt PID 等于 native 帧 PID。004 的唯一增量 `App.swift:34` 使用 `resolvingSymlinksInPath()`，使写入路径与 runner 的 `Path.resolve()` 使用一致的 symlink 规范，避免 `/tmp` 别名假失败。

实际验证和证据边界：

- 只读运行公开准入函数，C06 `sha256:872d0cbd49bee21d442f24be6536334a7cc8dff3a525839f2a889a7776ac908c` 准入通过，目标 `aarch64-apple-darwin`，provisional=false。Observation pin SHA256 `58fe0a1e2634947f87a2311f7a0cdb5efd0eaf98cc8c28cfe16f22e4a623b00b` 和全部十文件准入通过，导出产品 revision `9a72a6417858b7ee1a141ce8b13fa47ba7ef2f94`。
- 审查者实际执行 `python3 -B scripts/test_l04_native_ui.py`，Python 3.14.4，18 测试通过。004 未改 Python 文件；结果只证明 parser/准入及 mock 清理拒绝规则，不是 UI 或 native 运行。
- 原始 build001 保留：xcodebuild exit 74，SwiftPM manifest cache 写入被 sandbox 拒绝；`compile=failed, target=not_run`，没有改写为通过。
- 检查 build002/build003 原始命令及每个命令日志 SHA256/size。build003 的 `build-for-testing` exit 0、`TEST BUILD SUCCEEDED`；app 与 UI test 实际编译参数含 Swift 6、`-O`、`-warnings-as-errors`、arm64 macOS 14 target，工程声明 complete concurrency。build003 九文件与清单003及其 lab_inputs 全部一致。
- `/private/tmp/madb-chief-round11/l04-dev-build-003/report.json` SHA256 `f12c0b7405a351276b42054ab47d0426b4b5b44e2ab79740d77783032f36d773`，真实状态为 `compiled-only / compile=passed / target=not_run / owner_close=not_confirmed / provisional=true`；cleanup 无 receipt 为 not_observed，因为应用未启动。
- 003→004 逐项清单比较确认仅上述 App 路径规范化一行；不可把 build003 直接写作最终004已编译。下一次最终 clean 验证必须重新 build-for-testing 后运行，不能复用003 app。

未执行真实 SwiftUI/XCTest 应用运行、native 排水/释放动态观察、正常应用退出或真实失败进程清理；这些仍待目标证据。现有公开 SDK、ABI 和关闭语义未因本片改变；iOS/UIKit、真机/最低系统、并行页面、后台/旋转、native fault/late-callback、C10 分发及整包 L04 均不由本审查外推通过。审查完成，无其他范围扩张。
