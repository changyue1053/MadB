# Doctor 首片最终独立审查

结论：本次已批准 doctor 既有冻结工程上下文首片在下述 macOS Arm64、实际工具/安装/上下文范围内独立复验通过；没有未解决阻断。此结论不收口 I02 整包，完整集成门禁由主协调另行报告。

## 边界与依据

- 已批准提案：`i01-doctor-proposal.md`，SHA-256 `60bdba6ebb41197a4d7623dcdfe4d22a150126621f746b195675cd641c96c17d`。
- writer 停写清单：`doctor-candidate.json`，SHA-256 `a4eafbe997d266eb9baaf9eb9287e18c972744b3f9d5d5ac960826f17b00ac92`；本审逐字复核其 32 个生产/直接相关测试文件。清单形成于未提交工作状态，不把其 base HEAD 当作包含全部输入的生产身份；正式 source/image 身份须另绑定。
- schema/protocol/共享向量独审另见 `doctor-format-implementation-independent-review.md`。独立 Python 26 个控制字符/边界向量、49 处未知字段和256处必需字段删除通过预期，不代替执行证据。
- 此切片只观察既有冻结工程上下文。有限真实配方为 macOS Arm64 Rust→C values、固定 0.1.1 模板安装组合；返回 0 表示所请求级别的静态前提满足。它不生成、不构建、不运行普通 C，不构成 I02 整包、I03、SDK/Lab 或发布认证。创建前模板选择式 doctor 仍是后续子范围。

## 源码判断

复用 SnapshotAdapter 的完整 check 和既有 frozen-input/contract/policy 检查，局部 prepare 不足以产生 configuration passed；Context 记录、报告与观察端口不能构造执行授权。InstallationObserver 没有到 InstalledValuesTools 的转换，CLI 组合根不分发 ProcessRunner 或写入端口。

只读输入由 descriptor/NOFOLLOW 有界读取，保留根、文件和祖先身份并在返回前复核。已加载 main 的真实映像被绑定，版本、源码修订和 recipe 不替代映像字节。安装路径来自编译内置固定记录，不依赖 PATH 或用户指定工具路径。固定 Python loader alias 沿既有 verifier 验证实际 image_path 和完整 library_links，不误拒合法安装别名。

已定位并关闭：

1. root-open/reconcile 提前退出绕过 main/输入复核；最终所有已获得材料走规定收尾。
2. 两次观察清单长度不等被跳过；数量/ID 漂移现为 Conflict/QUERY002。
3. 报告退出码允许无根据抬升；当前严格等于真实错误类别最大值，DOCTOR001 的真实 Host 预算错误单独计入。
4. 仅插入顺序或空 blocked_by 导致成功阶段错序；当前按真实依赖后 id 排序，晚期 configuration Conflict 仍保留 inputs→snapshot→configuration，早 prepare 失败不制造环。
5. 默认通用读取器把权限错误归 InvalidInput；doctor 窄适配沿真实 IO 区分 Host，同时保留拒绝链接/非正规对象等边界。
6. 工具/SDK/资源先验证后取身份导致同字节替换窗口；当前 snapshot-before→真实读取/验证→snapshot-after，并与最终重观测比较。
7. 任意 InvalidInput 误归 unsupported；当前仅已明确有限配方 profile/SDK 不匹配为 unsupported，其余按真实错误分类。
8. 根 `/` 被 init 输出类型误拒；doctor 独立 RootPath 允许合法现有根，不扩 wire。

完整结果先按实际 canonical bytes（含 LF）测 1 MiB，再产生有界 DOCTOR001 摘要；摘要不隐瞒既有 Conflict。进入 doctor 机器分发后，参数/路径/身份/读取错误输出合法 JSON，帮助/版本文本为明确例外，stdout 不可写为 OS 输出例外。控制字符和长诊断在展示层有界处理；NUL 的字符串测试不冒称 OS argv 可以携带 NUL。

## 已读取的实际定向证据

`doctor-final-v2-01.log`：doctor 8、doctor_query 4、旧 query_races 6、explain 9 通过；包括晚期顺序、真实错误汇总、DOCTOR001 缩减保留 Conflict、52 码解释闭集。

