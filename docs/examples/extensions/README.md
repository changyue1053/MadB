# MadB 扩展接入样例

这些文件用于理解和校验新增语言的接入格式。它们没有可执行适配器、真实工具锁或认证结果，不能安装为扩展。Dart 仅作为验证扩展机制的示例，不表示新增了已支持语言。

| 文件 | 用途 |
|---|---|
| [dart-ffi.extension.example.json](dart-ffi.extension.example.json) | Dart Native 消费 C ABI 的清单结构；复用 C Reader，声明自己的生成、构建、验证与打包组件 |
| [protocol-session.example.json](protocol-session.example.json) | 一次正常生成会话的 hello、ack、invoke、result |
| [schema-vectors.json](schema-vectors.json) | 20 个结构正反例，分别指向清单或控制帧 schema |

会话中的 manifest_digest 与清单样例的文件字节一致；输入、输出摘要和大小是合成数据，文件路径只展示协议写法。合法结构不保证路径对应文件存在，也不保证后端提供相应功能。

schema-vectors 的 valid 只表示对应 JSON 应通过结构校验。example=true 的清单即使结构有效，正式安装仍应拒绝；未知必需功能、同名 ID、未声明组件和状态机违规需要独立语义测试。

文档校验应完成：

1. 使用[扩展 schema](../../schemas/README.md)验证清单与每个会话帧。
2. 按 expected_schema 验证结构正反例，不把任意异常视为负例成功。
3. 核对会话双方身份、组件、摘要、版本和请求 ID 一致。
4. 核对选定组件满足 hello 的必需功能，ack 不超过该组件声明。

运行时还需要[接入验收规范](../../validation/05-language-onboarding.md)中的真实进程、竞态、编译、消费与制品测试；它们不能由这些静态样例代替。
