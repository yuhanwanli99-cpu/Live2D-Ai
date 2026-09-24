//! WS 测试端握手 / 帧读取辅助 + 「101 与首帧同批到达」确定性回归。
//!
//! **为什么单独成文件**：`tests_ws.rs` 已贴到测试文件行数上限（头注豁免 ≤1000），
//! 而修复握手丢帧需要新增缓冲客户端与两条回归；把这一块独立承载，既有文件
//! 不再增长（修复本身也不掺进 e2e 场景）。
//!
//! # flaky 根因（阶段 2）
//!
//! 失败用例：`web_api::tests_ws::ws_and_http_concurrent`，断言「WS 首帧应为
//! subscribe_ack（契约 §4.4）」，`left="heartbeat" right="subscribe_ack"`。
//!
//! 旧 `do_client_handshake` 手工循环 read 直到看见 `\r\n\r\n` 就停，把整个 head
//! 缓冲做完 101 断言后**丢弃**；但生产 `web_api/ws/connection.rs` 在 101 之后
//! **立即**写 `subscribe_ack`。当这两段落进同一个 TCP 段（loopback 上 101 头
//! 只有百余字节，紧随的小帧几乎必然同批）时，残余的 `subscribe_ack` 帧字节已经
//! 在 head 那次 read 里返回——一并丢掉后，客户端下一个 read 只能等到 10s 后的
//! heartbeat（`seq=2` 证明 `seq=1` 的 subscribe_ack 已分配并被丢弃）。
//!
//! **修复**：握手保留 `\r\n\r\n` 之后的残余（[`split_http_head`]），首帧读取
//! 先消费残余再读 socket（[`WsClient`]）。产品协议与断言语义均未改动。

use std::io::{Read as _, Write as _};
use std::net::SocketAddr;

/// 测试端 WS 客户端：`TcpStream` + 握手响应之后**同一次 TCP read** 可能一并
/// 到达的首个 WS 帧残余字节。
///
/// `pending` 优先被帧读取消费；为空时才真正落到 socket `read`。这样「101 与首帧
/// 同批」与「101 与首帧分两批」两条时序走同一条读路径，不再有丢弃窗口。
pub(super) struct WsClient {
    stream: std::net::TcpStream,
    pending: Vec<u8>,
}

impl WsClient {
    /// 残余优先 + socket 补齐，读满 `buf.len()`；EOF / 读错误直接 panic。
    fn read_exact_bytes(&mut self, buf: &mut [u8]) {
        let mut filled = 0usize;
        if !self.pending.is_empty() {
            let take = buf.len().min(self.pending.len());
            buf[..take].copy_from_slice(&self.pending[..take]);
            self.pending.drain(..take);
            filled = take;
        }
        while filled < buf.len() {
            let n = self
                .stream
                .read(&mut buf[filled..])
                .expect("read WS frame bytes");
            assert_ne!(n, 0, "unexpected EOF while reading WS frame");
            filled += n;
        }
    }

    /// 读 1 条文本 WS 帧（**仅短帧 len<126**；server→client 无 mask）。
    ///
    /// 断言与旧 `read_one_text_frame` 逐条一致——修复没有放宽任何断言。
    pub(super) fn read_text_frame(&mut self) -> String {
        let mut head = [0u8; 2];
        self.read_exact_bytes(&mut head);
        let opcode = head[0] & 0x0F;
        assert_eq!(opcode, 1, "expected text opcode, got {opcode}");
        let masked = (head[1] & 0x80) != 0;
        assert!(!masked, "server→client frame must not be masked");
        let len = (head[1] & 0x7F) as usize;
        assert!(len < 126, "this test only supports short frames (len<126)");
        let mut payload = vec![0u8; len];
        self.read_exact_bytes(&mut payload);
        String::from_utf8(payload).expect("utf-8 payload")
    }

    /// 宽松版帧读取：EOF / 超时 / 非文本 / 长帧一律回 `""`（不 panic）。
    ///
    /// 供 `lifecycle_e2e` 的 40 次连接/断开循环使用——那里允许客户端在任意
    /// 时刻断开（旧 `read_short_text` 的等价语义）。
    pub(super) fn read_text_frame_lenient(&mut self) -> String {
        let mut head = [0u8; 2];
        if !self.try_read_exact_bytes(&mut head) {
            return String::new();
        }
        let opcode = head[0] & 0x0F;
        if opcode != 1 {
            return String::new();
        }
        if (head[1] & 0x80) != 0 {
            return String::new();
        }
        let len = (head[1] & 0x7F) as usize;
        if len >= 126 {
            return String::new();
        }
        let mut payload = vec![0u8; len];
        if !self.try_read_exact_bytes(&mut payload) {
            return String::new();
        }
        String::from_utf8(payload).unwrap_or_default()
    }

    /// 残余优先的容错读满；EOF / 错误返回 `false`。
    fn try_read_exact_bytes(&mut self, buf: &mut [u8]) -> bool {
        let mut filled = 0usize;
        if !self.pending.is_empty() {
            let take = buf.len().min(self.pending.len());
            buf[..take].copy_from_slice(&self.pending[..take]);
            self.pending.drain(..take);
            filled = take;
        }
        while filled < buf.len() {
            match self.stream.read(&mut buf[filled..]) {
                Ok(0) | Err(_) => return false,
                Ok(n) => filled += n,
            }
        }
        true
    }

    pub(super) fn set_read_timeout(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub(super) fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
        self.stream.shutdown(how)
    }
}

