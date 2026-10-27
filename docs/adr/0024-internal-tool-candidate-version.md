# ADR-0024 内部工具候选真实版本 0.1.0

项目所有者已选择“采用内部候选 0.1.0（推荐）”。本决定承接 [ADR-0023](0023-project-template-and-init-result.md) 的版本准入差异，不改变模板规则或任何路线认证。

## 差异与采用依据

将工具 workspace 的真实 package version 从 `0.1.0-dev.0` 改为 `0.1.0`，重新构建九个工具 crate。CLI 版本由实际 Cargo package 导出；同次构建的 MadB/finalizer 以 Cargo package/artifact 观察、源码及锁输入、实际镜像摘要绑定。不得裁剪开发后缀、替换版本输出或给旧镜像贴新版本。

原具体提案 SHA-256 为 `d9921cdf2d5dec40f8ec878060d7677dc918c92a8672ec2f40a9a7367ce8b0ee`，独审原文为 `098875564ef3eb1875e6b0bba9adc5fad90acf0317ce2cbfc24f97f057179f48`；原件随[第九批记录](../reviews/2026-10-26-chief-architect-ninth-batch.md)归档。用户批准发生在提案及独审完成之后。

## 兼容与责任边界

- 所有工具继续 `publish=false`。不创建标签、发行包或公开版本发布；阶段源码推送沿用用户授权且不得启动 GitHub Actions。
- 独立 SDK workspace、ABI、公开协议格式、模板兼容范围、历史设计向量和旧冻结候选不变。C06/C07/L02 的历史结论仍只覆盖各自原候选。
- 版本调整本身不升级 registry 依赖；本批初始化/扩展组合根新增的本地依赖单独按模块规范核对。
- 开发版、预发布和 build metadata 的模板拒绝测试保留。真实正式版本只满足版本前提，不能替代来源、工具、宿主、模板包、必需检查或真实执行证据。

## 检查

复核九个工具 crate 的 package/lock 版本与 `publish=false`，保留 SDK 原版本；真实构建与安装的 `madb --version` 必须一致。记录同次构建映像，检查错误 package/artifact、缺输出、替换字节和版本输出不符的拒绝路径。后续新候选重新冻结、独立复核和集成验证；未执行项不得标为通过。
