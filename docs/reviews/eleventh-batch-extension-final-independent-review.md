# E01 / E03 最终冻结候选独立审查

本报告只审现行 catalog 的 E01 六条、E03 四条验收；不改 catalog、产品源码或历史报告。依据实际读取的 AGENTS、工程方法与质量规范、两任务 documents、ADR-0017/0018，以及本批出口审计、依赖补丁独审和语义实现独审。审查者未参加这批实现，未运行 Cargo；独立脚本及输入副本只写本批临时证据目录。

**最终结论：E01 六条与 E03 四条现行验收均有足够的实现及验证证据，可按下述精确候选和有限范围收口；未发现阻断性缺项。状态只由主协调写入 catalog。**

## 候选与范围

- 干净源码：`/private/tmp/madb-chief-round11/extensions-source`，HEAD `fe75241697469292d7af06ff6bb4e1ce47cc4018`。最终再次读取 git status 为空。
- 13 个正式工具镜像：`frozen-extensions-001/receipt.json`，SHA-256 `e3da7d727d7b3bcde226c5b52eee9c96212af96d110b294980de6760657605c1`。源包文件身份与执行镜像身份分别核对，不能以 Git SHA、同名工具或包摘要代替镜像准入。
- 实际执行宿主为 Darwin arm64；有限主配置为 C provider → ArkTS consumer、`ARKTS-C-NAPI`、aarch64-linux-ohos / API23 的 Source 生产配方。profile 中的目标/`artifact_kind=har` 是该配方的选择信息，不是已构建或认证 HAR。
- 新配方 `source-semantics-{ordinary,enhanced,disabled}@2` 均采用既有 `composition/1`、`profile/2`。每份真实 IR 有 16 个 call；责任数为 95/107/95、资产 5/7/5、直接依赖用途 5/10/5、公开 Source payload 15/17/15。
- 保留 `chain-*@1` 的有限 5/8/5 项来源观察责任；旧单 generator 也是其既有范围。配方后缀不是 wire profile 的版本。这些入口不继承新配方的完整 16-call 责任保证；旧 profile/1 继续独立并拒绝组合新语义。

## E01 六条逐项结论

| 原验收条目 | 判断与实际证据 |
| --- | --- |
| 1. 清单正反例、唯一身份、引用、环、权限和锁 | 满足当前有限注册范围。`extension.rs`、`composition/pins.rs`、`ownership.rs`、`admission.rs` 分别检查唯一清单/端口/引用/DAG、真实 file-set 与 manifest/锁字节、宿主权威依赖完整集合、已观察镜像完整 tuple 和权限。原四个真实漏拒已由本次冻结镜像准确拒绝；独立补跑同名现有 tool 的 toolchain/generator 必需依赖仍拒绝，未猜测名称映射。无 run/workspace 分配或 Source 提交。全门禁覆盖正常/重复/缺项/环/权限/锁反例。一般外部依赖映射尚未支持，当前明确拒绝，不冒充已交付任意包解析服务。 |
| 2. DEC-02 共享格式、完整 payload 与摘要分域 | 满足。最终门禁执行共同 schema/decoder 向量及 Rust/Python/Node 确定性检查；组合语义摘要先拒重复身份再排序，manifest、文件和执行镜像仍按原字节身份。独立读取真实 reader/generator 原始 JSONL、read/generate 请求、冻结 IR 与 OutputSet：握手各自携带相符 manifest 身份，传递字节完全相同，一次完整 Source 提交。重复责任语义 key 在重绑摘要前准确 HASH004，未借旧摘要失配掩盖语义问题。 |
| 3. 清单/资产薄索引、无源码副本权威/私有依赖/最终提交权 | 满足。当前没有独立 `extensions/` 注册中心目录；本条不要求新建一套目录或平台。实际组合清单和文件索引引用 adapters/sdk 权威源，固定包物化的是本次交付输入副本；没有另维护 SDK/adapter 实现。逐个核对三配置源包 61/63/61 文件的字节与清单。peer 只有端口工作和 OutputSet，最终事务仍由 app/Host 完成；独立失败证据确认 reader 拒绝后索引不动、generator 不启动。架构依赖门禁保持。 |
| 4. 组件、作用域依赖、版本/目标与 owner | 满足。受信 `RecipeRequirements.dependencies` 独立给出精确 from/to/scope，不能从用户 dependencies 产生预期；先验证节点、重复和环，再比较完整用途集合。对未注册的 manifest 外部依赖按 id/kind 准确拒绝。同名工具不能补成映射。新 @2 配方依据实际导入/ABI 接线声明 5/10 用途；源码、生成槽与文件 owner、精确版本/目标/锁继续核验。原少边/错 scope 真实反例及新相干重绑反例均拒绝；无需新 wire，也未把说明文本、extensions 或私有配置当规则。 |
| 5. 有限推荐组合、精确锁、实验缺口 | 满足。CLI composition root 明确枚举旧单 generator、三种 chain @1 和三种 source-semantics @2；没有任意组合求解器。新三配置与旧三配置均在最终镜像上经过真实双会话、Source 单次提交、独立公开消费；工具版本、host/target/source/arguments 及字节与锁相符。@2 全 profile tuple/feature closure 有相干重绑拒绝；不支持的 selector 在执行前失败。保留的旧单 generator 另完成最终 9 项回归，不以双会话正例替代它。 |
| 6. ADR-0017 共用机制、普通/增强/替代与禁用 | 满足本包的声明/准入职责。普通、增强和禁用使用相同注册、协商与 Source 提交流程；增强增加 StateObservation/ArkUI 资产与各自责任，禁用重新生成普通闭包，含可选源码的禁用配置被拒绝。具名 UniFFI-ArkTS、ani-rs、KNOI 来源/依赖/责任缺口沿第十批固定研究事实，未伪造解析成功的 X profile。当前最终镜像再跑私有 backend_config、未选 recipe、缺组件、缺入口源、错镜像及未知/替代 selector 共 7 个拒绝，均在 run 分配前失败。该结果不表示 X 实现或兼容认证，也不把 X 完成反向列为前置。 |

