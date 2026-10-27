# I02 doctor 实施交付（待干净候选独审/最终本地验收）

已按批准提案 60bdba6ebb41197a4d7623dcdfe4d22a150126621f746b195675cd641c96c17d 实施。产品源码现已停写；未提交、push、发布或改原产品/Lab。32 个 owned 文件的逐项 SHA256/size 见 doctor-candidate.json（清单自身 SHA256 a4eafbe997d266eb9baaf9eb9287e18c972744b3f9d5d5ac960826f17b00ac92）。协议 schema、格式向量、ADR/catalog/文档由主协调持有，不包含在该清单内。

## 实际内容

- CLI 新 doctor 及完整参数错误机器分发；默认 build 仅选择静态前提集合。帮助/版本文本和 stdout 故障例外保持。无 child process、写证据、缓存或安装入口。
- app 复用 query 的有界 Reads；完整 SnapshotAdapter.check 后才写 configuration passed。缺源/快照准确 META001，失败后也复核已捕获材料。只读 root 允许 `/`，独立于 init 的“新输出目录”值。
- 独立必需集合、真实错误精确最大值、固定阶段/依赖后 ID 排序；晚期重验失败保留已完成阶段顺序。未知配方 unsupported 与元数据/IO失败分开。
- 输出先计算规范 JSON 与末尾 LF 的总字节，再检查完整结果或 DOCTOR001 有界摘要；Conflict 原码/分类保留。公共输出仅观察值，不可转成执行许可。
- Host 用 descriptor/NOFOLLOW 读取，明确 IO→Host；捕获根与输入祖先/文件身份。安装观察复用原完整验证函数，部分事实不暴露 InstalledValuesTools。工具/SDK/有限资源的实际字节、目录身份、缺项与声明 alias 前后观察，返回前再次复核；合法 Python loader alias 只经锁定 physical image 与完整 library_links 读取。
- DOCTOR001 新解释条目及 ExplainAdapter 的明确闭集注册；原 51 项保持，新增后 52。此前仅改 JSON 导致闭集拒绝的失败已保留并修复必要注册。

## 定向证据

完整命令及 exit 见 doctor-final-v2.json（全部 0）：

- contracts doctor 8，doctor_query 4，原 query_races 6，原 explain 9。
- Host doctor 根/权限/替换 3；installation 原验证 4 + 观察 4。含同字节文件在 read/verify 与身份复核之间替换的受控窗口，精确 Conflict。
- CLI 单元 17（含原 init/执行预算/取消回归），doctor 真实 CLI 3：机器错误 JSON、无工具/写入的缺配置路径、帮助/版本例外。
- madb-cli / madb-host / madb-contracts all-target Clippy -D warnings。

测试临时目录改用系统临时根 canonical 路径，真实加载镜像的 CLI 两组明确 macOS 适用；帮助/版本不受该限定。此最终测试修订的 Host 3 + installation 8 + CLI 3 + Clippy 均再次通过，命令/exit 在 doctor-final-v3.json。git diff --check 通过。原始失败日志（编译/Clippy、003 多 filter 无效调用、Explain 闭集遗漏）均留存，不列为通过。

开发期曾以空 PATH 对上一批真实原冻结工程做 structure 只读探针，exit0：doctor-live-structure-001.json。这是当时开发镜像的实际结果，早于最终排序修订，不能替代本清单/正式候选。未给脏镜像注入伪 clean source revision，未称 build/run 正式预检通过。

## 主协调下一步

1. e02_review_final 对 32 文件最终清单完成独立源码审查；我已停写和释放 cargo target。
2. 与 root-owned schema/文档一并选择提交；主协调从干净 HEAD 注入真实 MADB_SOURCE_REVISION 重建并冻结 main。
3. 独立以该 main 执行 structure/build/run、空/污染 PATH、实际权限/漂移、只读树守恒及 JSON 截止边界。可复制的实际上下文源为 `/private/tmp/madb-chief-round10/review-cli-001/default-build/cwd/最终 中文 工程/.madb/build/native`，参数 `--project project.json --request read.json --snapshot contract.json`；独审脚本由 e02_review_final 持有。
4. 完整所需门禁与状态由 root 集成/判定；I02 整包、模板选择式 doctor、Wrapper、其他宿主/IDE、SDK运行认证未因本首片解锁。

资源读取遵循受信本地 FS 的有限字节/条目预算，是冲突检测，不是对同权限恶意写者的原子快照、OS sandbox 或硬 wall-clock deadline。C06/C07既有状态与旧报告未改；固定安装资源原字节未修改。