/// 纯函数：从已累积的 HTTP 握手字节里切出 `(head, 残余)`。
///
/// 残余 = 首个 `\r\n\r\n` 之后的全部字节（可能含首个 WS 帧）；没有分隔符
/// （例如 EOF 前都没收全）→ `None`。抽成纯函数是为了让「残余字节被保留」
/// 有**确定性**单测，不依赖 TCP 分段时序。
pub(super) fn split_http_head(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = buf.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    Some((&buf[..end], &buf[end..]))
}

/// 客户端读 1 条文本 WS 帧（薄包装，保留既有调用点形状）。
pub(super) fn read_one_text_frame(client: &mut WsClient) -> String {
    client.read_text_frame()
}

/// 客户端走完 WS 握手，返回带回溯缓冲的 [`WsClient`]（持有供 caller 关闭）。
///
/// **D-P0B 2026-08-29**：WS 升级前 server 端会做 Origin 校验（同源白名单
/// = `http://127.0.0.1:<port>` / `http://localhost:<port>`），所以客户端
/// 必须在握手请求里带 `Origin: http://127.0.0.1:<port>` 头，否则 server
/// 回 403 而非 101。`addr` 是 `server_addr()` 返回的 `SocketAddr`，port
/// 字段取自真实监听端口（与 `run_request_loop` 内的 `ctx.security.port`
/// 一致——`start_server` 把 `port` 注入到 `ctx.security`）。
pub(super) fn do_client_handshake(addr: SocketAddr) -> WsClient {
    let mut client = std::net::TcpStream::connect(addr).expect("client connect");
    let key = tungstenite::handshake::client::generate_key();
    let port = addr.port();
    let req = format!(
        "GET /ws/runtime HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nOrigin: http://127.0.0.1:{port}\r\n\r\n"
    );
    client.write_all(req.as_bytes()).expect("client write req");
    client.flush().expect("client flush");
    let mut head = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = client.read(&mut tmp).expect("client read");
        if n == 0 {
            break;
        }
        head.extend_from_slice(&tmp[..n]);
        if head.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    // **关键**：`\r\n\r\n` 之后的字节是残余，可能含 101 之后立即写出的首帧
    // （subscribe_ack）。旧实现丢弃它们 → 首帧丢失 → 只能等到 10s heartbeat。
    let empty: &[u8] = &[];
    let (head_bytes, residual) = split_http_head(&head).unwrap_or((head.as_slice(), empty));
    let head_str = String::from_utf8_lossy(head_bytes);
    assert!(
        head_str.starts_with("HTTP/1.1 101"),
        "expected 101, got: {head_str}"
    );
    WsClient {
        stream: client,
        pending: residual.to_vec(),
    }
}

#[cfg(test)]
mod regression_tests {
    use super::{do_client_handshake, read_one_text_frame, split_http_head};

    /// **确定性回归（纯函数层）**：`split_http_head` 必须把 `\r\n\r\n` 之后的
    /// 字节原样交出。若有人把残余吞掉（回到 flaky 前的行为），此断言即红。
    #[test]
    fn handshake_residual_bytes_are_retained() {
        let raw = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n\x81\x03ack";
        let (head, residual) = split_http_head(raw).expect("应找到头部结束符");
        assert!(head.ends_with(b"\r\n\r\n"), "head 必须含结束符");
        assert_eq!(
            residual, b"\x81\x03ack",
            "\r\n\r\n 之后的残余字节必须保留（含首个 WS 帧）"
        );
        assert_eq!(
            split_http_head(b"HTTP/1.1 101 Switching Protocols"),
            None,
            "尚无结束符时不得消费任何字节"
        );
    }

    /// **确定性回归（端到端）**：伪造 server 把 101 响应 + 首帧**一次性**写出
    /// （Linux loopback 上单次 write_all 必落进同一 TCP 段），断言
    /// `do_client_handshake` 之后仍能读到首帧。修复前残余被丢弃 → 读超时/panic。
    #[test]
    fn first_ws_frame_coalesced_with_101_is_not_lost() {
        use std::io::{Read as _, Write as _};
        use std::thread;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind :0");
        let addr = listener.local_addr().expect("local_addr");
        let server = thread::spawn(move || {
            let (mut sock, _) = listener.accept().expect("accept");
            // 读完 HTTP 请求头（客户端一次性写出）。
            let mut req = Vec::new();
            let mut tmp = [0u8; 256];
            loop {
                let n = sock.read(&mut tmp).expect("server read req");
                if n == 0 {
                    return;
                }
                req.extend_from_slice(&tmp[..n]);
                if req.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            // **一次 write_all**：101 头 + 未掩码文本帧 "ack"。
            let mut out = Vec::new();
            out.extend_from_slice(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
            );
            out.extend_from_slice(&[0x81, 0x03]);
            out.extend_from_slice(b"ack");
            sock.write_all(&out).expect("server write 101+frame");
            sock.flush().expect("server flush");
            // 保持连接打开直到客户端读完。
            thread::sleep(std::time::Duration::from_millis(300));
        });

        let mut client = do_client_handshake(addr);
        client
            .set_read_timeout(Some(std::time::Duration::from_millis(1000)))
            .expect("set_read_timeout");
        let frame = read_one_text_frame(&mut client);
        assert_eq!(frame, "ack", "101 响应与首帧同批到达时，首帧不得被握手丢弃");
        let _ = client.shutdown(std::net::Shutdown::Both);
        let _ = server.join();
    }
}
