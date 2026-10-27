L05 integrated003 集成后新增原件差异独审（2026-10-27）

结论：未发现阻断或本片声明范围内的新证据缺口。integrated003 原件支持从实际集成 Lab 根重新构建并完成本次三轮 background/foreground、21 帧、9 个同快照 UI 阶段、公开 close 及随后设备清理。可将该运行按既有限定范围归档；本结论不扩大为整 L05 认证。

本轮复用 [formal002 的源码规则审查](/private/tmp/madb-chief-round12/l05-formal002-independent-review.md)，只核新输入根与运行原件。未运行设备、编译或测试，未改实际 Lab、产品或既有报告；只写本报告及 [只读核对记录](/private/tmp/madb-chief-round12/l05-integrated003-independent-checks.json)。后者 SHA-256 为 `cea99e89d2c7988bd25c42e4e9754ad331574392290c0407969f0169d2c54737`，仅记录文件内容与引用一致性检查。

输入及 fresh 构建核对：

- 实际根 `/Users/changyue/Desktop/MadB/madb_lab`：本轮只读 Git 核实 HEAD `f6f1d1bf071616d68cf68441adf9ded85e64b91b`、status 为空、550 个 tracked 文件；与已保存 source-before/source-after 一致。前置计数曾有 shell Python 语法错误的记录保留，独立核得的实际数量为 550。
- 新旧运行的 46 项 `lab_inputs` 完全相同；每项也匹配实际根字节和指定 Git 冻结对象。因此复用 002 的 parser、UI oracle、生命周期及 public-close 语义审查有字节依据。
- [report.json](/private/tmp/madb-chief-round12/l05/integrated003/report.json) 为 13923 字节，SHA-256 `fd1074c945a685c6b92771d1fe4beb1e42b810a10d99b7a1d065381f7837b5b9`。candidate 仍为 `sha256:7fce7dc8b8befbb985694ca08dfecff0b769fef5d0443608b61cc79edf1d921a`；manifest 身份、35 个唯一候选引用及 source export inventory/成员均重新核对。两份观察源与新消费工程复制品字节一致。
- [015 构建原件](/private/tmp/madb-chief-round12/l05/integrated003/commands/015-assemble-release-hap.json) 记录 release assemble、BUILD SUCCESSFUL、`34 executed, 0 up-to-date`；011 签名脚本路径来自上述实际集成 Lab 根，018 安装路径指向本次新 HAP。
- HAP 实际 SHA-256 为 `f3ac54cf18e555d7a23b86411e69cad540de09ee1c8edd1d9362782ff28bfc4d`；source archive 为 `1950e843d8821b1e1fbbf0f88978fc38fd661cdae9640d585a96ca7b194a5a8f`，均与本次 build/summary 相符且不同于 002。archive 的 24 个普通源文件与保存引用及消费目录逐字节匹配；相对 002 仅 ProbeConfig nonce、obfuscation-rules 的本次输出路径发生预期变化。
- HAP 10 个成员引用全部匹配、无重复，release profile 与 002 相同。public HAR 原件与本次 vendor 字节相同，15 个安装成员相同；两 native 库逐字节等于 HAR 和 candidate，不因 fresh 构建替换。
- CLI/JBR before-after inventory 逐字节一致，也与 002 同一冻结闭包一致；规范 entries 摘要、数量/大小及 candidate toolchain 绑定重新核对（101927/357 项）。tools.json 的工具输入与 closure 引用仍匹配；native inspection 和原 readelf NEEDED 输出一致。
- [125 项原件索引](/private/tmp/madb-chief-round12/l05-integrated003-evidence.json) SHA-256 `040aea1d6b47afd131b14ebca0298321f0dbc4e188599f8b6227d75171a6815f`；全部列项均重新核对实际大小与摘要。未用该索引替代对原始日志、UI 和命令内容的审查。

新运行证据核对：88 个 command 序号连续，全部 exit=0、无 timeout，所有 hdc 命令绑定指定目标 `127.0.0.1:5557`。目标参数原始输出仍为 API23、Release、arm64-v8a、OpenHarmony-6.1.0.115 emulator。

13 次 raw hilog 与 13 个 observation 一一相符；全部观察 PID 为 `29366`，nonce 为 `l05-bg-2a658bd5a7da438f8b565d2b47d0b130`。独立复用 002 已审 literal 预期检查 [084 最终原始日志](/private/tmp/madb-chief-round12/l05/integrated003/commands/084-probe-hilog.json)：21 个严格顺序/整数值 CASE，随后恰有一个精确 COMPLETE；同一 PID/TID/tag/nonce 绑定，无 FAILED、重复或遗漏。六个恢复帧的 HiTrace 包络均满足已审起点、规范小写十六进制、60/26/26 位和精确分隔符边界。各早期 raw poll 都是最终日志的前缀，缺最终证据时保留 incomplete；新 report/UI/final observation 内容相同。

| 循环 | Background 点击 | 恢复前后台交付 poll | aa 恢复 | 恢复后 poll |
| --- | --- | --- | --- | --- |
| 1 | 032 | 034：`background-published-1 [1,2,2,101]` | 035 | 037：foreground/reattach |
| 2 | 047 | 049：`background-published-2 [2,12,4,102]` | 050 | 052：foreground/reattach |
| 3 | 062 | 064：`background-published-3 [3,22,6,103]` | 065 | 067：foreground/reattach |

每轮恢复前 poll 均已有该轮后台交付而尚无对应 foreground；aa 对本 bundle/EntryAbility 的恢复均明确成功。后台交付时间 17:44:10.096、12.033、13.963 早于对应 foreground 10.218、12.126、14.030。结合字节未变的 moveAbilityToBackground 与 OS onBackground/onForeground 回调路径，这些是实际生命周期切换证据。

九份保存 layout 与 raw ui-read JSON 逐个相等；每份摘要同时绑定对应 assert-phase。READY_0/PUBLISHED_0 至 READY_3/PUBLISHED_3、FINISHED 的 phase/page-id/value/ordinary 四字段均在同一张布局中同时满足独立预期，节点唯一；恢复后 value 均 EMPTY、ordinary 为 2/4/6，最终为 FOREGROUND-3/7。八次按钮点击都匹配此前布局的目标按钮中心及实际 uiInput 命令；dump/read/remove 路径均绑定本次 nonce。

公开 close 与清理严格区分：新原件 079 已于 17:44:15.830 记录 `owner-closed [1,3,4,7,1]` 与 COMPLETE，080–082 捕获 FINISHED，同样的完整日志在 084 再次读取。复用未变源码可知该 marker 位于 join background/resume attempts、核七次交付及依次 await model/ordinary Subscription/Engine/Runtime close 之后；它发生在 085 强停前，非杀进程推断。085 明确停止本 bundle，086 PID 为空，087 确认卸载，088 无包信息；device-before/after 与原命令一致。没有 hdc server 停止或其他 bundle 清理记录。

因此新 report 的 `compile/target_execution/native/ui=passed`、`runtime_close=confirmed`、`cleanup=passed` 有完整原件支持。仍保留 C ordinary、开发 source extension、单页/shared owner、三短周期、单 emulator、社区签名等既有限制；排水后 publish 不证明 queued-late-callback/close 交错，没有故障注入/重试、严格 native 计数、长时后台或进程死亡恢复等扩大证据。`certifies_c07=false`、`certifies_l05=false` 不变；001/002 原报告不变。

差异独审完成，停止写入。本轮 L05 证据审查收束。
