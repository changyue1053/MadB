# MadB 历史研究索引

九组研究已归入 docs/references/legacy-boundaries。本页索引经过 0.9 对照修订的阅读版；原始 18 份文档和旧映射在[封存快照](legacy-boundaries/snapshots/2026-10-24-before-refresh.zip)中按原字节保存。使用边界与共同规则见[归档入口](legacy-boundaries/README.md)。

当前开发以[文档首页](../README.md)链接的架构、ADR 和计划为准。236 条来源问题的身份与公共问题映射未变；历史样本不表示每条都发生过线上事故，也不证明 MadB 已完成实现或认证。

| 语言组 | 问题数 | 问题清单 | 研究整理 |
|---|---|---|---|
| ArkTS_C_CPP | 30 | [清单](legacy-boundaries/ArkTS_C_CPP/ArkTS_C_CPP_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/ArkTS_C_CPP/ArkTS_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| ArkTS_Rust | 30 | [清单](legacy-boundaries/ArkTS_Rust/ArkTS_Rust_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/ArkTS_Rust/ArkTS_Rust_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_ArkTS | 28 | [清单](legacy-boundaries/Kotlin_ArkTS/Kotlin_ArkTS_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/Kotlin_ArkTS/Kotlin_ArkTS_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_C_CPP | 25 | [清单](legacy-boundaries/Kotlin_C_CPP/Kotlin_C_CPP_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/Kotlin_C_CPP/Kotlin_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_Rust | 26 | [清单](legacy-boundaries/Kotlin_Rust/Kotlin_Rust_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/Kotlin_Rust/Kotlin_Rust_架构总结与新版_mad-boundary_设计输入.md) |
| Kotlin_Swift | 24 | [清单](legacy-boundaries/Kotlin_Swift/Kotlin_Swift_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/Kotlin_Swift/Kotlin_Native_Swift_架构总结与新版_mad-boundary_设计输入.md) |
| Rust_C_CPP | 23 | [清单](legacy-boundaries/Rust_C_CPP/Rust_C_CPP_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/Rust_C_CPP/Rust_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| Swift_C_CPP | 24 | [清单](legacy-boundaries/Swift_C_CPP/Swift_C_CPP_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/Swift_C_CPP/Swift_C_CPP_架构总结与新版_mad-boundary_设计输入.md) |
| Swift_Rust | 26 | [清单](legacy-boundaries/Swift_Rust/Swift_Rust_跨语言边界工程清单.md) | [设计对照](legacy-boundaries/Swift_Rust/Swift_Rust_架构总结与新版_mad-boundary_设计输入.md) |

[逐条映射](../catalog/source-coverage.json)定位当前阅读版的路径、行号和摘要；其中 archive_snapshot 定位封存输入。快照中的 source-coverage.json 保存旧路径、旧行号、旧摘要和旧映射，可独立比较修订是否改变问题身份。

本机源码链接不作为日常阅读的前提；需要核对原研究观察时查快照，再按对应仓库版本定位。通用回归必须在 MadB-Lab 重新构造 fixture、独立预期及真实消费者；不得把旧项目的间接调用结果当作新直连路线的运行证据。

历次修改及当时检查见[复核归档](../reviews/README.md)。阅读版修订与封存输入分别管理，旧报告保持原字节。
