# E03 集成后复验

从实际产品根 `/Users/changyue/Desktop/MadB/madb`、干净 `e26a73b6bf8cbd9c6ffded3240ebb6f9e49a09c4` 只读复验，使用新的 `frozen-tools-001`（receipt `f312f465260147733d6cf3f802d72819f14b653bb2b60b817fa81c67e8a26977`），未使用先前fe候选镜像。

- E03统一 **31项通过**：3条真实Reader→Generator Source成功链、25项一致重pin metadata拒绝、1项重复key投影拒绝、2次真实Reader拒绝。后两次均完整本地清理，未启动Generator，未提交Source，旧索引不变。
- 旧本地配方`@1`普通/增强/禁用 **3项通过**，原5/8/5责任观察与15/17/15源码产物保持；wire仍是现行composition/1与profile/2。
- 两套运行前后实际HEAD、1401个tracked文件的字节/模式、全部13正式镜像及receipt、Python、工具观察一致；镜像sha/size/path逐项对照正式receipt。文件数量来自integration-madb-code.json。

原始结果分别位于 `post-integration-e03-001` 和 `post-integration-e03-legacy-002`。最初legacy001因执行参数把用户目录拼成`changue`，在git前置检查即FileNotFoundError/exit1，未启动业务进程；其空输出目录与失败日志原样保留，正确路径在新002目录完成。

本轮无产品、catalog或历史报告修改，无commit/push。全仓集成门禁由主协调单独绑定；这些结果不外推native/SDK/HAR、目标生命周期、页面、替代后端或发布。精确摘要、实际起止时间、原始进程及清理证据见同名JSON。
