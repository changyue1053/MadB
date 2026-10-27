# E04 Lab 日志与执行事实最终增量独审

结论：冻结的四文件修复通过本次独审，原“实际运行被写成 not-run”的 P2 在该源码范围内已修复。可由根协调者冻结并集成新 Lab revision，再执行正式002。本结论只覆盖私有日志/执行事实/报告接线，不代表正式 Dart 消费或 E04 整包通过；正式001及首审原件保持不变。

## 精确候选

基线 `189e1517feeb1be811bb030aaadb1dedc48a4c63`。作者 manifest SHA-256 `d0f009426b67258c65a2eac543ca1c80e62fe2ec31930adda29bff459ea07afe`，patch SHA-256 `b022c97cabbb46d921916acc79ec0ba7e64c11940d04cddbb8f53b5ec2542b79`。

| 文件 | 已复核 SHA-256 |
|---|---|
| harness/e04_dart_execution.py | a980e0294bbd9d0ddb074212f25eb581cd39ac72591f2acf179bd4fa4e6773c5 |
| harness/e04_dart_reports.py | e9c52e86204e52065849577500c932d44ffc189e4a8c45c884b1372439d8bdbd |
| harness/tests/test_e04_dart.py | fbed302fa6ade2b8489fded889f05618403ade1bfd5786b799d1cb73d034c1ad |
| scripts/verify_e04_dart.py | a417ebf8e0b2824e0d6920f4ffaf22a6f26dad74fe5f189e5570474babc47e39 |

四文件实际字节和大小在检查后仍与 manifest 相符，before SHA 与基线 Git 原件一致。另对照了11个保护文件：严格 decoder、候选准入、独立期望、固定 consumer 与模板、测试入口和五 case；它们均与基线逐字相同。未修改公共 schema、SDK、测试配方及业务预期。

## 实现边界复核

1. Reports.begin 每次建立独立 object token；Commands.run 对 token 单次使用，在重复 token 时于 Popen 前拒绝。Reports 仅接纳当前 token identity、精确 argv 和一次事实；上一调用即使 argv 相同也不能借用。begin 仅选定计划，没有进程事实。
2. ExecutionFact 是 frozen dataclass，成员采用不可变元组，记录实际 Popen 的 PID/argv/单次耗时/退出/清理及可用日志引用。仅 Popen 成功后通过回调登记；spawn 前错误不登记。
3. 写日志及文件引用产生错误时，finally 仍登记已发生的进程事实，log=None 照实保留；观察回调自身二次异常作为原错误 note，未覆盖原日志 IO 错误。此内部回调不触及 native SDK 回调或外部日志 sink。
4. Reports.complete 仍严格读取并核对日志字节、argv、PID、duration、exit、reaped、清理字段及其类型，再执行既有业务输出检查。ExecutionFact 不授予业务成功；坏 JSON、错 PID、错 cleanup 等继续拒绝。
5. 错误报告直接使用已登记事实计算证据层级和耗时，不再次解析坏日志。新私有 `execution-facts.json` 作为既有 report/1 File attachment 保存实际元事实；日志引用只有在字节/大小仍吻合时才保留，漂移或缺失记 null，不伪造有效附件。错误用 `error / unavailable / target-run`；原始 exit 和清理事实在附件中，符合 unavailable 的公共 exit_code=null 规则。
6. spawn 前失败仍为 `error / not-run / static`；错误时 cleanup 保守记 failed，具体已回收/排空事实保存在附件中，不冒充完整 case 资源检查通过。后续未开始 case 仍 blocked。
7. 运行时排水与回收仍各0.25秒；私有日志改整数毫秒250。严格整数 JSON 规则和既有60秒业务 step预算不变。

runner 不再在异常路径猜测 `commands.logs[-1]` 属于当前调用，改为显式连接本次执行 token。全部成功路径仍要求固定16次 JIT/AOT调用、实际匹配的日志与观察、输入最终完整性和清理后才可产生 passed。

## 实际检查

独立命令：`/opt/homebrew/opt/python@3.14/bin/python3.14 -B -m unittest discover -s harness/tests -p test_e04_dart.py -v`，在 dev Lab 执行，51项通过。日志 `e04-lab-log-unit-independent-review-final-tests.log`。包含原生命周期/超时/输出预算检查，以及新token、坏日志、日志写失败、实际字段不匹配、日志漂移和二次异常保护回归。这些是 Python 观察/报告模型与进程检查，不是 Dart 认证。

另运行三个独立真实 Python 进程探针，逐项核对错误报告附件字节：

- 坏小数日志：PID22391实际退出0；strict解析拒绝，业务观察0；public error/unavailable/target-run，耗时来自同次事实，仍有原始坏日志及事实附件。
- 日志写失败：PID22395实际退出0；事实含真实PID/argv/耗时/reaped，log=null；public error/unavailable/target-run，未接纳业务观察。
- 启动前失败：先真实运行相同argv作为前一调用，再移除仅本fixture的可执行，当前Popen失败；当前事实为空，public error/not-run/static，无借用前次PID，后续case blocked。

原件 `e04-lab-log-unit-independent-review-final-probes.json`。临时fixture由各自测试持有并清理；本审不写产品/Lab源码，不导入MadB私有实现。

## 历史与后续条件

首审 SHA `6a9d934dff0faa761364449ac41cb8ecdd60198ec4a3b88842d54ac652953b02` 保持。正式001 local report SHA `9155a7b9df26a1f9c04536c2fd74f84f4b53821282fbea6f7ddde3d28812734c`、public report SHA `f10bdeb8b6035cbd951058519a95b40870dd8f147ca5897a15e416905e41abc0` 均未改；其整体error及首case错误not-run历史字段由本审解释，不能倒写通过。其七命令局部退出0只证明已发生的实际步骤，尚不构成完整16次业务接受。

根可在收到本审最终停写通知后冻结四文件并重建Lab来源身份。正式002须绑定同一公开候选与实际新Lab commit，重新执行既定 JIT/AOT、参数拒绝、close、isolate及包拒绝集合并核输入前后和清理；本审没有执行或预判其结果。

证据绑定：`e04-lab-log-unit-independent-review-final-binding.json`。本次报告与绑定写完后停止所有审查文件写入，供根统一冻结。
