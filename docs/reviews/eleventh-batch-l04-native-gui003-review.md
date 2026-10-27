L04 GUI003 独立只读复核，2026-10-27。

结论：GUI003 真实失败；采用 Xcode 默认 LLDB debugger/launcher 没有解除本次启动阻塞。编译通过不能授予 UI/native 通过，正常业务退出和 native close 仍未确认。本次仅核既有证据，没有运行测试、再次排障、启动 GUI、resume 进程或修改安全设置，旧报告未改。

原报告 `/private/tmp/madb-chief-round11/l04-formal-gui-003/report.json` SHA256 实核为 `a7b85ea9805a95141ee58fbd56a3131e0e85a43180e84bcef01b0e8ff93b7fbb`，56890字节；clean revision `80118b4fb1ef617d2d789db53baaa2aea8dad512`，provisional=false。报告状态为 `failed / compile=passed / target=failed / owner_close=not_confirmed`。22个命令日志引用 SHA256/size 全部匹配；78项 Lab 输入绑定该 Git commit，87项消费输入、3项 app 文件和消费配置均与记录一致。

`logs/012.json` 的实际 build-for-testing exit0、`TEST BUILD SUCCEEDED`。`logs/014.json` 的 test-without-building 在原240秒 watchdog 超时后 exit -9，stdout止于 Writing result bundle，没有 TestSuite；`tests.xcresult/Info.plist`、业务 process receipt 和 native帧文件均不存在。因此没有有效 XCTest 完成摘要、32帧或业务生命周期成功证据。

既有诊断精确绑定 owned runner PID24578：1秒 sample 的883次样本为 `_dyld_start + 0`，进程 footprint 为96K；sample文件本身1043字节，SHA256 `396005f7d3bb6919094bc1d6d0da09e8cad16c61477e7da569838e5eed1ce644`。`l04-gui003-diagnostics/owned-launch-job.json`（SHA256 `5b5c4f2b07c3885f1102ef6060f6c36a2c5b68fc5a50723e4ea6f5abc765dfa8`）中 launchctl job 的 program 为该次精确 runner路径、pid=24578，并直接记录 `started suspended = 1`、`wait for debugger`。这证明被观察时的启动挂起状态，不说明 debugger未继续执行的唯一原因，也不说明业务 app/native 已启动。

自动收尾证据成立：`logs/015–016` 精确识别并复核该 runner；SIGTERM 后的 `017–019` PID查询均 exit1、stdout空，`020` exact runner inventory 为空，报告记 forced=true/exited。随后 `021` 业务 app精确路径枚举为空，业务 cleanup仍为 `not_observed / forced=false / recovered=[]`。这是 owned控制进程恢复，不能改称正常应用退出或 native close。

复核完成并停止；GUI003失败及 UI/native 验收缺口保留，不扩大任务范围。
