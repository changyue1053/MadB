# I01 完整格式提案独立审查 001

结论：当前捕获版本存在 1 项 P1、3 项 P2，须修订后重新绑定最终字节复核；本记录不是采用决定、生产 schema 验证或 I01 执行验收。ADR-0019 A–D 已批准，不重新申请批准。

## 审查身份与范围

- 提案：`/private/tmp/madb-chief-round7/i01-final-format-proposal.md`，捕获 SHA-256 `a40010d475b3fbef8e376d8d71c028079c6f2d2d21c604c7b2481798c5065ee8`。
- 设计脚本：`/private/tmp/madb-chief-round7/write-i01-design-examples.py`，SHA-256 `982bfed603a69a03d26cd6b9a98b3760e8e7738cc87193b9525e2a65a7aff851`。
- 示例索引：`/private/tmp/madb-chief-round7/i01-final-format-examples/index.json`，SHA-256 `486b64079ffb894e570178af69202c06e98679d31876f4c3a0ccc724d34e6805`。
- 读取了全文、设计脚本、完整 manifest/profile/结果与请求投影、全部反例定义和相关实例；核对 ADR-0019、已批准 project-init-decision、既有 common/strict JSON/profile/root/工具锁和诊断边界。
- 只运行内存中的 Python JSON/字节/SHA/USTAR 观察，无构建、init、CMake、Ninja、产品测试或源码修改；设计生成脚本未执行。此次唯一写入为本审查文档。
- 本文行号对应上述提案字节。协调者收到初步问题后已明确将统一修订；后续字节不能沿用本记录的结论。

## 具体发现

### R1 [P1] 正例改变原 profile 的逻辑根，SDK 闭包断裂

位置：提案第 35–38、103、138、144、250 行；设计脚本第 17–19、40 行；`package-input/boundary/macos/profile.json` 的 `sdk.path`。

提案正确规定内部引用按完整解包根解释；被复制的原 profile 原字节确实保持，但其中 `sdk.path="toolchain.json"`，defaults.origin.location.path 也仍是 `profile.json`。归档实际仅有 `boundary/macos/toolchain.json` 和 `boundary/macos/profile.json`，根 `toolchain.json` 不存在。把两个文件一同搬到子目录并未保留原协议逻辑根；若按 profile 父目录寻找 SDK 则直接违反已批 C。故该包不能成为 §4.3 完整闭包和正例 dry-run 的合法输入。

推荐修订：保持候选原 profile/SDK 字节，在包根保存 `profile.json`、`toolchain.json`；manifest 仍在 `meta/template.json`。同步重算 manifest.files、原包、目录记录、结果外层引用、plan/projection 相关身份及反例。不要通过修改原 profile 或 fallback 搜索使旧正例勉强通过。

关闭条件：从唯一包根直接解析 profile.sdk 能找到精确 size/SHA 的原 SDK；外层 evidence_root/input_root 复验得到同一闭包；嵌套 manifest 父目录仍不参与解析。协调者已接受此修订方向。

### R2 [P2] 正例结果引用的实际证据树不存在

位置：提案第 105、193–196、248–254 行；设计脚本第 52–54、72–79 行。

positive-dry-run/positive-created-model 引用 `template-package.tar` 和 `template-input/...`，当前示例根实际是 `template-design.tar`、`package-input/...`；结果文件还位于 vectors/，没有明确且可打开的另一个结果证据根。索引只给逻辑 FileRef，不能证明这些相对引用已保存。按当前“完整字节正例”描述，独立消费者不能直接重验所有引用。

推荐修订：提供明确的实际 `evidence-root/`，其中保存原归档为 `template-package.tar`、完整原树为 `template-input/`、模型日志及对应结果；也可明确指定另一个已存在的证据根，但不得再只在叙述中假定树存在。结果数据仍是设计模型，实际文件存在不等于 CommitNew 已执行。

关闭条件：给定公开指定的单一证据根，全部正例外层 File/FileRef 均可按原字节 SHA/size 检查，不需别名映射或按 basename 搜索。协调者已接受实际 evidence-root 方向。

### R3 [P2] 可信目录记录与 manifest 身份缺少显式等值门禁

位置：提案第 49、81、99–103 行。

目录记录和 manifest 都定义 id/version，但当前获取步骤只写目录 selector 解析、manifest 文件摘要和完整库存校验，没有明确要求解析后的 `manifest.id/version == 目录.id/version`。即使归档和 manifest 字节正确，也可能发布一个目录称 A@1、manifest 自称 B@2 的不一致映射；id/version 与包 SHA 不可漂移的规则不能代替这项交叉检查。

