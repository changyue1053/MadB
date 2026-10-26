# I01 完整格式提案最终独立复核

结论：**可进入决定审查；未实现、未验收。** 初审 R1–R4 已关闭，复核中新增的报告证据根定位冲突也已由显式待定字段修正。本结论仅绑定下列最终设计材料，不表示采用决定、生产 decoder 通过、真实 CommitNew、平台运行或 I01 整包通过。

## 最终字节身份

| 材料 | SHA-256 |
|---|---|
| `i01-final-format-proposal.md`（44,216 字节） | `b0cfc59eba442fa3d2a9e5485b9f5778a48ce330073314a044c04366ca600e9f` |
| `write-i01-design-examples.py`（14,447 字节） | `da4bb324b287f12d91cfb9a4acc805f71aa4460160e261be933203a6327fd866` |
| `i01-final-format-examples/index.json` | `062911be27ed0a3b3abdf40cdc37379cbd3973106b402edf4f5cad55c0ccf781` |
| `i01-final-format-examples/template-design.tar`（43,008 字节） | `2a47930bdb3f9298fc055469c8436f019fc648805dbdd3abcb94d78603d0a17b` |
| 原包 `meta/template.json`（8,848 字节） | `a5085f8a087bf6c1abe1bdf5183d7c384381402504f2f3811ebbcd6fa95fe034` |
| 原包 `profile.json` | `c65a81c80cff5c76733a52b8c4cc5dc9d7cf5c50c00c6343fa1e984901f35e1d` |

实际审查对象位于 `/private/tmp/madb-chief-round7/`。历史初审 `i01-final-format-independent-review-001.md` 保持原文，SHA-256 为 `232ee5a4c6e6e8589c50780628a0652c394cb762d71fddc9edcfb7d2fdb69cc5`；其提案 `a40010d4…` 的阻断结论不回写。中间修订提案 `f71aecb1f6e24977d00f8e06271bf09f68910e231328f9c5330df44697db37d2` 曾关闭四项初审问题，但报告证据根输出通道仍不完整；最终结论仅适用于上表的 `b0cfc59e…`。

## 初审问题关闭情况

| 项目 | 最终修订与只读复核结果 |
|---|---|
| R1：原 profile 根相对引用断裂 | 原 profile/SDK 回到包根 `profile.json` / `toolchain.json`，manifest 仍在 `meta/template.json`。原 profile 字节与冻结候选完全相同；从唯一包根按原 `sdk.path` 读取可精确匹配 SDK SHA/size。完整 entry 集仍是 manifest 加其 files 库存，共 9 个文件；没有 profile 父目录 fallback、改写冻结字节或 manifest/package 自引用。 |
| R2：结果 FileRef 缺实际证据树 | 明确的 `evidence-root/` 已保存 `template-package.tar`、完整 `template-input/` 和模型日志。独立按根解析 dry-run 模型的 3 个引用、created 模型的 4 个引用，实际文件的 SHA/size 全部相符；离线缺包模型无文件引用。原包副本和完整解包集合与设计包逐字一致。 |
| R3：目录/manifest 身份未显式相等 | 最终 §4.1 明确解码后的 manifest.id/version 必须与可信目录记录逐字相等；仍由受信固定目录绑定原包与实现镜像，不新增任意 public catalog 入口。该门禁是待实施规则，本复核不声称已执行生产拒绝测试。 |
| R4：downloads.source 形状不唯一 | 最终 §7 明确复用 `{kind:'bundled'}` / `{kind:'https',url}` 两个封闭对象及相同 URL 约束；version 与 size 定义明确，不再允许实现自行选择字符串或对象形状。 |

原包调整带来的 provenance 计划身份已同步：`.madb/template-package.tar` 与 `.madb/template-input/<包内path>` 均按新原字节重算；计划中的这些 SHA/size 与新包一致。模板宿主范围与正文同为 `[26.6.2,26.6.3)`，仍只表示待验证首组合的筛选，不将旧广范围当作兼容承诺。

## 复核补丁的证据根定位问题

中间修订声称 JSON 输出告知应用自选报告根，但精确 envelope 没有对应字段，stdout 消费者不能从相对 FileRef 推导根。协调者在本次未定格式中显式加入 `evidence_root: null | {path,redacted}`，没有改变已批准的 template 五字段或 input_root 含义。

