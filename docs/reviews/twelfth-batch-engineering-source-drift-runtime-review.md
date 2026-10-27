# 已捕获源增长修复后的真实工程独审

结论：本轮风险闭环无阻断。真实冷启动成功；捕获后源增长使发布以 Conflict／exit 4 拒绝；恢复源码后的正常重试成功，新消费者输出精确字节 `7:x\n`。此结论限于这三步，未将其扩展为整个 I01/I02/I03 验收。

统一产品为 clean `bf478767273069c47798bc04140185c671d93957`，工具 `/private/tmp/madb-chief-round12/e04-formal-producer-001/images/madb`，61,591,512 B、mode 0500、SHA-256 `447b781651e70dc27a9bbcedd554dffb52b8d3dbe3fc994cebc5a5fcfd8b8edb`。生产者 receipt SHA 为 `44f16aef3058a1d5945af65bdd9e6ad044ddb3566eef20075ab7cb45350c7770`。独立核对构建前后源清单相等、实际 checkout clean、1550 个源文件及源归档的字节/大小/模式；新域 `engineering-replay-e2e-001/freeze.json` 绑定的 2715 个文件均匹配，域内文件集合无缺漏。首个 tar 的模式失败原件保留，正式核对使用显式 umask 0022 的 `source-bf478767-002.tar`。

逐步原件支持以下事实：

- 冷启动实际退出 0，structure、native-generate、c-link、ordinary-run 四项通过。保存的 Init 回执与原 stdout 字节相同；普通消费者 stdout 为 `7:x\n`、stderr 为空，进程记录为 exit 0 且 leader reaped。
- 增长步骤在实际 attempt 的捕获源码与原文一致，且 `native/contract.json`、`native/build-execution.json` 已存在后，追加注释。三个屏障文件的实际摘要与记录相符。这是捕获/prepare 文件屏障，不是 CAS 内部钩子。原报告前八阶段 passed、publish failed，诊断为 STORE003／`captured input identity changed`，真实 exit 与报告 exit 均为 4，published=false、result=null；previous 精确指向冷构建。
- 源码恢复后的重试真实退出 0、published=true；完整输入与冷构建相同，snapshot digest 相同，previous 仍指向冷构建。最终 current 为 generation 2，指向新结果；current 的 record 摘要、结果投影及所有输出均核合。冷构建和增长失败的 attempt 仍在，新消费者实际退出 0，stdout 精确为 `7:x\n`。

旧 current 的原字节保全由运行 oracle 在重试之前执行的即时相等断言、后续完成记录支撑。独审另核冷批次 record、增长前 current 摘要、失败及重试的 previous 链和最终 generation 2；重试已替换 current，未将事后读取表述为对中间时点原字节的直接观察。顶层 `invocation-receipt.json` 明确为实际调用和最终 exit 0 的事后转录，空 combined log 原件保留，没有伪称同步 shell 回执；执行脚本摘要一致且调用未启用 Python `-O`。

独立按归档中的 schema 验证 4 份项目报告，并逐项核对其 21 个输入、38 个输出、267 个 log 引用，以及 Init 的 6 个 log 引用、保存回执和原 stdout。36 个项目进程记录均 exit 0、leader reaped、无 signal_failed 或待清理标记；本轮预期失败发生在发布重验。绑定 JSON 保存 383 个直接引用文件的独立摘要，并绑定冻结域与源清单。

三条顶层命令均明确使用 `/usr/bin/env -i PATH=`；冷 Init 带 `--offline`，两个 wrapper 构建带 `--frozen`，工具生产构建带 `--locked --offline`。这些环境和参数事实不构成操作系统网络隔离证明。旧 0007 的 PLAN001／exit 2 原件摘要仍与先前独审绑定相同，未覆盖或改写。本次独审没有重跑目标、编译或修改产品，只读取原件、校验 schema/摘要并写本报告及 binding。

完整绑定：[engineering-source-drift-runtime-independent-review-binding.json](/private/tmp/madb-chief-round12/engineering-source-drift-runtime-independent-review-binding.json)。
