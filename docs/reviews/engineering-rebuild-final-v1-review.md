# 工程重复构建决定稿 v1：冻结复审

审查对象：manifest `107fc442f7ee4c2b0420bb4475597579b76a78dd7db46d93cb9d21f61c92b370`，提案 `a362ca7c7f5bed70cd50e2d983eb7abb42ecc51eb920e046f83c1b78cd46f5ff`。只读核对了全部 28 个冻结文件和 2 份历史报告的 SHA-256/size，全部相符。历史报告没有修改；本结论不反写旧稿。

## 结论

**仅剩一处必须消除的直接文字矛盾；除此之外，本次未发现阻止进入所有者决定的架构或新增格式问题。** 矛盾为提案第 111 行 `result` 仍写“仅 build 且 publish 通过时非 null”，但最终 schema 的发布尾窗条件和 `result-published-uncertain-positive.json` 均明确：`published=true, publication_durability=uncertain, publish.status=error` 时 result 仍保留。第 92、115、128 行也支持保留真实发布事实。

最小修订只需将该处改为：**“仅 build 且 published=true 时非 null，包括 publication_durability=uncertain 的发布尾窗。”** 不变更已冻结 schema、向量或运行机制；保存 v1 后形成文本 v2 与新 manifest，再核对该单处差异即可，不需重跑字节未变的 17 项结构与 10 项摘要检查。

## 既往问题的闭合

- 发布取消：schema 已禁止 published=true 搭配 130；build/install-tool 输出完整 9/3 项顺序，发布 confirmed 为 passed、uncertain 为 error；未知发布是 null/uncertain/exit3。schema 仅做结构与简单状态约束，真实阶段、清理、CAS、损坏旧结果和未知发布根定位仍按提案由 app/Host 校验，不能从任意 JSON 的结构通过获得执行权限。
- 本机路径：第 104 行已明确本次入口只输出本机绝对路径和 redacted=false，不再声称具备脱敏导出；没有改变旧 InitResult 的脱敏能力。
- 预算：第 130–132 行保留现行 1 MiB 解码上限，输入/输出/附件索引/诊断分别有 128 KiB 元数据额度并预留尾窗空间，超限在发布前拒绝；不需要扩大旧格式预算。
- 摘要：第 120–124 行已给出三个域与精确投影、路径角色、排序与重复拒绝；副本 path 不进入输入投影，descriptor 在原始文件集中必须同字节。10 项设计向量包含投影、规范文本、preimage 和既知 SHA-256，明确区分作者/目录存在/工具/输出变化及集合重排。该范围足以定格式，不替代真实候选证据。
- 来源与 Wrapper：来源文件固定 `.madb/project-origin.json`，有不可变基准与 managed_roots 列表，并清楚限制为本机一致性检查；原型在工具执行前先核对 origin→descriptor 字节，再核对工具镜像。四项 fixture 仅证明已声明的假工具启动/拒绝观察。app 仍只定义端口/领域约束，CLI 复用既有 coordinator；无第二套 DAG 或第四个 C04 Layer。
- install-tool 尾窗：第 112、132 行保留已经逐字确认的镜像事实，即使结果保存失败仍非零返回且不删除正确镜像；普通重试复核匹配镜像，错误已有字节不覆盖。

## 已有检查证据与本审查的边界

读取的 `final-check-v1.json` 绑定 22 个输入文件，逐一 SHA-256/size 核对相符；其中 17 项结构观察的 actual 与 expected 相符，10 项摘要观察含固定 preimage 的 Python/OpenSSL 一致结果。`launcher-probe-002/observation.json` 明确保存 4 项 fixture，工具与 launcher 身份已绑定。这些是实施线已经执行并保存的观察，本审查没有重新执行 validator、OpenSSL、Wrapper、工程构建、IDE 或运行测试。

结构 positive 使用假制品身份，其名称与 scope 已限定结构；完整公开候选、输出库存排序/唯一性、目录全量成员、blocked_by 只指向较早阶段、输出预算和真实 Host 事实仍须由生产实现及独立检查落实。它们不是目前已经通过的工程验收，也不需要为决定稿伪造真实 SDK 文件。

用户决定之后才进入生产 schema/decoder、Host 与模板实现；新候选仍须集成后独立复验。旧模板、旧 init/doctor、C06/C07 及整包退出范围保持。
