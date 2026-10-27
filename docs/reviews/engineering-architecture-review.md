# Rust/C 工程重复构建：架构独立只读审查

审查范围为 `engineering-rebuild-proposal.md` 第 1–6、8 节，对照实际源码与 ADR-0019/0023/0024/0025、工程初始化和 C04 规范。**本次不批准第 7 节，也不把本报告当作完整 wire/CLI 格式通过。** 第 7 节的输入集合、完整工程结果身份和可执行向量仍待展开，最终字段齐备后必须再审。

## 结论

方向可行：显式选择新 0.2.0、保持旧 0.1.0/0.1.1 字节和默认入口、一个 Wrapper、独立尝试目录、现有 C04 三层事务、固定完整结果、保守全量构建，足以承接本有限子范围。没有必要增加插件、通用迁移、第二套 DAG 或第四个 C04 Layer。`project build` 与 `install-tool`、可执行权限及新增输入/结果是待决定的具体增量，不是现行 init 的隐含能力。

**现稿有 3 项批准前阻断，修订幅度可保持有限。** 以下是设计缺口，不是对尚未实现代码的运行失败判断。

## B1：发布后的错误与取消不能被归为“current 不变”

位置：提案 §6（当前行 72–76），§2 行 25。§6 无条件写“失败或取消……current 不变”，没有说明原子替换成功、目录 fsync 或回执/结果保存随后失败的窗口。

实际依据：`adapters/host/src/transaction/commit.rs:94–106` 在最终重验后执行 `renameat(current.json)`；替换已发生后 fsync/回执错误明确为 `STORE008`，不能声称未提交。`crates/madb-app/src/transaction.rs:251–268` 在 publish 中封口取消；`pipeline.rs:190–218` 保持准入直到最终提交。当前 `bootstrap/init/control.rs:49–54` 会无条件设置 requested 标志，这适用于“源码已创建后仍可取消后续检查”的 init 生命周期，不能原样当作“工程完整结果已发布”的终态。

影响：报告可能同时返回取消/失败并声称 current 保持旧批次，而下次运行实际选中新批次；恢复错误回退会覆盖别的成功发布。

最小修订：明确工程级最终提交门与 C04 各子阶段的提交门分离。CAS 前失败、冲突或取消不改变工程 current；CAS 线性化胜出后不再接受终态取消，后续信号不得改报未发布。结果/证据应尽量先冻结保存再提交，且仍保留“已替换但持久化或回执不确定”的非零结果和核对 current 的恢复步骤；不自动回滚、不凭失败码重跑覆盖。第 7 节后续必须能表示真实已发布与不确定观察，不能用一个默认 false 掩盖。

必要定向验证：CAS 前/后屏障取消、替换后 fsync/回执失败、错误后另一构建发布；核对真实 current、退出类别、保留证据及不执行回退。当前未运行。

## B2：静态 Wrapper 的首次摘要核验尚没有闭合的数据取得步骤

位置：提案 §3 的 `tool.sha256`（当前行 41）和 §4（行 50–58）。launcher 要求是归档中固定原字节，只声明 `/bin/sh` 和 `/usr/bin/shasum`，而预期工具摘要在每次初始化产生的 JSON 描述中；没有定义启动 MadB 前如何可靠、无歧义取得此值。

实际依据：现有 `adapters/host/src/initialization_source.rs:20–40,55–73` 能捕获、绑定并复核实际运行镜像，但这是进程启动后的 Host 事实，不能替代 Wrapper 执行前核验；0.1.1 也没有可直接复用的 Wrapper。把新候选摘要写死进“被该同一候选内置的模板”还会形成构建自引用，不能靠每次改模板填自身摘要解决。

影响：实现者容易用 grep/sed 提取 JSON、执行后才自检，或悄悄引入未登记解释器/第二份可编辑摘要权威；这些都不是当前描述的精确路线。

最小修订：在本稿一次性选择并写出一个可执行的 bootstrap 规则。优先保留静态 launcher 和单一 `project-build.json` 权威，明确所用受支持宿主 JSON 提取能力及其固定定位、错误/重复键/类型拒绝、digest 严格格式和缺工具结果；如果选择独立有限数据 sidecar，则它必须是由 descriptor 派生、初始计划登记、每次 Host 对照的非第二可编辑权威，并把新增文件/模式纳入本次决定。两种都无需通用脚本 renderer，不要把未选择的两种并行实现。仅有“先 shasum 后 exec”口号尚不能批准。

