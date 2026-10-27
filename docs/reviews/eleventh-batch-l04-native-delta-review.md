L04 GUI001 后入口与进程收尾窄修独立 delta 审查，2026-10-27。

结论：最终清单006静态审查无未解决阻断；明确入口和 owned XCTest runner 收尾已在代码层修正。审查期间发现的清理先后竞态也已修复并通过对应 mock 交错测试。此结论不等于实际构建、AppKit 启动、GUI/native 生命周期或异常进程清理运行通过；新代码仍须在最终 clean revision 上重新构建并取得真实目标证据。GUI001 继续保持真实失败，不从本次代码修复推定其超时唯一原因。

范围仅为相对冻结 `814bbd99681aff9b38a861ec96ba64e3ff434f8e` / 清单004的四文件：`consumers/swift/l04_native_ui/App.swift`、同目录 `README.md`、`scripts/verify_l04_native_ui.py`、`scripts/test_l04_native_ui.py`。没有修改公开 SDK、ABI、普通订阅、native 关闭协议、32帧行为 oracle、240秒目标超时或任何 TCC/DeveloperMode 设置。本审查没有启动 GUI 或目标 app，只读源码与原件，并运行专属 Python 测试。

身份核对：清单005 SHA256 `9bf957b7a072a1ac1be722c37d4f127ab21747ede391b0e700f6d15511be792e`；最终 `/private/tmp/madb-chief-round11/l04-files-006.json` SHA256 `954242d49222282c4d668457521f117e3f2d753bdf69eedf9948a3cd27c7b6e5`。实际九项文件 SHA256/size 全部与006一致。005→006仅修改 runner 与对应 Python 测试；显式 App 入口保持005内容。两版清单不混同于已有编译或 GUI 证据。

复核结论与位置：

- `App.swift:5–13`：独立 `NativeAppMain.main` 显式构造 delegate，先写独占并同步的 process receipt，再获取 `NSApplication.shared`、设置 `application.delegate`，通过 `withExtendedLifetime(delegate)` 保持其贯穿 `application.run()`。不再依赖无 nib/无显式登记的 `NSApplicationDelegate.main` 装配。native owner 仍在 `applicationDidFinishLaunching` 中建立；receipt 不被当作 native 初始化或关闭证据。GUI001 的 P1 入口装配缺陷在源码层闭合，尚待实际编译和运行确认。
- `verify_l04_native_ui.py:36–50,146–184`：进程枚举只选择本次全新输出目录中业务 app / XCTest runner 的精确规范可执行路径，排除同名、其他目录和非绝对路径；日志只保留匹配项，不写入无关进程 stdout。对将发送信号的 PID 再查询完整 comm 并精确核对；未知或变化身份抛错，不 signal，不称已退出。终止为有界 SIGTERM→必要时 SIGKILL，失败进入错误。这里的路径匹配没有改成名称或前缀匹配。
- `verify_l04_native_ui.py:187–219`：业务 app 有 receipt 时仍核 nonce/PID/executablePath；无 receipt 时只回收本次精确 app image，并保留 `not_observed`，不授予正常完成。XCTest runner 使用同次 DerivedData 的精确路径独立收尾，随后再枚举确认不存在，并以 `xctest_runner_cleanup` 单列；控制进程强制清理不是 native close。
- 已解决的新增 P2 时序问题：005 原 `finally` 先检查应用、后停止仍可启动应用的测试 runner，存在应用在第一次扫描后才出现的窗口。006 `cleanup_execution`（222–234行）先停止并确认测试控制进程，再最终检查应用；两个独立 try 使任一域出错仍尝试另一域。对应测试在 runner 停止期间才“启动”应用，核最终应用收尾仍执行；另测 controller 失败也不跳过应用清理。
- `verify_l04_native_ui.py:320–330`：两种收尾错误都使整体失败；真实成功仍需业务 app 已退出且未被强制终止，并且 receipt PID 与 native 帧 PID 一致。对测试控制进程的独立强制恢复没有伪装成业务 app 正常退出，也没有降低 XCTest 或32帧条件。

审查者实际执行 `python3 -B scripts/test_l04_native_ui.py`：22项通过。新增的精确路径排除、无业务 receipt 的 XCTest runner 清理、控制进程/应用交错和异常不跳过另一域，均为端口 mock 测试，不是实际 GUI、系统进程或 native 关闭运行。未扩大到无关测试矩阵；最终 Swift 入口尚未在本次审查中编译。

旧原件完整性再次核对：

- 静态审查 `l04-native-independent-review.md`：`a0cf46be938d7dfe15eeb828f3a0dc41a27a250b39fcc7fab719bc4c5125f040`。
- GUI001 运行复核 `l04-native-runtime-review.md`：`c15829cea65889dbc2de2633f0a66cb86196c8d3132bb70891e0e86f89b0eec8`。
- GUI001 原始 `report.json`：`c2c38e3066df696c78d9115d705e99c872bc1b5987e3c24a1e25684833166ade`，保持 `failed / compile=passed / target=failed / owner_close=not_confirmed`。
- GUI001 外部恢复记录：`97ddc5aea7335c71abb847d76d6afd791e3c1ee00967e545a7f95f771187fa42`。

后续运行须使用新目录和最终冻结身份，重新 build-for-testing；若仍停在 XCTest 启动阶段，继续保留实际失败与精确 owned PID 的诊断，不能以本次两个代码缺陷修复推定环境因果，更不能通过改期望、延长超时或改变系统权限制造通过。iOS/UIKit、整包 L04、C06/C10 和发布资格均不在本 delta 结论中。
