# 扩展作者的进程入口

`madb_protocol::extension_author` 提供既有 v1 的有界读写、握手、独立协议/IR 版本交集、会话身份、取消事件和终态诊断。作者只依赖公开 `madb-protocol`，不导入工具内部 app、contracts 或 host。`extension_peer.rs` 是可运行的协议测试助手；其 BuildEvidence 是固定测试数据，没有调用编译器。

当前宿主策略为每组件、每进程一个请求。按以下顺序使用支持库：

1. 从当前包读取精确清单字节，用 `AuthorSession::from_manifest(bytes, component_id)` 创建会话。不要重新序列化清单后计算身份。
2. 用 `read_frame` 从 stdin 读取完整帧，交给 `receive`。收到 `AuthorEvent::Reply` 后用 `write_frame` 写回 ack 或 reject；stdout 仅承载控制帧，日志写 stderr 或声明的日志文件。
3. 收到 `AuthorEvent::Invoke` 后，验证公开端口 payload 及其引用的实际文件字节，再执行端口业务。相对输入在 `inputs/` 域；输入 payload 内的路径相对此域。输出只写入请求的 `output_root`，输出 payload 内的路径相对此输出域。
4. 业务运行时仍须继续接收 stdin。`AuthorEvent::Cancel` 是停止请求；作者负责将其传递给正在执行的业务并等待派生工作、输出写入和清理结束，再调用 `cancelled()`。同步业务不能阻塞控制读取到结束后才处理取消。库不代替业务停止机制，也不证明原生资源已释放。
5. 成功时，先完成并关闭全部输出文件，计算响应文档的真实 size/sha256，再调用 `complete(FileRef)`。业务失败用 `failed`，传入既有 code、message、stage 和可选 source；不放入凭证或环境变量值。三个方法均返回需 `write_frame` 发送的终态帧，一个请求只能有一个终态。
6. 发出终态后不再修改输出，完成清理并退出。宿主关闭输入；本策略不保证跨请求复用进程。

`read_frame` 和 `ExtensionFrame` 共同执行现行 schema、重复键、深度及 1 MiB 帧上限检查。完整结果仍要按对应公开 payload schema 和语义身份校验，控制帧 `ok` 不表示构建、验证或发布通过。宿主独立复核清单、身份、能力、版本、文件闭包和输出，并在终态捕获、退出/EOF/清理后复核同一快照，最后执行取消准入。使用官方作者支持不会使宿主信任扩展自报结果。

测试构建、真实负例和当前限制见仓库 `tests/integration/e02_session/README.md`。组合配置、精确包锁、公开包交付和平台验收有各自的完成条件，本示例不授予这些结论。
