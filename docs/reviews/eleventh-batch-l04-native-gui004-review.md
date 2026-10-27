L04 GUI004 最终独立只读复核，2026-10-27。

结论：本轮目标运行仍失败。正常本地 ad-hoc 构建/签名通过，XCTest runner 已加载测试 bundle 并与 testmanagerd 建立会话；失败现已定位到启用 UI Automation 的认证阶段。没有业务 app receipt、32帧或 native close 证据，不能标记 UI/native 或整包 L04 通过。本审查完成后停止，不再运行 GUI、测试、排障或修改权限。

`l04-formal-gui-004/report.json` 实核 SHA256 `791026575f02ce77d277bfa106e058a9c261075e462509802a48f6d4a3569dc8`，61004字节；clean revision `153edd12d76bf416e075a0430f18a2eaeabaa92d`、provisional=false。27个命令引用均匹配；78项 Lab输入绑定该commit，87项消费输入、4项app文件和消费配置均匹配记录。状态准确为 `failed / compile=passed / target=failed / owner_close=not_confirmed`。

实际 build-for-testing exit0；app、Runner、test bundle 的 strict signature verification 均通过，实际 `Signature=adhoc / TeamIdentifier=not set`，App/Runner有get-task-allow。`logs/023.json` 的测试命令却为exit65、timeout=false，Xcode记录67.603秒；这是XCTest自身初始化失败，不是原240秒watchdog。错误为 `com.apple.dt.XCTest.XCTFuture Code=1000: Timed out while enabling automation mode.`

有效诊断证据：

- `l04-gui004-diagnostics/001.json`（SHA256 `71e5220f97ab3af5e0db575e711345e9f480494fadd6ca30dd7f9bafe6fb07cd`）中的xcresult摘要为 Failed，0 passed、1 runner初始化failure、0 skipped；不是业务用例已执行后失败。
- `000.json`（SHA256 `baf9eee7bb6de043fa34fc77f03411b420aa5fecd43d7031978631cc864819b3`）在同一会话中记录PID41242于03:36:28.214请求automation mode，03:36:28.230明确出现 `Writer daemon requires authentication to enable automation mode.`，随后03:36:28.232调用 LocalAuthentication `evaluatePolicy:2`，reason为 `Enable UI Automation`。当前证据窗未见认证/启用成功；约60秒后对应请求超时。
- 导出Session日志（SHA256 `d2b346ef4bb930ad18e6ad3bd9d3e623d64ef0950fe9f6f858f8954a0c3252ad`）记录测试bundle加载完成和60秒automation future等待/超时。`002-export.json`导出命令exit0，原始diagnostics保留。

这些证据已明确本次需要本机认证；没有观测到实际认证弹窗或用户完成认证，也不据此认定某项具体TCC设置、DeveloperTool preflight或其它权限拒绝是唯一原因。关联窗中无关进程的TCC事件不作为本任务证据，未通过改权限或手动resume制造成功。

清理记录正确：Runner已在清理检查前退出，两次精确runner inventory均为空；随后业务app精确路径inventory也为空。runner cleanup为exited且processes=[]，没有强杀；业务cleanup仍为not_observed、forced=false、recovered=[]。这是无遗留进程的清理观察，不能替代正常业务退出或native关闭。

旧运行与审查报告不改写。可集成本轮实现和已验证构建/签名能力，但目标运行仍失败；在本机完成相应认证并另获真实UI/native证据之前，验收缺口保持。本轮收束。