必要定向验证：动态不同候选摘要、缺失/错误镜像、重复键或同名子字段、换行/空格/中文根、空 PATH、镜像在核验后更换的既定非沙箱边界；禁止把执行后观察写成执行前通过。当前未运行。

## B3：工程级发布域的归属、旧结果固定和恢复策略须明确到 Host 准入

位置：提案 §3 行 46、§4 行 52/56、§6 行 72–76。保留工程根目录身份和“本机 current 是私有存储”不能自行证明已有 `.madb/build`、current 或 attempts 内容都属于该工具。另“旧有效批次不自动 GC”不能直接继承现有通用 recover。

实际依据：`adapters/host/src/transaction/ownership.rs:16–23,50–68` 要求有效 owner，遇未知内容拒绝认领/清理；`transaction/mod.rs:99–129` 仅新空域可建立 owner。`transaction/recovery.rs:14–49` 只保留 current 的三层链和活跃 lease，其他批次会被删除。`crates/madb-app/src/transaction.rs:118–134` 发布新 Source 会清空同域 Build/Candidate，故尝试必须真正独立，不能误把新工程 current 放进旧 native domain。

影响：根据路径名认领、覆盖人工 current，或把通用 recover 用在历史工程结果域，会破坏用户文件/旧成功结果保留承诺。对缺失、损坏或重定向的旧 current 若都降级成“首轮无旧结果”，也会掩盖已存在的有效性问题。

最小修订：列明 init 一次创建的共享文件、本机域和固定 owner 责任；后续 build/install-tool 仅接纳这份来源与 Host owner，缺失可以按有限规则建立，非空无 owner/未知条目/链接/错误已有镜像零覆盖拒绝。捕获 current 时固定其代次、完整结果身份与路径并核验；区分合法 absence 和损坏记录，损坏先拒绝而非自动 reset。原始描述/归档/profile/SDK 的基准以已核对初始化来源为依据，不能把用户改过的 descriptor 自己充当批准来源；该依据的确切文件或内置目录映射须列出。每次 C04 domain 只在自有尝试内，工程 current 是薄聚合索引；不增加 Layer、不复用通用 recover 去回收历史工程结果。首版无自动 GC/跨尝试 resume，孤立或清理未知尝试只保留诊断即可。

必要定向验证：非空无 owner/陌生 current、损坏旧批次、错误镜像、两个首次 build/install 竞争、旧成功批次无租约但仍守恒、SIGKILL 后不按 PID 猜删。当前未运行。

## 实施约束与无需另加框架的修订

1. **组合根须准确归位。** 提案 §5 行 66“应用层复用 C04 coordinator”容易被解释为 app 导入 CLI/Host。真实 `crates/madb-cli/src/bootstrap/execution.rs:16–24,52–58` 是装配具体 contracts/host 的 `execute_controlled`；`docs/architecture/02-modules-and-ports.md:161–173` 禁止 app 依赖具体适配器。最小修改表述为“CLI composition root 复用现有 coordinator；app 持有受检请求、阶段/结果规则与小型端口”。抽取 `bootstrap/init/recipe.rs` 的共用有限 helper orchestration 即可，不搬整个组合根到 app、不新建服务容器。
2. **完整输入是明确上集，不是缓存许可。** §5 的完整目录捕获、实际头依赖复核和每次全量执行是合理限制；继续复用 C04 捕获/观察和现有有限 Rust 读取器。`adapters/contracts/src/native_values.rs:693–740` 已拒绝隐藏编译输入宏/模块；`adapters/generators/rust_c/values_driver.py:56–80` 又以 rustc dep-info 核对单一作者源和环境。不要为了重建把这些放宽。C 的实际搜索路径/系统头及不存在条件需在最终字段/配方中精确展开；不能只靠 depfile 列表忽略 `__has_include` 的搜索域。
3. **固定路径与锁次序保持简单。** 按 §5 在最终 attempt 路径生成/链接后不搬动引用绝对路径的 consumer；固定 Candidate 与整个工程结果后再给调用方。最终 project 提交锁只做有界检查、CAS 和原子 IO，不启动 CMake/MadB、不取得可能反向等待 project 锁的子域写锁；C04 租约在进入 project 临界区前已固定。无变化允许完整新尝试，不伪造 cache hit；默认入口仍保留 0.1.1。
4. **内置安装路径不等于可迁移第三方安装。** `InstalledValuesTools` 继续从编译进工具的 installation 记录核对固定绝对路径（`initialization_execution/installation.rs:11–12,44–65`）。`install-tool` 可以恢复项目内同字节 MadB 镜像，不应据此声称换机器后第三方链已经齐备。缺失工具正常 preflight 失败；精确第三方安装的便携化不是本次隐含扩展。

