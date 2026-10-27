# 工程重复构建决定稿 v2：最终独立增量复审

**结论：此前阻断已闭合；此冻结 v2 可提交项目所有者作具体契约决定。** 这不是用户批准、生产实现通过、工程验收或发布许可。

绑定 manifest：`381a00bdcf104afec78a1c43a866d8374258894e7230b2e71bda1397fe89ae0c`。绑定提案：`ebd8e133eaac9898db37fb3ad4789e46b972d0e3a8adf46da95afba0e96f6b3e`。

## 最终差异

1. 提案第 111 行将 result 存在条件修为 `build && published=true`，明确包括 publish.status=error、durability=uncertain 的已发布尾窗。与现有 schema、正例及 CAS 真实事实规则一致。
2. 提案第 134 行给 install-tool 唯一接受点：新镜像核对后的 no-replace 发布，或已有匹配镜像最后复核接受；与取消共用最终准入门，接受后 late cancel 为 AlreadyFinished。镜像真实事实在持久化/回执错误中保留，未经过接受点则不能填 installed_tool。结果 schema 同步要求非 null installed_tool 时 exit_code 不是 130、install 阶段为 passed/error，并新增 result-installed-cancelled 反例。

proposal v1→v2 的逐行差异仅上述一处表格修订和一个取消段落；未顺带改变配方、模板版本、受支持目标、锁语义、来源/所有权、安装范围或旧格式。schema/向量的对应变化也仅用于上述安装尾窗约束。与既往三份审查共同阅读后，没有剩余架构或格式阻断。

## 冻结与检查绑定

- 当前 manifest 的 29 个文件和 3 份历史报告，SHA-256/size 全部实际核对相符。
- `engineering/frozen-v1` 保存的 28 个 v1 文件，逐项与原 v1 manifest 相符；旧审查保持原字节。
- `final-check-v2.json` 绑定的 23 个输入文件，SHA-256/size 全部核对相符。其实际记录有 18 项结构观察，actual 全部等于 expected；10 项既知摘要包含原 Python/OpenSSL一致观察。新增安装后取消反例实际拒绝，安装后回执失败正例仍接受。
- launcher 和 launcher-probe-002 的字节与 v1 一致，沿用原 4 项假工具启动观察；没有把它扩写成真实工程或 IDE 通过。

本复审执行的是只读文件、摘要/size核对和差异审阅，没有重新运行 validator、OpenSSL、Wrapper、安装、编译、运行或 IDE；上述执行结果来自身份匹配的已有检查记录。新增输出报告之外未修改冻结集、源码、schema、提案或历史报告。

## 决定后的实施边界

所有者决定后按本稿明确的工程线/root责任进入实现，保持 app/CLI/Host 分层、一个 C04 coordinator、独立尝试与完整结果提交。原生进程取消、工程 current CAS、工具无覆盖安装、失败恢复须对最终集成候选独立复核；数据结构样本不是实际候选授权。旧模板0.1.0/0.1.1、init/doctor、C06/C07及I01/I02/I03整包退出不因本次决定稿审查而改变。
