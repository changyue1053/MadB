# ADR-0019 工程初始化的四项增量契约

状态：项目所有者已确认采用四项修订；完整模板与 InitResult 格式尚须定稿。日期：2026-10-26（America/New_York）。关联 I01；不改变其阶段、依赖及整包验收条件。

## 决定与依据

项目所有者明确选择“采用四项修订（推荐）”。具体差异见[提案](../reviews/project-init-decision.md)，独立意见见[最终审查](../reviews/project-init-decision-review.md)。审查绑定原提案 SHA-256 `105405575e121c861f81e56651a4bea76c47e95417f5e7ef032714eb45141e43`；58 项设计片段、19 个来源 pin 与 6 项独立边界观察只证明该设计子集。

1. 请求摘要完整度不决定错误类别。`request_digest:null` 表示尚不能形成完整投影，合法请求在模板读取阶段受阻也可为空。实际错误沿现有 InvalidInput/Host/Conflict/CheckFailed 映射；一旦已形成摘要，后续失败保留。请求展示对象与完整摘要投影分别判断，null 有具体诊断。
2. 普通成功必须有真实、同根同计划的无覆盖 CommitNew，以及所选配方独立要求的全部检查、请求的 open 和清理观察。`--verify structure` 仍实际创建；提交后失败保留 created 并非零退出。dry-run 由冻结请求决定，保持 not-created、空 checks、不下载或启动，不能用纯计划冒充普通成功。
3. 未采用 InitResult 草案的 template 对象增加必需 `input_root:Path`，明确报告证据根下保存的原模板输入根。`template.manifest` 与已选 `boundary_profiles` 的外层 FileRef 添加该前缀；logs/attachments 仍直接相对报告证据根。内部已冻结文档原字节、逻辑根和包限定路径保持；嵌套 manifest 的父目录不是包根。不能保留单根闭包时拒绝准入，common FileRef 不变。删除模板宿主前提的重复 `requirements.*.sdks`，其对象为 `{hosts,tools,ide_entries}`；SDK 仍由 profile.sdk 及固定有限配方实际检查，composition 的 SDK 资产与运行依赖归属不变。未知 SDK 描述或配方不准执行。
4. 显式 template profile 且省略 targets，采用该 profile 完整平台族集合。都省略则使用唯一 default_profile；只给 targets 时先拒绝空/重复，再完整集合精确匹配，多个匹配仅以明确 default_profile 消歧；两者都给则必须相等。任何显式 targets（包括同时给 profile）均先拒绝空列表和重复项。记录解析后有序唯一 family，不裁剪、不补 ABI、不制造新组合。

## 实施边界

上述决定只批准四项增量，可落实相应内部纯规则和到现有 ProjectPlan 的适配，不直接批准完整 `madb.project-template/1`、`madb.init-result/1` 的生产 schema/decoder。包摘要的精确字节域和完整包范围、获取锁/重验凭据、首批 recipe/renderer/版本比较表（含 JSON 数字子集）及剩余 envelope 字段须按同一格式流程定稿后再准入。

ADR-0018 已采用的 profile/2 保持；解码支持与真实组合执行仍分开。ProjectPlan 继续是纯计划，不成为公开 wire 或提交许可。C06/C07 原结论、CLI 归属、公共 API 和独立 Lab 边界不变；本决定不证明 init、IDE、平台构建或整包验收完成。历史提案与审查保留批准前结论，实时任务状态只在 catalog 维护。
