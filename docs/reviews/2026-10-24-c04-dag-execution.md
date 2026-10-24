# C04 静态串行 DAG 与固定结果传递

任务：C04 DAG 执行子范围。基线 `cf199f80456414345a9e56a48f11854912084663`，分支 `codex/c04-dag-execution`；设计 0.9、模块/生成规范、DEC-04/05/06 与 CRIT-06/09/10 的适用前置。PR #7 已通过双 CI，未合并。

范围：app 的执行图、输入来源分类、串行调度、图级取消与资源预算；contracts 的同上下文图绑定；host 的私有图目录、节点执行与 owner 保留；独立端口及真实多进程验收，同步规范/计划/HTML。不做并行调度、动态任务/未知哈希产物发现、跨运行结果复用、事务/current/CAS、缓存、完整依赖闭包、E02 或正式 build/generate CLI，不计 C04 整体验收。

实施决定：

- 保留既有 BuildTask schema、task_digest 与来源链/批次语义。当前执行子集要求输入文件大小/摘要在绑定时已知；未知产物先生成、取得真实身份后再绑定下一张图，不在执行中静默修改请求或摘要。
- 同一张受检图共用 project/request/IR/profile/lock，上下文与工具 image 校验复用单任务实现。单任务入口仍拒绝非空 dependencies；只有图入口协调节点执行。静态规划与具体配方保持分开。
- 全图先验证唯一 output owner、路径前缀别名/祖先冲突、缺失/重复依赖及循环，再选择 named target 的依赖闭包。输入来源在完整图上确定：匹配输出路径时必须声明该 producer 为直接依赖；否则拒绝，不能因 producer 未选中、失败或字节不符而回退 source。
- 命名子图仍经过完整图静态校验，但运行时结果预期只取所选消费者；单独执行 producer 不受未选消费者的产物预期约束。完整执行则在 producer 接纳时提前拒绝与所选消费者不匹配的字节。
- 不属于任何声明输出的外部 source 在首节点前按合并清单一次捕获；同路径预期不同直接拒绝。选中子图不读取无关 source。节点只从已捕获 source 和直接依赖的 outputs 组合输入，不从活跃 work 目录或日志提取文件。
- 复用确定性拓扑序，串行且 fail-fast：任一节点准备、执行、校验或预算失败后不再启动后续节点；保留已启动/部分准备的 owner 和目录。节点历史观察不改写为从未成功，但失败/取消不对外暴露图结果集。
- GraphControl 用一个短锁协调图准入、active 节点控制器注册、取消和最终接受。每节点独立 ExecutionControl；锁内只进行状态和原子取消操作，所有读取、进程和校验在锁外。预取消、节点之间、子节点已完成但图未结束时取消都由图状态决定，不透传子节点 AlreadyFinished。错误/panic 通过 guard 结束图准入。
- 首批执行图至多 64 个节点；绑定文档总量有界。根 source 与所有选中节点的输入副本预先计费，加上累计捕获 outputs/logs 的文件字节总量最多 256 MiB；每节点仍满足既有 128 MiB/文件数等限制。剩余结果预算在读取结果文件前传入并检查，不能靠事后累计容忍过量捕获。原始管道/任务文档另受进程及文档预算限制；不声称限制工具 RSS、任意磁盘写入或网络。
- 图运行目录独占创建，节点使用固定索引目录名，不把任意任务 Id 当路径。目录不自动删除；结果值从节点 owner 中移交给图，取消/失败清空图结果时也不会通过节点 value 泄露。pending child 的 owner 继续由节点持有，poll 不改变图终态。

验收：真实 producer→consumer 和分叉/汇合、命名子图、固定源与输出、错摘要/上游失败/坏结果/准备失败不启动下游、缺边及同名旧文件拒绝、日志不作依赖结果、路径别名与预算；图预取消/运行/校验/节点间/最终接受的确定交错；失败后 owner/目录保留；独立审查、完整门禁与最终 head Linux CI。

交付：[PR #8](https://github.com/changyue1053/MadB/pull/8)，基于 PR #7；代码提交 `6f68accfb37446cc4bd027eafbfe22bd04d6621d`。最终 head 及 CI 以 PR 记录为准，本次未合并。

## 实际变更与审查

| 落点 | 本批交付 |
|---|---|
| app / plan | ExecutionGraph、GraphControl、GraphExecutor/NodeRun 端口、固定输入路由、串行执行和结果准入；读取前剩余预算；修复输出父目录大小写别名 |
| contracts | bind_graph/TaskBinding，单份受检上下文与共享 schema registry；保持原单任务接口和 wire schema |
| host | 复用独占目录分配；LocalGraphExecutor/LocalNodeRun 保留图/节点现场；私有冻结输入准备入口和结果移交，公开单任务入口拒绝 dependencies |
| 验收 | 22 项纯 DAG、图控制与预算负例；17 项独立真实多进程测试及 test-only dag_probe；绑定、目录和既有单任务回归 |
| 文档 / 计划 | 架构 02/07、实施计划、权威 catalog、README/审查索引；从 catalog 重生成 13 份 HTML；未改变工作包状态或认证范围 |

独立只读审查未发现遗留阻断问题；复核重点为取消短锁、owner 与结果分离、直接依赖来源、完整图/命名子图边界和读取前预算。补充了源捕获期间及节点准备期间取消的确定交错。命名 producer 的新增回归先确认旧行为受未选消费者约束，再验证收敛后的行为；完整执行错摘要仍在 producer 接纳时拒绝，测试同时断言 DAG003 和下游未启动。没有放宽负例、修改既有 schema 或新增依赖。

## 验证结果

- `.venv/bin/python scripts/check.py`：完整通过；Rust 141 项，Python 127 项，共 268 项。Python 分组为 contract 5、C03 6、queries 13、process 19、inputs 16、tasks 21、DAG 17、architecture 9、docs 21。
- 同一门禁完成 fmt、workspace check、all-targets/all-features Clippy `-D warnings`、隔离 Release CLI 安装、Node 规范化向量和依赖反向负例。
- 文档检查通过：95 份 Markdown、52 份 JSON、13 份 HTML。DOM 替身覆盖 29 配置、173 图过滤、87 时序场景及 3044 键盘节点选择；未执行真实浏览器渲染，本次没有修改展示模板或交互代码。
- 本地环境为项目锁定的 macOS Arm64 工具链。Linux CI 由关联 PR 的最终 head 验证，不以本地结果冒充远端执行；本批不新增原生 SDK、设备或平台认证。

后续仍按 C04 完成完整依赖发现与重绑定、三层提交、索引/CAS 与固定批次消费、崩溃恢复和缓存/`generate --check`。本记录只覆盖静态串行 DAG 子范围，不授予当前结果提交或发布资格。
