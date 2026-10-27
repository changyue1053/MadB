# GUI005 失败与三文件修复独立复核

结论：本次窄修无新的具体阻断，可冻结后进入已授权的 GUI006。该结论仅覆盖三文件静态差异及既有证据；GUI005 仍为 target failed，不能称为 L04、32 帧生命周期或正常 native close 通过。

复核对象为 Lab `/private/tmp/madb-chief-round10/lab` 相对 `153edd12d76bf416e075a0430f18a2eaeabaa92d` 的差异。实际仅 `LifecycleUITests.swift`、`scripts/verify_l04_native_ui.py`、`scripts/test_l04_native_ui.py` 三文件变化；`gui005-fix-files.json` 全部九项大小与 SHA256 均匹配，清单 SHA256 `7d358d03fe91cdecb7453ad6c4a82333ca806a20cf3562790efafe88b0945f42`。业务 App、NativeOwner、32 帧 harness、工程、签名、候选与公共 SDK 输入规则没有变化。

GUI005 原始 `gui005/report.json` SHA256 `01c1220f1f59e6ab4cb94b85856e9d9262553a6104906477ee5d578a82c1a75d` 匹配，绑定上述 clean revision，compile passed / target failed / owner_close not_confirmed；26 条命令引用的大小与 SHA256 均匹配。logs/023 为真实 XCTest exit 65；测试 suite 与业务 App PID 49653 已启动，首个 summary 的 label 等值等待超时，测试约 17.537 秒失败。原始日志只有 `bootstrap-verified`、`owner-started` 两帧，后一帧 native activeSubscriptions=1；没有页面循环、完整值 oracle、drain、release 或 owner-close 证据。

AX 观察 `gui005-ax-observation.json` SHA256 `ccf3f39f065548d1831b1542288d4b8954241ea1861fe493aeb33a376f383a2a` 与其原始 Session 日志引用匹配，三段摘录与指定原文行一致（仅末尾换行差异）。实际树显示 summary 为 `StaticText ... value: active=1 pages=0 d...`。这证明属性选择问题的修复有实际依据；截断文本不证明完整预期值正确。Swift 差异只有三行：predicate 改为 `value == %@`，测试声明 `throws`，pageID 改为 `try XCTUnwrap(...value as? String)`。全部原期待字面量、UUID 检查、15 秒等待、动作顺序及真实按钮操作保持；无 label fallback、contains 或放宽等值。

路径诊断 `gui005-path-diagnosis.json` SHA256 `7aa8f55e8cfcad51a759759630607048a4faf5631b60c6239c61ffbd333e6737` 匹配。原 receipt 是 `/tmp/.../MadBL04NativeUI`，实际 strict resolve 后恰为本次 `/private/tmp/.../MadBL04NativeUI`；旧代码以字符串比较拒绝了同一文件，原 report 的 cleanup_error 保留。修复先要求字符串绝对路径，再 `resolve(strict=True)`，只接受本次指定 executable 的规范路径且为文件；nonce、正整数 PID、receipt 文件限制以及每次 signal 前的运行 image 核验均保留。新增测试覆盖正确 alias，以及相对、不存在、同字节其他文件、错误 alias 和非字符串拒绝；没有以内容相同替代文件归属。

XCTest 原始 teardown 明确 terminate 业务 App 49653；`gui005-diagnostics/post-test-application.json` 的随后精确 PID 查询 exit 1、空输出只证明查询时该进程已不在，不能当作正常关闭证据。XCTest controller 清理与业务 native close 仍分别记录。

协调者报告 28 项纯测试与 Swift parse 通过；本独审未重跑测试、编译或 GUI，未改源码、原 GUI005 证据或旧 round11 归档。最终三文件的实际编译和完整 GUI 行为仍待新冻结输入下的 GUI006。
