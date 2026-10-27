L04 macOS Native SwiftUI 切片：GUI006 集成后验证完成

限定结论：本次 macOS Arm64 C06 Native → 既有普通 Swift owner/订阅 → 公开 MadBObservation → 真实 SwiftUI 页面切片已通过真实运行和独立复核。不是整个 L04、iOS/UIKit、全部 UI 场景、C06 新认证或 C10 发布通过。

最终执行根为实际 Lab `/Users/changyue/Desktop/MadB/madb_lab`，clean revision `7a252092a0b50850c961f7d617facce20fbc57a2`。GUI006 是该集成版本的一次正式重建与目标运行；没有在通过后重复目标。

原公开 C06 candidate 仍为 `sha256:872d0cbd49bee21d442f24be6536334a7cc8dff3a525839f2a889a7776ac908c`，macOS Arm64 SKIE。Observation 公开源码 supplement pin 仍为 `58fe0a1e2634947f87a2311f7a0cdb5efd0eaf98cc8c28cfe16f22e4a623b00b`；未改 SDK/native ABI 或旧候选、历史报告。实际环境为 macOS26.6.2/25G83，固定 Xcode27.0/27A266a；Release、本地 ad-hoc 测试签名，非分发签名。

GUI005 保留真实失败：系统自动化初始化已越过，真实 app/native 产生 bootstrap-verified、owner-started 两帧；第一项 Mac StaticText label 断言失败，XCTest teardown终止应用。另有/tmp与/private/tmp同目标别名导致receipt字符串校验报错，独立ps确认无遗留应用。该报告 SHA256 `01c1220f1f59e6ab4cb94b85856e9d9262553a6104906477ee5d578a82c1a75d`，未回写。三文件修复只将 macOS文字改为value精确等值及pageID强类型读取，receipt要求绝对路径、strict resolve后仍精确同一可执行目标，保留nonce/PID/运行image确认；错误路径/相同字节另一文件等拒绝，专属纯测28项通过。原独立期待值、32帧、点击流程、等待时间与原生关闭要求保持。

GUI006 正式报告 `gui006/report.json` SHA256 `d293ea897b167bd6db801e744804422d51aeabad885c817390caa12d4947dc04`：编译通过，XCTest一项通过、零失败、零跳过；实际测试58.062秒，未触发240秒watchdog。观察恰32帧，业务PID60519、nonce/候选身份/mainThread持续一致。

三次页面创建为不同UUID；真实 onAppear/onDisappear 接线驱动订阅，refresh及重复启动未增加工厂数；每次严格依次出现 disappeared → drained → model-release-observed，随后普通共享订阅继续接受新值。六组实际值包括ASCII、Unicode、NUL、空字符串及字节边界，页面与普通订阅双方均与独立UITest字面量一致。

最后严格 ordinary-drained → owner-closed → application-terminated；activeSubscriptions归零、其余原生资源计数为零、closed=true。XCTest明确点Close owner、检查closed=1、点Quit、观察Not Running，未走emergency terminate。receipt与最终帧PID一致，cleanup state=exited/forced=false，XCTest runner准确归属清单为空。任务join根据源码既有await顺序及正常关闭控制流审查，实际帧没有也未声称包含ownerTasksJoined字段。

独立复核 `gui006-independent-review.md` SHA256 `9fa26ea90082e2221e9cd3ad1f99f1792f693a137cc969751845bfd54880abbd`：29条原始命令、78项commit输入、87项消费输入、4项app文件、9项工具/签名输入全核；候选、公开源与工具前后守恒。另行逐帧复核覆盖值、资源和时序。审查报告落盘曾短暂遇模型容量错误，恢复后由原审查者本人完成；未因此重跑目标。

四张实际XCTest附件在 `gui006-attachments/`，其中Unicode页为 `FFB767B6-F1D5-4EAB-A901-6293C32002F2.png`，最终关闭为 `5B03C5F2-0D57-4A53-8376-BC94B1CF6EC2.png`；均已读图确认。附件包含桌面背景，只保留本地验证证据，不应随产品源码公开推送；未编辑图像。

本轮未修改系统安全设置、TCC或DevToolsSecurity，未代输认证资料。GUI001–004归档与GUI005原失败保持；当前补充记录只新增GUI006实际通过证据。状态由主协调在产品catalog维护。本agent至此停写源码/证据，不再执行目标；可供持久归档与阶段交付。