最终正文明确：未脱敏值是实际报告根的规范化绝对路径；有 File/FileRef 时不能为 null；脱敏值精确为 `<evidence-root>`，必须随结果文件/证据目录提供且不能独立重放；普通与 dry-run 成功都必须实际保存根下的 `init-result.json`，保存失败不得伪造文件引用。此字段也明确列入本次待决定范围，不伪称 ADR-0019 已批准。

只读字节核对：所有 InitResult 模型向量都已带该字段；正例根值指向真实设计证据根；离线缺包模型为 null 且没有 FileRef。根中的 `init-result.json` 与 positive-dry-run 模型 JSON 相同。created 模型独立事实仍为 `actual:false`，没有冒称真正创建工程。

## 复核范围与限制

本轮只复核初审四项、证据根定位修订及所列补丁是否造成新的实质冲突；未扩大功能或验证矩阵。采用只读文本、JSON、文件集合和 SHA/size 检查，未执行设计生成脚本、生产构建、init、CMake、Ninja 或产品测试；只新增本最终审查文件，未改提案、001 历史记录或两仓。

在这个范围内未见剩余 P1/P2 阻断。已批准 A–D 继续直接采用；profile/2 通用格式与首 recipe 仅准入 profile/1 分开，不自动降级；初始 provenance 输出明确纳入同一受检计划，实际报告根位于用户最终根之外；required 检查、真实 CommitNew、frozen dry-run 和已创建后失败保留的边界保持。

后续仍须按既有决定流程批准剩余 envelope/包/renderer/recipe，随后实施并独立验证。CMake/Ninja/recipe 安装镜像 pins、完整生产目录、包获取与安全解包、真实最终根创建竞争、最终根消费者构建/运行、IDE/Wrapper 及独立 Lab 证据仍未由本材料证明。native-values 首片不替代 C05 或 I01 最终承诺，旧候选与历史报告结论不变。

## 主协调精度修订复核

本追加绑定提案 `/private/tmp/madb-chief-round7/i01-final-format-proposal.md` 的 SHA-256 `09672664872c3d56fe1b81857692be87f4bd8222e38b012e9d5902055e207fa6`，45,441 字节。仅复核 §7 第 226–230 行的字段精度补充；上文 `b0cfc59e…` 版本复核原文和 001 历史记录均保持。

对照实际产品 `docs/schemas/report.schema.json` 第 23–73 行，`checks.environment.items` 的七个必需字段、未知字段拒绝、Id/Digest 引用以及 version/source/arguments 字符串和数量边界完全一致。提案明确只复用 items：其外层 0…4096 允许静态检查为空，与 report.environment 外层至少一项的差异已明示；source 仍是观察字符串，不混成目录/download 的判别对象。

`checks.command` 和 `recovery[].arguments` 的非 null 值已精确限定为 1…4096 个字符串，单串 0…4096、首项非空；空字符串参数仍可保留，不拼 shell。command 是记录，recovery 仍仅为建议；提案明确这些 wire 约束不能替代现有 ProcessPolicy 的执行字节预算、无 NUL 与环境白名单。没有新增字段、状态、执行授权或 A–D 决定变更。

设计脚本仍为 `da4bb324b287f12d91cfb9a4acc805f71aa4460160e261be933203a6327fd866`，索引仍为 `062911be27ed0a3b3abdf40cdc37379cbd3973106b402edf4f5cad55c0ccf781`，原包仍为 `2a47930bdb3f9298fc055469c8436f019fc648805dbdd3abcb94d78603d0a17b` / 43,008 字节。现有三个正例使用的静态空 environment、null command 与空 recovery 均满足此次精度定义，不需改变模型语义。001 文档 SHA 仍为 `232ee5a4c6e6e8589c50780628a0652c394cb762d71fddc9edcfb7d2fdb69cc5`。

结论：本次精度修订无新的实质阻断，**可进入决定审查；未实现、未验收**。只读核对及本段追加不构成生产 schema/执行测试；未改提案、实例或两仓。后续审查/决定应使用本段绑定的新提案 SHA，不能继续引用旧字节作为当前版本。
