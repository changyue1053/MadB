# C04 已声明输入快照与私有运行目录

任务：C04 固定输入子范围。基线 `b70c45bd0d2e33f2aab4119bc91383863bc1fb54`，分支 `codex/c04-frozen-inputs`；依据设计 0.9、模块/生成规范、DEC-04/05/06 和 CRIT-05/09/10 的适用前置。PR #5 已通过双 CI，尚未合并。

范围：app 的不可变 FrozenInputs 与受检捕获；host 的新建私有 inputs/work 目录、受限物化、输入副本复核、复用真实进程端口的单次调用；独立单元和真实进程负例。涉及 crates/madb-app、adapters/host、tests/integration、架构/计划与生成视图。不修改公开 wire schema，不开放 build/generate，不实现 DAG 调度、三层事务、自动 GC、缓存或 E02，也不计 C04 整体验收。

实施决定：

- 只固定声明清单中已有且大小/字节摘要匹配的文件。清单数量、单文件/总量、路径深度与目录项有上限；重复、大小写别名和文件/目录冲突在读取/写入前拒绝。读取复用 ReadWorkspace，捕获后再次核对，失败不返回部分快照。
- FrozenInputs 私有字段、只读访问。快照不引入新 wire 摘要，也不证明传递依赖、缺失条件、源目录成员、解释器/动态库或工具链完整；不能据此启用缓存。
- 宿主只在调用方授予的稳定 parent 下原子新建一个单层命名目录，拒绝覆盖已有路径。inputs 保存副本、work 供进程运行；不猜测或重写 argv，配方显式引用 ../inputs。工具位置/环境继续沿用现有 ProcessRunner 契约。
- 准备/执行 owner 非 Clone；执行消费准备对象，返回进程观察及保留目录。任何运行过的目录不在 Drop 自动删除，失败也返回可定位的已分配路径；本批没有自动清理或 GC。测试 harness 在确认其辅助进程停止后才清理。leader/pipe 清理观察不足以证明逃逸后代不会继续写目录。
- 将副本复核放在执行端口返回前，因此取消/结果准入覆盖复核窗口；副本损坏或新增/缺失路径拒绝结果，保留原始进程观察和目录。不把 exit 0 或本地清理当任务提交成功。
- 原源文件在捕获后改变，不改变旧快照的消费结果；是否允许旧结果提交到 current 留给后续输入/CAS 复核。chmod 只降低误写，前后复核不证明没有 ABA 修改；同权限恶意写入、任意路径读取和网络隔离均不在本批保证中。

验收：不变/错误/变化清单、预算和路径冲突；真实链接/硬链接/非普通文件拒绝；私有副本独立于源修改；真实工具读取固定副本、改坏副本时拒收、取消及逃逸后代时目录保留；独立审查、完整 scripts/check.py 与最终 head 远端 CI。只在实际执行后补充通过证据。

## 交付与实际检查

新增 app 的受限输入捕获及不可变 FrozenInputs、host 的 PreparedRun/RetainedRun 和精确副本树校验；公开接口仍为内部 Rust API，未增加 build/generate CLI 或 wire schema。现有 ProcessPolicy 作为不可变配置允许 Clone，执行初始化与验证统一位于一次性 gate 内。

独立源码审查发现并修复一次性准入缺口：原实现先初始化工具目录，再进入执行 gate，初始化失败后控制器仍可复用。现已将初始化移入端口调用窗口；两项回归验证失败后 AlreadyFinished/EXEC001，以及预取消优先于缺失工具目录。

| 实际执行 | 结果与范围 |
|---|---|
| `cargo fmt --all -- --check`、workspace check、Clippy all-targets/all-features `-D warnings` | 通过，无新增依赖 |
| workspace Rust 测试 | 76 项通过，其中新输入捕获 15 项、准备/所有权/失败准入 7 项 |
| Python 契约、C03、C04 查询、进程与输入、架构、文档测试 | 89 项通过，其中新真实输入回归 16 项；前后各 6 种篡改使用子用例逐一断言 |
| 完整 `.venv/bin/python scripts/check.py` | 通过，共 165 项 Rust/Python 测试；另含隔离 Release CLI 安装、Node 摘要向量、依赖检查及 HTML DOM 检查 |
| 文档/视图 | 93 Markdown、52 JSON、13 HTML 静态检查通过；视图由 catalog 重生成，没有执行浏览器布局测试 |
| 真实负例 | 链接/硬链接/FIFO、NAMETOOLONG 后部分目录保留、预执行拒绝启动、零退出仍拒收、取消、持管道/关闭管道的逃逸后代、既有路径不可认领均通过 |

本地环境：项目锁定 Rust/Python/Node，macOS Arm64。独立预期来自测试端口替身或 Python hashlib/实际文件；未用生产生成器制造预期，驱动崩溃、超时和未启动均不冒充成功。

独立最终复核已检查 7 项宿主、16 项真实回归及完整检查日志，未发现剩余阻塞。

交付于 [PR #6](https://github.com/changyue1053/MadB/pull/6)，基于 PR #5；实现与独立测试提交为 ee74bd0、b6082a6。Linux 远端结果以该 PR 最终 head 的两项 CI 检查为准；本地检查不能代替该证据。MadB-Lab 未修改，没有执行新的原生 SDK/设备认证。C04 保持 in_progress/running；下一步为可执行任务绑定与结果验证，再按计划补三层事务、完整依赖追踪/缓存与 generate --check。
