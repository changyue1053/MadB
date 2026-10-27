# 工程运行独审补充 001：工具恢复与公开 SDK 产物拒绝

本补充承接 `engineering-runtime-independent-review.md`，不覆盖其原始结论与证据绑定。实际读取作者新保存的两个目录原件及三个脚本，执行独立只读检查 `engineering-runtime-independent-review-delta-001.py`；7 个实际 CLI 结果、53 个文件绑定及下面的字节/stat/索引断言通过。

候选仍为 `0007f8a55510037d055f59411ba9e879b421a02c`，工具 SHA-256 `8938727e5cee5abf7e06f8005f47632bcece3681dc7112be7c7607c168140a36`。没有新修复候选，也没有把源增长 exit 2 缺陷改判通过。

## 新工具恢复原件

`engineering-install-e2e-002/observations`：

- 已有正确镜像：真实 install exit 0；before/after 原始 device、inode、mode、size、SHA 完全相等（inode 694397211）。
- 错误镜像：实际 Wrapper exit 4、install exit 4；before/after stat 和 SHA 完全相等。保存的 `wrong-image.bytes` 实际为 41 字节且与该 SHA 一致，证明该次拒绝没有覆盖 fixture。
- 缺失镜像：before 明确记录不存在，实际 install exit 0；after 与本审读取的现场 stat/字节一致，精确恢复候选工具（新 inode 694397254）。未创建 `.madb/build/attempts`。

这些是新独占 scratch 工程的真实重放，补齐这一安装场景的原始 before/after 证据；不重建或篡改安装001当时仅汇总布尔的历史观察。

## 旧完整批次的 SDK 产物完整性

`engineering-sdk-asset-e2e-001/observations` 使用作者自有完整工程副本，针对上一成功结果中的公开 `libreading_demo.dylib`：

- 在副本中仅翻转首字节、长度不变。独立从原库计算翻转后的摘要，等于 wrong-before/after 的实际记录。真实 project build 在 capture 阶段返回 STORE003/exit 4，未发布、无 result、无新 attempt。
- 移走副本库后实际返回 HOST003/exit 3，同样无新 attempt。首次移除因只读父目录而失败的 harness 原件保留；002 只临时修改私有 fixture 父目录权限后执行真实缺失场景，随后恢复。
- 本审逐字核验原成功结果与恢复后的副本各 19 公开输出，二者均与原报告一致；原库与副本 inode 不同。当前索引 SHA 与 before/wrong-after/missing-after 一致，attempt 集合未增加，副本库及其父目录 mode 已恢复。

这里只补证“已发布 SDK 产物损坏/缺失时旧完整结果不能被接受”。它不证明安装 Xcode/系统 SDK 实体错身份/缺失的准入，不能替代主审明确保留的该缺项，也不构成工程迁移支持承诺。

## 仍须处理

主审的源增长 Conflict 分类缺陷、安装 SDK 实体准入缺项、外层空 PATH 单列原件、对应范围内其它缺项及最终集成候选复验仍保留。I 链、IDE、P09 和发布结论没有变化。

证据索引：`engineering-runtime-independent-review-delta-001-binding.json`。只读命令：`python3 /private/tmp/madb-chief-round12/engineering-runtime-independent-review-delta-001.py`。
