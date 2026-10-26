# I01 第三批独立审查

只读审查；不是格式采用决定、状态变更或验收证据。实际输入为 `/private/tmp/madb-chief-round3/madb` 的完整脏输入副本（HEAD `5468d314928f8c5d494768a9b17e3ad6b7d22fcb`），并读取了真实产品根的 AGENTS、共同方法、I01/catalog、architecture14、模块/整洁代码/门禁规范。没有修改产品源文件，也没有引入模板 schema、InitResult 或更新模型。

## 当前纯计划结论

`ProjectPlan` 的模块 DAG、文件归属、路径前缀大小写及文件/目录冲突暂未发现可证明的实现缺陷。特别是草案第62/83行允许嵌套模块目录、根 owner 及显式文件归属，不能自行改成“最深目录独占”。类型没有文件内容、当前基准或提交授权；这表示更新准入尚未实现，不表示可从现有 metadata 安全推导更新权限。没有为了推进任务添加第二套更新对象。

实际运行 `cargo test -p madb-plan --locked --offline`，退出0：TaskGraph 4项、ProjectPlan 9项通过。仅证明现有内部纯规则；没有模板渲染、文件提交、IDE或更新恢复验收。

## 格式定稿前需要修正

以下行号对应 `docs/reviews/2026-10-26-project-init-format-draft.md` 当前输入。

1. **第121、160行：未解析默认值不能一律变成 InvalidInput。** 请求投影需要模板默认参数和默认 targets；合法的 `--template foo --offline` 在本地无 foo 时不能计算该投影。当前文字强制 null digest → InvalidInput，会丢失实际离线缺包/宿主错误。最小修正：解析前允许 `request_digest=null`，保留实际稳定诊断与退出类别；只在请求本身非法时使用 InvalidInput。完整解析后才计算现有单一摘要，无须再增加一种摘要域。反例：合法输入+缺缓存，不得报成参数语法错误；后续补齐可信模板后同一完整请求得到稳定摘要。

2. **第164行：普通成功必须要求真实 CommitNew。** 当前列出的成功条件没有 `creation=created`；结构检查通过、未提交、`open=null` 的结果仍满足文字条件。最小修正：非 dry-run 的 exit0 必须 creation=created，且模板/profile/输出/计划身份齐全，所请求检查来自受检选择闭包。dry-run 保持独立的 not-created 分支。增加反例：非dry-run + not-created + 全部structure passed 必须拒绝。

3. **第64、75、123、125行：FileRef 的解析根仍不明确。** common.file_ref 只包含逻辑相对 path/sha256/size/schema，不含作用域。模板资产的 source 已明确属模板包，但 boundary profile、SDK清单和结果中的 manifest/profile 引用没有一致的基准定义；同名 `profile.json` 分处包缓存、报告目录时会发生歧义。建议不扩 FileRef：模板内引用一律相对同一冻结模板包根；结果引用一律相对同一报告证据根，必要的公开描述文件以真实字节留存后引用。若选择其他基准也应只选一种并明确，并加同名异内容/越界引用负例。`sdks` 所谓“SDK清单”当前未找到通用已注册 schema；必须点名既有受检适配器清单和支持规则，或首片使用已锁定组件+显式配方检查，不要暗建第三种通用公共格式。

4. **第77、126、160行：显式 profile + 省略 targets 需要一个明确分支。** 目前只完整描述了两个参数均省略、提供targets、显式profile与请求集合不符。建议明确：只传profile时采用该profile完整family集合；两者都给才比较精确集合。例：默认profile是android+ios，显式mobile-ohos为android+ios+ohos，只传后者不应先套默认targets再报冲突。该默认规则属于待采用格式决定，不应先写入代码。

## 可以减小的部分

- `checks.prerequisites` 已由精确模板及check id确定，可从结果删去；验证器从受检模板读取，避免第二份依赖图。`request.open` 与 `open.requested` 二选一；若为可读性保留重复字段，必须明确相等检查。
- 暂不增加通用跨厂商版本求解器。首片使用有限、固定工具/SDK配方；MadB/template版本比较规则只在确有范围选择需求时定稿，并复用已明确的统一规则。版本字符串可解析不等于任意区间可求解。
- 第91行 json-values 需要明说是 MadB 的严格JSON数字子集还是普通应用JSON：现有 `strict_json`/`canonical_json` 只支持安全整数字面量，不能直接宣称可处理包含小数的任意JSON配置。可以先交付 copy 和明确子集的 json-values，后续专用renderer仍在队列，不扩大通用模板语言。
- 保留新模板与结果 envelope 的独立性；不能复用必须含候选/Lab身份的 report/1，也不要让模板任务根变成任意shell。无需为减字段删除真实阶段/失败恢复信息。

## 最小可实施入口（取得格式决定后）

1. 在既有 SchemaRegistry/共同向量加入两个闭合格式；adapter将受检模块/资产映射到现有ProjectPlan，先做选择、引用闭包和明确错误反例。
2. Resolve/Preflight先纯化选择与摘要，host只提供显式冻结元数据及环境观察；渲染仍由有限copy/json-values适配器承担。结果汇总消费真实阶段观察，禁止自报check名单覆盖模板必需项。
3. 新目录提交、失败恢复、Wrapper/IDE/更新沿现有I任务和事务责任推进；本审查不创建执行入口、不提前改变I01整包退出。C06/C07及既定CLI/Swift分责不受影响。

## 输入摘要

```json
{
  "docs/reviews/2026-10-26-project-init-format-draft.md": "e871462e3c3a8a80edcfa8d72064f4ab60df6d43d6cf063e28c74290097d7331",
  "docs/architecture/14-project-initialization.md": "e2684e5a94cb340ec290a19b90b10cac7d473092447d004c7918744874f211d6",
  "docs/product/04-project-initialization.md": "3cc468ed05aa1de94e220e25551feb0c93e6308a9f8cd2c1695101fc23f746a3",
  "docs/catalog/implementation-plan.json": "5a95eb18a247a3d78546f7d03d04699688812697e55281c336d27ec3097274c2",
  "crates/madb-plan/src/project/mod.rs": "3f526d07d3a8231166d4b8004af3f83e3050fbab67bc1cc0e65d7db463bca65a",
  "crates/madb-plan/tests/project.rs": "18f36ea3e31d2056f09f16792a2ddd0c0f54aeb91063aca3f384db384ed050bd"
}
```
