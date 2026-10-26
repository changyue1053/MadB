# L02 explain 最终独审

结论：**无剩余阻断，可交项目所有者裁定；尚未采用。**

最终提案 SHA-256：`6bc0f329a0e3c7e0c537b235ede7efede61bf58366c414cce95e97b6e07bf821`。

## 已闭合阻断

CAP003 原表述错误：实际规则在 `precision == Heuristic` 时拒绝安全包装，并非“符号调用形态不支持”。作者已按 [实际分支](../../crates/madb-rules/src/contract.rs#L161) 修正最终冻结稿并保留旧材料；CAP001/002/004 亦复核对应。

## 非阻断实施注意

- [现有宿主读取](../../adapters/host/src/lib.rs#L127) 将多数 I/O 错误归 InvalidInput；新 explain 路径需要正确区分 EXPLAIN003/Host/3，同时保留原六命令分类。不能靠错误文案猜分类。
- [现 CLI](../../crates/madb-cli/src/commands.rs#L10) 使用可直接退出的 `get_matches()`；解释分支的 `--json` 参数失败须接管为规定结果，help/version 与旧命令保持原行为。

两种闭合 envelope、0/2/3/4 查询退出、同根来源引用、byte-lf/UTF16/BOM 与无 producer-auth 边界明确。model/app 纯逻辑、contracts/protocol 转换、host 读取、CLI 展示的分层可实施；没有必要引入新 crate、任务系统或自动修复。SDK constructor/hooks 不在本决定内。

## 实际验证与限制

只读重放最终设计脚本：28 个格式/语义片段、14 个位置例通过；29 来源 pin、47 已有码拼写存在检查和5交叉检查另列。仅抑制脚本写回作者结果的那一条语句，所有验证逻辑执行。另用10个固定位置反例对照真实 ArkTS helper：BOM、astral、中点拒绝、CRLF行尾/换行内部拒绝、末尾空行、第二BOM、非JS分隔、LS及非法UTF8均一致。最终输入 hash 未变。

**这不等于已实现或验收51项说明字典。** 未运行生产CLI/host/reader接线、SDK/Native/设备或整包验收；源字节点一致性不认证 producer。未修改产品、作者提案/脚本/结果或历史报告。详细来源与计数见 [JSON](l02-explain-independent-review.json)。

仓库导航副本仅将三处临时绝对路径改为仓库相对链接；原结论不变。[批准时原稿](l02-explain-independent-review-original.txt) SHA-256 `f0d0903150e8201b8c77b0c33cf9c8cb0ae8f3e8f7da22c96d87aacb87c6b9e4` 保留。