`doctor-final-v3-01/02/03.log`：host 3、installation 8、CLI doctor 3 通过；installation 包含 tool.ninja/sdk 在真实读取与后快照间同字节替换、坏字节再变化、alias 漂移、SDK 无权限及三个独立缺/错输入类别。旧安装 4 项验证保持。CLI 单元17见 `doctor-final-v2-04.log`；最终 all-target Clippy 见 `doctor-final-v3-04.log`。

这些 Cargo 命令由实施者运行，本审读取原始日志并对应具体断言，没有冒称独立重复执行。v3 的测试临时目录改为系统 canonical temp；需要真实已加载映像的两项 CLI 测试准确限定 macOS，帮助/版本仍跨平台。本次不外推其他主机。

保留先前失败：schema 初稿控制字符尾 LF；fixture 非现有 FileRef 的 encoding；向量索引 1.5 的词法失败；required-schema 期待码修正；安装快照 signature move 编译失败；Cargo 多过滤器命令调用失败；DOCTOR001 漏入 explain 封闭码表失败。修后通过不改写其原始结果。

## 正式候选独立回放

实际 clean source 为 `e26a73b6bf8cbd9c6ffded3240ebb6f9e49a09c4`；13 镜像正式构建收据 `frozen-tools-001/receipt.json` SHA-256 `f312f465260147733d6cf3f802d72819f14b653bb2b60b817fa81c67e8a26977`；main 为同目录 `images/madb`，SHA-256 `ca285dcface93ed83e448ca08d86d6b9c7bb04a5f3a2637d0c1d50f5235252c6`。本审开始时核对收据/实际 main，回放期间及完成后复核 clean HEAD、13 镜像、32 项生产清单未变，见 `doctor-independent-source-guard-001/002.json`。

独立回放按 **15 + 2** 项原始证据通过，明细见 `doctor-independent-final-summary.json`（SHA-256 `e5786e58a3beda205ed9e154b9df540cf09b99a7ba5ce3db7bc9c77384c82683`）：

- structure 空/污染 PATH 各退出 0、各精确 6 行；build 空 PATH 退出 0、精确23行；run 空 PATH 退出 0、精确24行。所有必需行实际通过，真实阶段拓扑顺序正确。run 仍是 preflight-only。
- LF、DEL、超过4096字符的根、超长未知参数、父级逻辑路径均准确退出2并输出合法机器 JSON；合法根 `/` 进入读取后准确报告缺失配置；帮助/版本保持文本例外。
- 缺 snapshot 精确为 context.snapshot failed/invalid-input、META001、observed missing；configuration blocked_by snapshot，Context snapshot/recipe 均 null。
- `explain DOCTOR001` 文本和 JSON 实际退出0，解释包含预算原因和不改变 Conflict/不执行构建等边界；既有 `SEM007` 的文本与 JSON 两项补验实际退出0。

`doctor-independent-final-002` 保存前15个有效通过及一个回归码选择错误；其整个脚本实际退出1，未改写通过。旧码曾误选 `META001`，但原51码闭集中本无此码，实际输出 EXPLAIN001/2 是正确行为。已用原闭集真实存在的 SEM007 在 `doctor-independent-explain-003` 单独补两项，仍绑定同一 main/source/schema/空 PATH 与只读 guard，没有为测试追加公开码。

此前 `doctor-independent-final-001` 的复制夹具仅复制 read.inputs，漏了 lock.schemas 28个成员，真实 doctor 正确报 META001 并 blocked；保留退出1及原输出。新002复制 read.inputs、lock.inputs、lock.schemas 的现有原字节，不修改原工程、不重写锁或降低期待。新根为 `/private/tmp/madb-chief-round11/doctor-independent-final-002/中文 空格 context`，原根与逐文件一致性清单见 `doctor-independent-context-002.json`；测试 harness 仅为缺 snapshot 场景主动删除再恢复其副本，原工程不变。

每条通过命令保存 argv、实际退出码、stdout/stderr、上下文树前后比较和 main 字节复核；输出经独立 schema 引擎和严格重复键/整数 JSON 检查。小矩阵本身只证明受测上下文子树不变，不单独宣称操作系统全局无写入/进程/网络跟踪；后者的实现边界依据上述能力隔离源码审查，不伪装成 syscall trace。

本审未启动 Cargo、GitHub Actions、push、发布或 SDK/Lab运行。Actions 未执行不计通过；完整仓库门禁与文档/目录集成仍由主协调绑定其真实结果。
