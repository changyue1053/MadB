# C04 单任务执行与固定结果字节

任务：C04 单任务执行/结果验证子范围。基线 `497b5016b695e1694914b987e0d23e3ac644f461`，分支 `codex/c04-task-results`；依据设计 0.9、模块/生成规范、既有 BuildTask/BuildEvidence、DEC-04/05/06 和 CRIT-06/09/10 的相关前置。PR #6 已通过双 CI，未合并。

范围：contracts 的受检任务投影；app 的结果验证准入和不可变输出值；host 的单任务准备/执行与精确输出树检查；独立端口、实际配方与失败回归。同步架构/计划与生成视图。不执行 DAG，不实现 current 提交、三层事务、缓存、完整依赖闭包、E02 或正式 build/generate CLI，不计 C04 整体验收。

实施决定：

- 只执行依赖列表为空且 working_directory 为 work 的显式单任务配方；参数不自动改写，配方以 ../inputs 读取固定副本。任务保留全部执行字段，不从静态 TaskGraph 反推。
- 配方绑定既有受检 project/request/IR/profile/lock；task_digest 复用既有算法。input_digest 保留来源链语义，匹配受检 IR.source_digest，不改成 task.inputs 清单摘要；每个 task.inputs 的真实字节仍独立捕获，完整清单已参与 task_digest。generation_id 仅绑定请求声称消费的既有批次，不计算成本次未来输出批次；其批次提交/来源链证明属于后续事务，不将本批结果命名 ready。
- 首批工具仅绑定所选 profile.tools 中被 lock 完整锁定的工具 image，由宿主注入安装后相对路径/大小/字节摘要；不把组件包或组件清单摘要误当可执行 image。
- 本批支持受控一次性配方：stdin 传既有 BuildTask，stdout 返回既有 BuildEvidence，outputs/logs 位于 work。普通编译器由配方适配，不要求原生编译器直接输出 MadB JSON；这不是 E02 帧协议，也不新增 wire schema。
- 真实进程 exit、诊断、pending owner、管道收尾先决定是否允许验证；helper 自报 passed 不替代实际观察。随后校验 payload 与任务身份、输出清单、真实文件/日志大小和摘要、精确目录集合，并捕获不可变结果字节。
- 结果验证及副本最终复核都在一次性取消/结果 gate 关闭前进行。验证失败保留原始 ProcessRun/目录，取消胜出不暴露已捕获结果；迟到取消不能回滚已接受的观察。准备与执行阶段保持分开，准备失败不启动进程。
- 私有运行目录持续保留，无自动 Drop 删除或 GC。对外固定的是已捕获字节，不是声称工作目录以后不变；只读权限与前后复核不是安全沙箱、完整进程树收尾或 ABA 证明。

验收：实际正常配方与独立字节 oracle；错上下文/工具/身份、未满足依赖、参数环境和输出路径冲突；零退出但坏 payload/缺失/多余/错摘要/链接/FIFO/超预算输出拒收；屏障取消验证窗口；捕获后原产物修改不改变结果；原始 owner/目录保留；独立复核、完整仓库检查及最终 head Linux CI。

## 交付与实际验收

- contracts/task 复用现有上下文、task digest 与 BuildEvidence 配对，输出稳定的 BuildRecipe；query 仅增加 crate 内复用可见性，原只读行为不变。
- app/process 将校验纳入同一次 gate，错误保留原始观察、取消丢弃结果；app/task 共享路径前缀/预算检查并捕获输出与日志字节。
- host/task 组合准备与执行，复用进程监督、私有目录和精确集合检查；验证后再检查输入及 work 目录身份。RetainedRun 泛型携带受检值，原 API 通过默认类型兼容。
- 真实驱动位于 CLI examples，保持 host 不依赖 contracts。只新增对工作区已锁定 serde_json 的 CLI 开发依赖，无新外部包或版本升级。

独立复核没有发现阻塞。已将新验证窗口测试的无限 Barrier 改为带 5 秒 watchdog 的进入/释放/完成通道，保留确定次序。复核同时确认 profile.tools.arguments 记录工具配置而非通用强制前缀；任务 argv 完整执行，不做隐式拼接，规范已澄清。未修改旧 schema 或黄金预期。

| 实际执行 | 结果与范围 |
|---|---|
| `cargo fmt --all -- --check`、workspace check、Clippy all-targets/all-features `-D warnings` | 全部通过 |
| Rust workspace 测试 | 106 项通过：新增任务准入 7、结果捕获 8、契约绑定 15 |
| Python 各套件 | 110 项通过：新增真实单任务 21 项，其余既有契约、查询、进程、输入、架构及文档回归继续通过 |
| 完整 `.venv/bin/python scripts/check.py` | 216 项 Rust/Python 测试通过，新增 51；另含隔离 Release CLI 安装、Node 摘要向量、依赖检查及 HTML DOM 检查 |
| 文档与视图 | 94 Markdown、52 JSON、13 HTML 静态检查通过；由 catalog 生成视图，没有执行浏览器布局验证 |
| 真实正反例 | 独立配方实际读固定输入并产生输出/日志；错身份、坏 payload、缺失/额外/坏摘要/链接/FIFO/预算、验证内取消、输入改动/work 替换、捕获后磁盘改写均通过 |

本地环境为项目锁定 Rust/Python/Node、macOS Arm64。Python 独立生成控制测试输入和预期；保留合成 profile 的目标身份，不声称执行了该目标的原生 SDK。失败、非零退出或未运行不能作为成功证据。Linux 远端最终 head 检查在 PR 发布后独立核实。

MadB-Lab 未修改，未新增 SDK 或设备认证。C04 仍为 in_progress/running；后续先接 DAG、依赖满足及结果传递，再完成三层事务、批次来源/索引/CAS、完整依赖闭包/缓存和 generate --check。
