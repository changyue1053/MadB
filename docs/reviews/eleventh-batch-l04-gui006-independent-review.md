# GUI006 实际运行独立复核

结论：GUI006 的 macOS Arm64 / SwiftUI / SKIE + 公共 Observation 消费切片实际通过，无阻断发现。此结论来自原始命令、XCTest、32 帧和退出证据复核，不仅采用 report summary；不扩大为完整 L04、iOS/UIKit、C06 新候选或发布认证。旧失败记录保持不变。

`gui006/report.json` SHA256 `d293ea897b167bd6db801e744804422d51aeabad885c817390caa12d4947dc04`、62575 字节匹配。实际执行日志绑定集成 Lab `/Users/changyue/Desktop/MadB/madb_lab` 的 clean revision `7a252092a0b50850c961f7d617facce20fbc57a2`，provisional=false。全部 29 条命令引用、78 项 committed Lab 输入、87 项复制消费输入、4 项应用产物、9 项工具/SDK/签名输入的大小与 SHA256 已独立核对；前轮获审九文件清单亦与本次 committed 输入一致。

candidate、public_source、tools 与 GUI005 完全一致。复用候选 digest `sha256:872d0cbd49bee21d442f24be6536334a7cc8dff3a525839f2a889a7776ac908c`，原候选 58 文件匹配；公共 Observation 10 文件匹配，pin SHA256 `58fe0a1e2634947f87a2311f7a0cdb5efd0eaf98cc8c28cfe16f22e4a623b00b`，产品源 revision `9a72a6417858b7ee1a141ce8b13fa47ba7ef2f94`。runner 源码在目标后重新核复制输入、Lab 输入、上游候选/公共源、工具字节及 Lab HEAD，再决定结果；本次原件无该阶段失败。未导入产品私有源。

logs/012 的 Release build-for-testing exit 0，实际编译命令含 Swift 6 与 warnings-as-errors。三产物 codesign strict verify 均 exit 0，实际 Signature=adhoc / TeamIdentifier=not set；这仅为本机开发签名。logs/023 的 test-without-building exit 0、timeout=false，原始 XCTest suite 完整执行 1 个测试，58.062 秒、0 failures。logs/024 的真实 xcresult summary 为 Passed / 1 passed / 0 failed / 0 skipped，与 report 完全相符。

原始 `target-evidence.jsonl` 恰为 32 帧、22987 字节，SHA256 `b2448a6f1c9edca403bf818d4c0147d21073959e2d339b365aeda01cfbeaf769`。本独审与并行只读帧复核均逐项核验：所有帧统一 PID 60519、nonce `67a278debddf4c8d8f2e2d34a7ad0977`、mainThread=true，bootstrap interface/profile 与候选身份一致。三轮 fresh UUID 不同；创建时普通订阅 active=1，真实 onAppear 后 active=2、factory 总数依次 1/2/3；refresh 与 duplicate 不增加 factory。

第 7/16/25 帧 page 与 ordinary 双方值一致，分别为 `native page one` + `007f80ff`、`你好é🙂` + `00c3a9ff`、`left\0right`（真实嵌入 NUL）+ 空 bytes。第 11/20/29 帧 ordinary 分别收到 `after one` + `ff00`、`页面退出后` + `8001`、空文本 + `00`。这些值与独立 XCTest 的完整 value 等值字面量吻合；原 XCTest 实际完成所有相关断言，未依赖截图截断内容或放宽比较。

每轮事件均严格为 page-disappeared → page-drained → model-release-observed → after-page-value；状态依次 stopping、stopped，弱 model 释放后才记录 release。页面退出后 native activeSubscriptions 保持 1，普通订阅继续接受发布，最终 deliveries/publications=6。全部观测帧的其他 native 资源计数为零。源码接线为实际 SwiftUI onAppear/onDisappear，测试通过按钮操作驱动，不直接调用生命周期回调。

末三帧依次 ordinary-drained（active=0、closed=false）→ owner-closed（closed=true）→ application-terminated。任务 join 的依据是 NativeOwner 源码依次 await publication、cleanup、release、ordinary subscription/collector，再 await native close；帧本身没有 ownerTasksJoined 字段。native close 后至 owner-closed 是无 suspension 的 MainActor 尾段；termination 回调再次检查任务容器和 ordinary 引用已清空。

原 XCTest 先断言 owner-state=finished 及完整 `active=0 pages=3 drained=3 released=3 factories=3 deliveries=6 ordinary=after-3 closed=1`，随后点击 quit，等待 Not Running 后才 teardown；日志没有 emergency terminate。receipt SHA256 `7b7c1490a49a14cb40ed205cc7f570b2882fee48669b6b3dabb3633d999307a3` 匹配同 nonce/PID，绝对路径 alias 规范化绑定本次应用。logs/028 精确 PID 查询 exit 1、空输出；cleanup exited / forced=false。logs/026–027 两次本次专属 XCTest runner inventory 均为空。正常退出证据由关闭帧、termination 回调、UI 等待和独立进程查询共同支持，控制进程清理未冒充 native close。

本次仅只读复核既有原件并新增本报告；未重跑编译、测试或 GUI，未扩大验收、修改权限、源码及旧报告。保留报告声明的物理/最低 OS、故障注入、后台/旋转/并行页面及分发发布证据边界。