## E03 四条逐项结论

| 原验收条目 | 判断与实际证据 |
| --- | --- |
| 1. 未知必需功能、模型版本与证据失效 | 满足。最终全门禁实际执行 `evolution.rs` 六项及既有功能/协议向量：未知 IR/model、未宣布 protocol/IR 版本、未知 required feature 均拒绝；同名自报 supported 不能绕过。形状/策略改变真实投影与 candidate 身份；六类证据依赖以及 case 定义变化准确使旧报告失效，重绑报告 catalog 摘要不能令旧 case_digest 复活。这是契约/证据规则测试，未当作真实平台重认证。 |
| 2. DEC-02 共用格式及摘要 | 满足，复用上列 E01 第 2 条最终候选的格式/完整双会话字节证明；不是再次定义一套格式权威。 |
| 3. 按 symbol/stage/instance 唯一责任、能力前提与配置分层 | 满足固定 @2 Source 配方。`c07_source_requirements` 先核 profile tuple、snapshot/IR、19 项核心与 2 项可选固定源码的原始及包内字节，再由真实 16-call 形状/有效策略/公开接线派生所需集合；不是对提交责任自证。provider payload release、Core source ticket retirement、残留 payload release、native context、JS listener、state/event anchor 分开。缺失类型读取/ABI/投影/异步/dispatch/subscription/stop/drain/cancel/release，错 owner/anchor、假 physical-assembly、改 frame/validator、额外资产、弱化取消等相干重绑反例准确拒绝；两项还走真实 reader 会话，确认后续 generator/Source 被阻断。必需语义、未验证私有配置 CAP005、非语义 metadata 各有独立规则。 |
| 4. ADR-0017 有限配置与禁用/替代边界 | 满足。三 @2 配置的 95/107/95 责任与 5/10/5 依赖用途实际贯通 metadata → IR → generator → OutputSet → Source。三旧 @1 保持 5/8/5 观察责任，不扩大旧结论。未知/替代准入及禁用资产拒绝绑定最终镜像；具名后端仍显式列真实缺口，不授予它们主线认证。 |

## 独立复核与机器证据

以下相对路径均属于 `/private/tmp/madb-chief-round11`。