## 后续决定与验证边界

先修以上 3 个阻断和组合根表述；再补第 7 节可执行 schema/摘要投影及正反向量，整体新增契约只提交一次用户决定。现有 doctor/init/C04 的旧语义和旧候选不重审。独立检验应明确区分 SDK bootstrap 运行、普通 consumer 编译/链接、普通业务运行和 IDE 导入/运行/调试；本提案 build 子范围不代替 I01/I02/I03/I06 整包出口。

本次只读源码/文档，未执行构建、测试、IDE、恢复或发布；仅写本审查文件。尝试委派额外只读窄审因 agent 并发上限未启动，本报告由当前独立审查者直接完成。

## 绑定证据

- 实际产品 HEAD：`2fac2f7437e7bc235392fcca8c5d7af86b510e22`。
- 提案本次读取得到：`sha256:e6a8181bcd2074e1ecc443c519f060e0605c2a95a4ae764ef05f92628edad3cb`，17519 字节；后续提案变更不自动继承本次结论。
- 开始核对时工作区已有他人的 catalog 与第十二批记录改动，未触碰。

```json
[
  {
    "path": "crates/madb-cli/src/bootstrap/init/recipe.rs",
    "sha256": "e06762df1ca09d67338e842858d2aa6a11e6d3c835c417fbcc10a051ba5c996c"
  },
  {
    "path": "crates/madb-cli/src/bootstrap/init/control.rs",
    "sha256": "0f45ec1c86e8748ec40e65cb4acc8ad05e09d32bb33730fdda09a32d32dfc635"
  },
  {
    "path": "crates/madb-cli/src/bootstrap/execution.rs",
    "sha256": "929a7e318d47addbb6e47da3f9f76accb0ddba85b50a2c30f2eb744c84f1e320"
  },
  {
    "path": "crates/madb-app/src/transaction.rs",
    "sha256": "76bac52640ef3abaa708f22890584c7faebca14d54369ff3136c657424068656"
  },
  {
    "path": "crates/madb-app/src/pipeline.rs",
    "sha256": "a354d884ad017a490daee6cba375e4d790a1c69b269b34516d4321e8fe07e3b5"
  },
  {
    "path": "adapters/host/src/transaction/commit.rs",
    "sha256": "fc3de6f7383263fa1b11c98b448f224e6752a62dc812402e9379f5c800c60531"
  },
  {
    "path": "adapters/host/src/transaction/recovery.rs",
    "sha256": "fa51f7c763b55294ced35a095c93462630d77e24f8bbd617c7fdb171e82df0a5"
  },
  {
    "path": "adapters/host/src/transaction/ownership.rs",
    "sha256": "e0e46cb9316bd7f275bef6c8cf486b56d2eeaf21576c15799987b8ffa89ad597"
  },
  {
    "path": "adapters/host/src/initialization_source.rs",
    "sha256": "a238d0de95962aca2c3ef0072ae44aaa82d27c05963fdf9c94b556b381230f0c"
  },
  {
    "path": "adapters/host/src/initialization_execution.rs",
    "sha256": "f13919c2659d1892041a9536efef430dc223a24e2d475259fa5f2139cacfef6d"
  },
  {
    "path": "adapters/toolchains/rust_c/init_values.py",
    "sha256": "abaf165a8f9a048843f7b8526509db745b535c82f8e1840b782eba29aeafb4ca"
  }
]
```