推荐修订：在 manifest 严格解码后、选择模板 profile 前明确要求两项相等；同时核所选 task recipe 在目录允许且精确绑定的 recipe 集内，不用版本文字代替镜像身份。增加完整不一致设计反例或将此门禁列入既定格式测试表。

关闭条件：目录/manifest 身份不一致明确拒绝，不自动改写、跟随 manifest 另选模板或通过别名兼容。协调者已自行识别并准备补入此门禁。

### R4 [P2] plan.downloads.source 的 wire 类型仍有两种实现解释

位置：提案第 81 与 210 行。

目录 source 已明确两个封闭对象 `{kind:'bundled'}` / `{kind:'https',url}`，但 plan.downloads.source 仅写“固定 HTTPS URL 或 bundled 标识”。按“所有对象精确”目标，执行者仍可分别实现为 URL 字符串、`'bundled'` 字符串或上述对象，形成两个不兼容 decoder。

推荐修订：明确 plan.downloads.source 完全复用 §4 的两个封闭对象，未知键与 kind 拒绝；其余 id/version/sha256/size 指向本提案已有 Id/有限版本/Digest/safe_uint 定义，不引入第二套解释。

关闭条件：一个完整示例或精确类型句子使 source 的值形状唯一。协调者已接受封闭对象修订。

## 已核实的设计一致性

- 原归档实为 43,008 字节，SHA-256 `bec36088423886f73558bae0cb0d04adb37bf292906c5723085124e0cc99523f`，与索引和提案一致。内存逐 entry 检查得到 9 个普通文件，USTAR magic/version/typeflag/header checksum、路径排序、data padding 和恰好两个零结尾块均符合当前实例描述。
- 原包 entry 集精确等于 manifest 自身加 manifest.files 的 8 个成员；所有 inventory SHA/size 与提取内容、package-input 内容一致。manifest 不列自身，未形成 manifest/package 摘要循环。
- 原 profile 与所指冻结候选 profile 的字节完全一致；这证明“字节未改”，并不消除 R1 的逻辑根错误。
- dry-run 请求摘要可从完整投影独立重算为 `sha256:6c4d1f2969bbcb244fd1b201fb32ac1341756be7e4ceb04040051f383a4ec1cc`。本实例所有对象键均为 ASCII，脚本 sort_keys 与当前投影所需 UTF-16 键序结果相同；不能将此观察推广成通用 JCS 实现。
- 正例初始 plan 共 17 行：16 行有真实可重算的渲染/原包/provenance 内容身份，1 行 local-state 为 null/null；独立按各 renderer 的当前实例参数重算，集合与 SHA/size 一致。该计算未创建最终工程。
- single-byte tar 反例与原归档确实恰差 1 字节；unlisted tar 确实只多 `extra.bin`。模板和结果反例以完整对象变更构造；三个 strict-JSON 特例是解析层材料，不是已通过新 schema 的模板。
- A 的 typed error/请求摘要独立性、B 的真实 CommitNew 与 frozen dry-run 分支、C 删除 sdks 与双根、D 完整 profile 集合已继续采用，没有重复审批请求。
- 模板本身不开放 argv/env/shell/hooks；identifier renderer 限定字符及固定资产上下文；CMake 参数由受检配方作为独立 argv 构造。没有将这一点宣传为执行任意作者源码无副作用。
- required 检查由内置配方/选中目标/请求等级独立推导；不是根据结果自报 requirement 或 checks 空集通过。普通成功使用内部 Host 事实，明确 C04 BatchStore receipt 不能冒充 CommitNew；created 后失败不删用户目录。
- 文档明确首 init recipe 是本次新增提议，CMake/Ninja 镜像 pins 和执行均未齐备；native-values 首片不替代 C05 资源目标，不改变 I01 最终退出、IDE/Wrapper/其他目标承诺，不继承 L02/C06/C07 的认证。

## 其余待实现限制

已在提案中真实披露的事项不作为新增审查阻断：生产 schema/decoder、受信目录发布、归档 Host、安全创建及最终根并发、实际 tool/SDK pins、CMake/Ninja/Run、IDE/Wrapper、独立 Lab 联合验收均未实现。设计模型中的 `actual:false/assumed_for_model:true` 不能转换成验收 passed。

协调者同时拟将示例宿主范围收窄至实际观察版本并明示 recipe 自有 provenance 输出；这是提案精确性完善，不改变既有整体平台承诺，也不代表已认证该新 init 配方。完成 R1–R4 后应另留最终审查结论和新 SHA；本 001 历史结论保持不改。