1. `e03-final-001/runs/checks.json`：31 项全部通过，SHA `010db8278d88d28d46cbdfc860995f85b1407549c49d96bf35fb6957c66162b7`。构成是 3 个真实 @2 正例、25 个相干重绑语义/metadata 拒绝、1 个重复 key 的摘要前拒绝、2 个真实 reader 拒绝；没有将 25 个 metadata 入口说成 25 个完整进程流水线。
2. `e03-legacy-final-001/runs/checks.json`：三 @1 真实双会话与 Source 消费通过，SHA `ebd072ec685b6523b816aea4814f631a164cc950c323a4578b553cb93e6aec65`。
3. `e01-frozen-rejections-001/checks.json`：4 个原漏拒反例在最终镜像上 exit 1、准确错误、stdout 空、无 run 目录，SHA `ff329974ec7483feed0217ead074aa44fec9ea970597adaf6ff4bb9feab9b7f6`。旧缺陷成功执行记录仍保留，未改写为通过。
4. 独立 `extension-independent-003/facts.json`：核对上述 6 个成功场景的原始源包、真实独立握手、冻结 IR、Source 全部字节/无额外文件、一次提交与完整进程清理；核 1371 tracked 源文件及工具/观察身份，另补 2 个同名工具依赖反例。SHA `02dabc9f74f246e401dad6f723b36b6637dfe8d13605095d1fa099868ee6bea6`。
5. 独立 `extension-independent-003/negative-raw-audit.json`：核对 26 个 metadata/hash 原始错误及对应 stdout/stderr receipt，2 个 reader 原始 JSONL、Failed/Unavailable、索引 1→1、无 generator/Source、清理完成。SHA `0aa45fcf4aa3d27ba81522cc32a58d7918c40524a279eb1e14748d1ff906ea4b`。reader 外层 probe exit 0 表示观察程序完成；其内部业务明确失败，未把外层零退出写成业务成功。
6. 独立 `extension-independent-admission-004/checks.json`：同最终镜像、@2 普通真实输入副本上的上述 7 个准入/selector 拒绝全部通过，所有原镜像字节保持，SHA `52c73e8e33bed8aad9680ca96150a70f734757699d51e1d2b244989afd3cdc2f`。复用了已有输入装配/进程包装，逐场景断言准确错误与无 run 分配；没有伪造完整 X 输入或执行 X。
7. `extensions-full-gate-002.json`：同干净 HEAD 完整 `scripts/check.py` 实际退出 0，receipt SHA `ae0bb1ebeaa00b7560406d828f6d4ea2d49911aabaa9192608078d037e0e1db1`，日志 SHA `b8089250e215c7444f5406cb43a4f82d74481c1b37cdcf1bc41379401af9d3ee`。审查再次核 receipt、原日志及源清单摘要；1371 tracked 文件字节/模式仍相同。初次 001 因 `UnixListener::bind` 临时 Unix socket 被沙箱 EPERM 拒绝而真实失败；提权后原检查完整重跑，无删测试或放宽断言。不是 FIFO 错误，也不是 GitHub Actions。

8. 独立 `extension-independent-single-005/summary.json`：同一冻结候选补齐保留旧单 generator 的原 9 项正式矩阵：来源/镜像分别准入、同镜像逻辑别名拒绝、成功、三处取消、无效输出、公开 Source 清单/消费及篡改拒绝，整体实际退出 0。summary SHA `390c7ba69f9dee47b8600cada5589ead1f928933e141db6ded8ab0c294019994`；`runs/checks.json` SHA `4585d572c56740701569d5ec7b805a857d3d8e130566aed2b926f177b6dafbf4`。前后 1371 tracked 源文件、13 镜像及观察字节/模式相同；另直接核原始 generator JSONL、Source revision 1→2 和全部 15 payload 字节。

## 判断边界与未冒领项

本审查证明已登记有限组合的来源注册、完整责任/依赖协商、精确准入和公开 Source 消费。Source 消费者独立读取公开清单与 payload 字节；它没有加载 Native、构建 SDK/HAR 或执行 ArkTS 页面。有限编译/装配/目标运行、真实 UI、Swift/Kotlin/其他 ArkTS 后端、E04 的另作者扩展实践及后续 L/I/C/X 的承诺继续由其现行 catalog 验收承担；没有将它们取消，也没有反向新增为 E01/E03 的退出前置。

新语义模块读取并分配生命周期责任，没有修改 SDK 的原生生命周期或并发实现。既有原生/协议测试由完整门禁复验；本报告不将责任表本身写成新的目标运行认证。C06/C07 历史候选结论、旧报告、发布与 Actions 范围保持各自身份。

审查工具自身的 001/002 首次核对因误假定包根字段及旧证据文件名而在执行新增探针前失败，原日志保留；003 按实际 wire 路径/旧证据文件名核对后通过。未改生产格式或放宽产品预期以修正审查脚本。

本报告不写当前任务状态。主协调可依据上述十条逐项结论完成 E01/E03 的 catalog 收口与证据归档；若继续合入其他改动，应保持本候选证据身份，并按实际集成差异复验，不将本报告直接套用于变化后的候选。
