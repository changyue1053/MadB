# E03 第十一批最终来源语义验证

最终候选为干净 `fe75241697469292d7af06ff6bb4e1ce47cc4018`，源码位于 `extensions-source`，实际三工具来自同次 `frozen-extensions-001` 的13镜像构建。已核实际执行前后所有tracked源码、三镜像、receipt、Python和工具观察字节不变；并行doctor未混入。精确路径/摘要与原始日志见同名JSON。

统一检查实际 **31项通过**：3条真实公开Reader→Generator Source成功链；25项一致重pin后的metadata/语义拒绝；1项重复责任key在语义投影阶段准确HASH004早拒；2次真实Reader拒绝，均完整本地清理、未启动Generator、旧Source索引与字节保留。直接metadata反例复用真实Reader声明形状，不冒称又执行了声明提取。

新有限@2普通/禁用覆盖16call、95责任、5资产、5直接依赖、15产物；增强覆盖107责任、7资产、10依赖、17产物。责任来自固定policy/实际IR和19个机制源，而不是申请者责任数组；明确区分provider释放、Core ticket退休、native context、JS listener与ArkUI snapshot。缺行、owner/scope错配、API/ABI/runtime放宽、帧布局互换、校验容量变化、可选源偷渡与能力前提弱化均准确拒绝。`physical-assembly`不属于本source配方，额外claim被拒绝，不生成虚构装配owner。

旧@1普通/增强/禁用另以同候选真实双会话复验，**3项通过**，原5/8/5观察范围与15/17/15产物保持。这里@1指本地配方版本；wire仍为现有composition/1与profile/2，不表示旧Profile1接受新语义。旧Profile1隔离与模型/证据演进由6项定向测试覆盖，另5项有限准入测试覆盖23拒绝分支。

本报告只记录作者实际验证。共用格式向量与全仓门禁由主协调绑定；替代后端事实/准确未准入仍接已有E02/E01证据，不声称执行X后端。独立收包审查与E03整包状态由主协调决定。未执行native/SDK/HAR装配、目标生命周期或页面，不继承主线认证；未改旧报告、catalog或发布。
