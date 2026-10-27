# E04 Lab 正式002独立消费审查

结论：正式002在下列精确候选和环境上的**固定 macOS Arm64 资源首片**通过独立复核：5个公共case、16次JIT/AOT目标调用、22条实际命令日志、31个唯一报告附件相互闭合，report/1与五个case/1实例schema通过。未发现本范围阻断项。该结论不等于完整E04、第二业务、全部EXT条件或主线认证/发布通过；正式001的失败和此前P2首审保持原件。

## 绑定来源

- 实际Lab：`/Users/changyue/Desktop/MadB/madb_lab`，干净HEAD `ac12f847a46ca9c8dba6f79dcc1b2db993fb6c37`。运行前、运行后与根freeze002的565个文件字节/mode一致；本审再次读取当前实际文件和Git状态核对。
- producer revision：`bf478767273069c47798bc04140185c671d93957`。
- 公开候选：`/private/tmp/madb-chief-round12/e04-formal-producer-001/pipeline/package-run/candidate`，digest `sha256:ca5f71c451be9cfb408501ae2017daf3ec773b6ae23949ea4a6d27a35fe2b47c`。本审按公开投影独立重算candidate摘要，并核127文件完整库存、原候选及隔离副本原字节。
- Dart 3.13.7 stable，实际工具版本日志与镜像SHA一致；官方固定原包SHA `9f972ee1e1139cb4f845f35c7c77bef269dd2ec727b239fb17a2ebcb66d0fb2f`。1044个安装SDK文件分别与记录及ZIP原始成员字节一致。两项固定Pub依赖为ffi 2.2.0、crypto 3.0.8，54个安装文件、原包、实际解析图和`packageUri=lib/`的真实本地导入根均吻合。
- 宿主原件记录Darwin arm64、macOS 26.6.2（25G83）、Python 3.14.4。只对这一实际环境下的运行作结论。

新public report、case-inputs均显式绑定ac12；`execution-facts.json`通过public report中有SHA/size的附件绑定到同一Lab revision。其16条事实逐项对应002的实际日志、PID、argv、耗时、退出及清理，不沿用001或producer smoke。private fact本身不新增公共schema字段。

## 完成的实际业务调用

| 固定case | 实际目标调用 | 原件核对 |
|---|---:|---|
| values | JIT/AOT各1次 | 空值、0、255、混合值、65536字节上限的checksum；输入字节不变；显式关闭；同版本顺序重开256；再次关闭。 |
| ranges | JIT/AOT各1次 | 65537字节、close参数-1与4294967296准确RangeError；拒绝后owner仍open且checksum=5；随后关闭。 |
| close | JIT/AOT各1次 | 1000与4294967295毫秒的正常关闭；重复close保持closed；关闭后调用准确StateError。未将这些正常关闭扩为超时竞态证据。 |
| isolate | JIT/AOT各1次 | library和engine发送到另isolate均ArgumentError；原owner仍可计算23；显式关闭；接收isolate确认正常退出且无错误。 |
| package | JIT/AOT各4次 | 原包实际load并计算3；缺原生库、仅翻转首字节、符号链接三个自有副本分别准确missingAsset、digestMismatch、pathViolation。错误路径和原/实际摘要与受损文件逐字一致。 |

独立消费者只使用`package:madb_dart_ffi/madb_dart_ffi.dart`普通API；本审实际读取固定consumer与独立字面期望。没有引用生产者smoke来替代这些结果。逐次检查stdout为完整单个响应，case/PID与启动日志一致，字段类型和值均一致；业务观察与public结果一一对应。AOT是本次独立编译的实际可执行文件，JIT是固定Dart镜像、包配置及同一消费者源；二者路径/镜像不同且均逐字绑定。

六个准备命令包括版本、公开包`pub get --offline --enforce-lockfile`、独立消费者离线Pub获取、依赖图、analyze和AOT编译；随后为16次实际目标进程。22次命令均exit0、已回收、管道排空，捕获完整且未超时/截断。公共报告五个结果均passed/target-run/cleanup complete；每组duration_ms等于对应实际fact耗时之和取毫秒。没有把准备命令、51项观察器测试或producer编译/冒烟计成目标业务调用。

## 身份与文件守恒

- 本审独立重算五个实例case摘要和完整case catalog摘要；模板到实例仅改变预定kind、selector及实际工具环境。required_cases恰为这五项，16步顺序、模式、variant与固定case一致。
- profile/2外层摘要为 `sha256:743086e279536dddb8960b81eca2f731349ee847977e859e62aa9c3f0cdda067`，按已批准投影独立重算相符。原native identity的profile摘要仍为 `sha256:9c0ad5e4148d5b72da86642e6dd1296eb70e0920e471f199ac27e166946afa7d`。两层保持区分；本次扩展包不继承provider的主线认证。
- 127个公开候选输入、1044个SDK文件及固定原包、54个依赖文件、565个Lab源文件保持原字节。原候选完整文件集合没有新增/缺失。包内Pub生成的本机状态不被当作原候选源文件。
- 公共report、case实例、命令表、16执行fact及22日志的所有31个唯一附件引用均按真实路径、SHA、大小核验，不能仅凭JSON中的passed赋予执行证据。
- 12个本次TMPDIR私有镜像目录的36个文件在清理前逐字对应本候选native/bridge/identity，清理后目录实际为空；清理前提为全部已知进程退出。此证据是文件清理和已知进程回收，不是原生分配计数归零、动态库卸载或任意退出竞态证明。

## 独立检查方法与原件

只读审查原件：`e04-lab-formal-002/`及`e04-lab-formal-002-execution/`，根freeze002和实际集成来源。未重跑Dart、修改产品/Lab源码、变更case期望或写catalog。

本审脚本`e04-lab-formal002-independent-review-check.py`仅使用标准库和项目既有jsonschema校验库，不导入MadB/Lab实现。它依据公开摘要投影、schema、固定独立consumer和实际原件进行字节、实例、普通输出、进程/事实、清理及引用闭包检查。

实际命令：`/Users/changyue/Desktop/MadB/madb/.venv/bin/python -B /private/tmp/madb-chief-round12/e04-lab-formal002-independent-review-check.py`，exit0。独立检查原始输出保存在`e04-lab-formal002-independent-review-check.log`，绑定索引为`e04-lab-formal002-independent-review-binding.json`。实际Lab集成后51项定向测试原件也已绑定；没有用测试数量扩大业务范围。

## 保留的范围

本次未覆盖native counters、受控超时/故障排程、loader-graph故障、第二值业务、禁用/版本隔离、更多目标或完整EXT交付。资源关闭失败保留owner等未被本套正常关闭/参数拒绝场景覆盖的承诺，仍须按各自既定验收补证。没有远程Actions、发布或公开包认证动作。

正式001 local report SHA `9155a7b9df26a1f9c04536c2fd74f84f4b53821282fbea6f7ddde3d28812734c`、public report SHA `f10bdeb8b6035cbd951058519a95b40870dd8f147ca5897a15e416905e41abc0` 仍保持。001真实JIT已运行但日志解析阻断接受的事实与错误not-run历史字段，由既有首审解释，不倒填通过。002使用新Lab来源完成独立全套固定首片，因此是新的有效证据。

报告与证据绑定写完后，本审停止所有写入，供根统一归档与catalog按范围记录。
