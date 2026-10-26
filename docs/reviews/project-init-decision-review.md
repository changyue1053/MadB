# I01 四项格式修订独立审查

结论：**建议仅将 A–D 四项增量交项目所有者裁定**。未发现阻止这四项决定的剩余问题；当前仍未采用。完整 `madb.project-template/1`、`madb.init-result/1` 及其生产 schema/decoder 不在本次批准范围，I01 整包也未通过。

最终提案 SHA-256：`105405575e121c861f81e56651a4bea76c47e95417f5e7ef032714eb45141e43`。实际来源为 `/Users/changyue/Desktop/MadB/madb`，HEAD `5468d314928f8c5d494768a9b17e3ad6b7d22fcb` 加 19 个已核对文件字节；工作区有既有改动。精确输入、命令方式及结果见同目录 `project-init-decision-review.json`。

## 已修复的审查意见

原第117行将“四项被采用”直接连接“新格式 decoder”，与第7行其余草案字段未批准存在授权歧义。作者仅替换这一段，明确 A–D 只授权相应内部纯规则和到现有 ProjectPlan 的适配；完整两 envelope 的 schema、decoder 和格式向量须其他字段按同一流程定稿。已复核只有第117行变化，旧提案/结果的原字节保存在 `project-init-decision-before-authorization-fix/`。

## 四项结论

| 范围 | 审查结论 |
|---|---|
| A：摘要与错误 | null 表示请求投影未完整，不强制 InvalidInput；当前四类退出映射可复用。取得时 pin 不符与冻结后来源变化分开，后续失败保留已形成摘要。 |
| B：创建与预览 | 普通零退出要求同根、同受检计划的真实无覆盖 CommitNew，以及独立导出的必需检查/open/清理观察；dry-run 使用冻结请求，保持 not-created、空 checks、无下载或启动。 |
| C：引用与 SDK | `input_root` 明确恢复模板原输入根；报告外层 FileRef 加前缀，内部文档原字节和包限定路径不改。嵌套 manifest 的父目录不是包根，无法保留单根布局则拒绝。删除的是模板宿主前提的重复 SDK 列表，不删除 profile.sdk、固定配方的实际 SDK 检查或 composition 中 SDK 资产/运行依赖的包归属。 |
| D：选择 | 四分支清楚；显式 profile、省 targets 使用该 profile 全部 family；显式空/重复拒绝，多匹配由明确默认消歧或要求 profile。不自动补 ABI、裁剪或创造组合。 |

`profile/2` 已采用的决定不重开；认识其格式仍不等于 I01 可以执行组合。ProjectPlan 保持内部纯结构职责，不变为 wire 或提交许可。

## 真实检查及其限制

使用固定 Python 3.14.4，以 `-B` 加载作者脚本并调用真实 `main()`；仅拦截指定 result 路径的写入到内存，逐项精确比较原 result，避免改历史记录。最终 **58 个设计片段通过、19 个来源 pin 匹配**：selection 13、classification 9、success 17、references 12、SDK gate 7。另执行 6 个只读边界观察，涵盖选择重排/重复 family、dry-run open、嵌套 manifest 根；全部符合提案。

这些是受信参数上的设计片段。独立复核确认：调用者将 `required_checks` 换成空数组时 success 片段会接受；不需 SDK 的阶段会绕过 SDK 专项谓词内的 known_recipe 检查。它们与脚本明示范围一致，**不能**被当作生产独立责任派生或未知配方准入证明。真实配方须在外层独立派生要求并拒绝未知身份。没有运行 init、生产 schema、CommitNew、SDK/编译、IDE 或设备。

## 仍须定稿的契约

- 包摘要的字节域与完整包范围、获取锁及重验凭据。保存所选引用闭包不等于能重算未定义的整个模板包摘要；不能擅用 ADR-0018 的 source 域作为模板包算法。
- 首批有限 recipe/renderer/版本比较表，包括精确身份、阶段、descriptor/validator、目标兼容、工具/SDK 输入和独立必需检查集合。`json-values` 的严格 JSON 安全整数范围也须在完整格式中明确。
- 两 envelope 其余字段、共同格式向量、真实文件闭包与路径/链接/并发边界、提交和阶段观察接线。I01 的 IDE、平台配方、已有工程变更/恢复、自动构建及 Swift 两类消费配方验收继续保留。

本审查只写两份 review 文件；未修改产品、catalog、作者提案或历史结果。第二位只读审查者 `/root/l01_contract/l05_evidence` 对双根、SDK、未定包身份和授权边界给出了相同结论。
