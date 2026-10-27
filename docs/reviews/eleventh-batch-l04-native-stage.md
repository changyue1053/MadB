L04 本轮 macOS Native SwiftUI 切片交付记录

本轮实现与测试装配已冻结；真实 UI 及 native 页面生命周期尚未验收通过。不要据此修改旧 C06 报告、登记整个 L04 或宣称 iOS/UIKit/C10/发布完成。

最终 Lab clean revision：`153edd12d76bf416e075a0430f18a2eaeabaa92d`。源码清单 `l04-files-009.json` SHA256 `bdf164237e7ac396b992580608b22e067d3e88367e8e754ebf554c191c40ff7f`；独立签名装配审查 `l04-native-signing-review.md` SHA256 `bbbb709024509d01770904aa98f0855c84170e2580f3e028a98ba6a742fcdd46`。最终26项纯测试通过，属于证据门禁/收尾模型检查，不等于页面运行。

消费的旧 C06 macOS Arm64 SKIE candidate 保持只读，candidate digest `sha256:872d0cbd49bee21d442f24be6536334a7cc8dff3a525839f2a889a7776ac908c`。公开 Observation 源 supplement 的 pin SHA256 `58fe0a1e2634947f87a2311f7a0cdb5efd0eaf98cc8c28cfe16f22e4a623b00b`，来自 product `9a72a641`。未改产品 SDK/native ABI，也没有新增公开包认证。

实现保留普通 owner 与普通订阅，真实 SwiftUI onAppear/onDisappear 驱动三个新页面实例，严格等待页面 drain/模型释放再观察普通订阅继续工作，最后 owner close。预期为六组独立值和32帧、资源全零与正常进程退出；这些真实检查没有删减或放宽，但迄今没有得到运行结果。

四次正式尝试原件全部保留：

- GUI001 / Lab814bbd9：编译通过，240秒控制面超时；业务未启动。发现并修正显式 AppKit delegate 生命周期接线、owned runner 异常收尾。旧 runner 外部精确 SIGTERM 恢复单独记载，未伪称 native close。
- GUI002 / Lab1891b135：重新编译通过，仍240秒控制面超时；样本停在 dyld入口，自动 runner 收尾生效。未推断 App 已运行。
- GUI003 / Lab80118b4：采用实际 Xcode 默认 LLDB scheme，重新编译通过，仍240秒超时。精确 launchd job 为 started suspended / wait for debugger；该 scheme 修正未单独解阻。report SHA256 `a7b85ea9805a95141ee58fbd56a3131e0e85a43180e84bcef01b0e8ff93b7fbb`。
- GUI004 / Lab153edd12：改为固定 Xcode 正常本地 ad-hoc 签名后，重新编译与三个产物严格签名验证通过；App/Runner 获 Xcode 自动开发 entitlement，未手写权限 plist。runner 已实际加载 XCTest 并连接 IDE/daemon，越过前述 dyld 入口。随后67.603秒以 exit65 失败，非240秒 watchdog；错误为 `XCTFuture Code=1000: Timed out while enabling automation mode.`。report SHA256 `791026575f02ce77d277bfa106e058a9c261075e462509802a48f6d4a3569dc8`。有效 xcresult 为 Failed、0 passed、1 runner 初始化失败、0 skipped。runner自行退出，准确归属进程清单为空，业务无receipt/原生帧；仍无 UI/native close 通过证据。

已证明的最新阻塞位置：GUI004 系统日志绑定 runner PID41242 的自动化请求，随后写明 `Writer daemon requires authentication to enable automation mode.`，并由 LocalAuthentication 发起 reason=`Enable UI Automation` 的认证请求。60秒内未得到完成，XCTest初始化失败。精确摘录见 `l04-gui004-diagnostics/authentication-excerpt.json`，摘要 `25ab839137924c67e390511c5656fdaf0436d114e2822f4c5439b2ea0dca7399`；它绑定完整原始日志。

此证据证明本次进入系统自动化认证环节，并非已经观察到可见弹窗、用户拒绝或所有权限问题的唯一因果判定。DeveloperTool preflight 的 denied 也不单独当作主因。没有启用 DevToolsSecurity、修改 TCC、安全设置、输入认证资料或手工恢复 suspended runner；没有借代码直调模拟 UI。

最小后续动作由主协调安排：用户在场时启动全新 GUI005，并由用户本人处理 macOS 的“Enable UI Automation”认证；助手不代输密码或更改系统授权。若没有可见提示或认证仍不完成，应只诊断该认证会话，不预先批量改动权限。仍须原240秒、32帧、真实页面回调、正常退出及独审全部满足后才可授予本 macOS 切片通过；人工认证成功本身不是目标验收。主协调已询问用户，本轮不继续无新事实重跑。

所有失败报告保持原字节；本记录不覆写历史结论。最终 GUI004 独立复核另见 `l04-native-gui004-review.md`。本记录形成后本 agent 停写源码与诊断，仅将独审结论交主协调归档。
