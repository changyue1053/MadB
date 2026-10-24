# C04 静态 DAG 真实执行验收

运行：`.venv/bin/python -m unittest discover -s tests/integration/c04_dag -v`。

独立 Python 投影生成 wire 输入和已知期望文件字节；Rust `dag_probe` 仅组合 contracts/app/host，不替测试计算期望。真实 Python 子进程读取冻结 inputs、写 outputs/logs，并返回 BuildEvidence。

覆盖串行 producer→consumer、分叉/汇合、命名子图、外部源一次冻结、上游 work 变化、缺依赖与同名旧源、不匹配摘要、日志不能充当依赖输出、失败/坏结果停止下游、目录冲突/替换，以及预取消、节点间取消、运行取消。失败或取消保留原始节点观察和目录，节点 owner 的结果值必须已被移交。

节点间测试通过测试端口包装器在真实节点返回的确定窗口修改文件或取消；运行取消等待 helper 的 ready 握手，超时只作 watchdog。测试清理仅在确认 PID 与唯一 marker 对应且进程停止后进行。此套件不覆盖事务、current/CAS、动态未知摘要、并行调度或 SDK 认证。
