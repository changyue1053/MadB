# E01/E03 组合格式独立审查（最终修订3）

**当前四项提案可以作为一次具体格式决定交项目所有者；本轮独审未留阻断。** 这不是采用批准，不是产品 decoder、候选认证或 E01/E03 整包验收。所有产品与原稿均由各自 owner 写入，本审查只写临时证据。

## 已核实的修正

- 订阅/Runtime 释放不再混淆：`Subscription.close` 的 facade listener 在 `Boundary.ets.in:318–325` 退休；Runtime dispatcher napi_ref 在 `native-gateway/src/runtime.cpp:127–170` 的整个 Runtime teardown 删除；native callback context 由 `api.cpp:99–108` 与 protocol drain 负责。原错误 subscription+environment 行已删除并有准确拒绝向量。
- `generated.producer` 必须属于已选择组件且唯一，按固定配方核对 generator/contract-reader 单 port；解释组合的 reader/generator 必须声明 `core.extension-composition@1`。一致重 pin 不再绕过这些条件。
- 固定配方的 route/direction/target/profile_id 全量核对；相关嵌套对象未知字段拒绝；旧聚合 ID 不冒充新单 port。
- 包内 File/manifest 引用保持包内相对路径，扁平 lock.inputs 使用 `packages/<package-id>/<exact-version>/<relative>`，包源摘要仍按包内路径计算。该规则避免两个包同名源文件直接撞入一个锁路径，不需要 lock/2。

## 接口与摘要判断

包源/manifest → composition → profile/2 → 源锁/IR → OutputSet 的方向可成立。generated 仅有 producer/slot/path 意图，真实输出身份向下游绑定而不回写本轮输入；sdk/backend_config 同样禁止本轮反向引用。先计算输入身份、后完成语义准入的阶段拆分避免“算profile需要IR而读IR又需要profile”的前提回环。

新组件的 sha256 作为包源摘要仅适用于新 ID/新 profile 约定；它与执行镜像是不同身份。原 `task::bind_tool` 保持镜像核验，本轮 `check_extension_pins` 的 component=image 窄路径保持拒绝，/2 需单独受检映射。没有把新语义静默放进旧 profile/1 或放宽旧锁。

scope 由固定配方与真实受检声明/接线产生，不由申请者责任数组生成 required set。当前两配方的未知 anchor/重复资产拒绝可成立；跨包 re-export、任意物理实例别名和完整责任覆盖尚未证明，稿中已明确。该限制不阻止采用格式方向，但阻止以后把这些设计向量登记为完整组合支持。

新 feature 按真实操作责任施加给解释 profile/组合的参与者，普通 BuildTask 编译器不机械承担组合语义；控制帧仍为 v1 和原五种操作。旧 /1、旧 C06/C07 候选及历史验收未改变。真正端口实现、全部包闭包、共同 schema/decoder/digest 向量、事务和普通/增强/禁用独立消费仍是采用后的交付要求。

## 独立验证结果

1. 修订3默认命令实际 exit0：49项设计检查、17个已捕获源文件，读取不参与并行写入的 madb-baseline；`baseline-r3.log`。
2. 原5个独立反例及新增 reader错误port、generator遗漏必需feature 共7项，全部准确拒绝；`probes-r3.json`。首次误接受及修订2结果保留在 `probes.json`、`probes-r2.json`，不覆盖。
3. 真正 Rust `contracts::source_digest` 与三份最终样例包源摘要一致；真正 ExtensionManifest decoder 验证候选字节，registration 仍准确拒绝 example=true；`identity-r3.log`。没有运行所描述的包或 SDK，也没有把 source hash 当执行镜像。

复验命令：

```text
/Users/changyue/Desktop/MadB/madb/.venv/bin/python -B /private/tmp/madb-chief-round3/composition-decision.py
/Users/changyue/Desktop/MadB/madb/.venv/bin/python -B /private/tmp/madb-chief-round3/composition-review/probe.py
CARGO_TARGET_DIR=/private/tmp/madb-chief-round3/e01-review/target cargo run --manifest-path /private/tmp/madb-chief-round3/e01-review/Cargo.toml --bin composition_identity --locked --offline -- /private/tmp/madb-chief-round3/composition-decision.json
```

最后一条命令从 madb 副本运行，使用仓库 Rust 1.99.1。原审查输入、最终输入与原审查记录分别保存于 `composition-review/review-input.*`、`final-input.*`、`review-initial.md`。

最终输入 SHA-256：

```json
{
  "composition-decision.md": "2ce3ac90a8a870f2c8ab3658bf10f41f8d3061aaf2d498a06da9223289e7a18b",
  "composition-decision.json": "525ccbca40adee65e1f6601d74fe52f9681203d879d69e95fd96551e61015c82",
  "composition-decision.py": "6082cbe94193ff91a7589acf5fd04fa442523db55484517a05be1407335071b4"
}
```

独立证据 SHA-256：

```json
{
  "baseline-r3.log": "287111aa6eadc25a9b5d1ab8700464aeaa467311328a52468b5f92cb43f59c61",
  "probes-r3.json": "a428a27538340909fc840b15ba1d05c9aabdac36dff6a8fadf8826b405c92151",
  "identity-r3.log": "4cd32fe89f7312f85d336a802806022d197a7bab8830637d67062718f43999f1",
  "probe.py": "3b47c72975ef1c079af4a0617a6ed8a5d365d7bf2e86f5cdca72466ae1816705"
}
```
