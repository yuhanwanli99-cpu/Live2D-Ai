//! supervisor 闭环测试共用基建：mock HTTP server、collector 别名、wait/fact
//! 助手以及 detached audio 工厂。所有符号 `pub(super)`，仅在 `supervisor::*`
//! 测试模块之间共享，外部不可见。

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app_event::{AppEvent, RootFact};
use crate::audio::{PreparedPcm, TryEnqueue};

// ---- 极简 HTTP/TCP mock 基建（std 线程；每连接独立处理）。 ----

pub(super) struct MockReq {
    pub body: Vec<u8>,
}

pub(super) fn read_mock_request(stream: &mut std::net::TcpStream) -> Option<MockReq> {
    let mut buf = [0u8; 4096];
    let mut got = Vec::new();
    let header_end;
    loop {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        got.extend_from_slice(&buf[..n]);
        if let Some(pos) = find_subslice(&got, b"\r\n\r\n") {
            header_end = pos + 4;
            break;
        }
        if got.len() > 64 * 1024 {
            return None;
        }
    }
    let head = String::from_utf8_lossy(&got[..header_end]).to_string();
    let mut content_length = 0usize;
    for line in head.lines() {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }
    while got.len() < header_end + content_length {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        got.extend_from_slice(&buf[..n]);
    }
    Some(MockReq {
        body: got[header_end..].to_vec(),
    })
}

pub(super) fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

pub(super) fn mock_respond(stream: &mut std::net::TcpStream, ctype: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// LLM mock：固定 SSE 流 + 请求体收集。
pub(super) fn spawn_llm_mock(bodies: Arc<Mutex<Vec<serde_json::Value>>>, sse: String) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind llm");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut sock) = conn else { continue };
            let bodies = bodies.clone();
            let sse = sse.clone();
            std::thread::spawn(move || {
                if let Some(req) = read_mock_request(&mut sock) {
                    eprintln!("[llm-mock] 收到请求 body={}B", req.body.len());
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&req.body) {
                        bodies.lock().expect("poison").push(v);
                    }
                    mock_respond(&mut sock, "text/event-stream", sse.as_bytes());
                } else {
                    eprintln!("[llm-mock] read_request 返回 None！");
                }
            });
        }
    });
    format!("http://{addr}/v1")
}

/// 慢速 LLM mock：响应前先睡 delay，制造稳定的「进行中」窗口供 stop 命中。
pub(super) fn spawn_llm_mock_slow(
    bodies: Arc<Mutex<Vec<serde_json::Value>>>,
    sse: String,
    delay: Duration,
) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind llm");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut sock) = conn else { continue };
            let bodies = bodies.clone();
            let sse = sse.clone();
            std::thread::spawn(move || {
                if let Some(req) = read_mock_request(&mut sock) {
                    eprintln!("[llm-mock] 收到请求 body={}B", req.body.len());
                    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&req.body) {
                        bodies.lock().expect("poison").push(v);
                    }
                    std::thread::sleep(delay);
                    mock_respond(&mut sock, "text/event-stream", sse.as_bytes());
                }
            });
        }
    });
    format!("http://{addr}/v1")
}

/// TTS mock：固定小段偶数字节 PCM（s16 LE 8 个源样本 → 24k mono）。
pub(super) fn spawn_tts_mock() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind tts");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut sock) = conn else { continue };
            std::thread::spawn(move || {
                if read_mock_request(&mut sock).is_some() {
                    mock_respond(
                        &mut sock,
                        "audio/pcm",
                        &[
                            0, 0, 255, 255, 0, 0, 128, 128, 10, 0, 240, 255, 5, 0, 250, 255,
                        ],
                    );
                }
            });
        }
    });
    format!("http://{addr}/v1")
}

/// 慢速 TTS mock：响应前先睡 delay，制造稳定的「TTS 在飞」窗口供 stop 命中。
/// 收到请求后才会连接句柄（不预热），确保 stop 期间未达请求不会自动回包。
pub(super) fn spawn_tts_mock_slow(delay: Duration) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind tts");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut sock) = conn else { continue };
            std::thread::spawn(move || {
                if read_mock_request(&mut sock).is_some() {
                    std::thread::sleep(delay);
                    mock_respond(
                        &mut sock,
                        "audio/pcm",
                        &[
                            0, 0, 255, 255, 0, 0, 128, 128, 10, 0, 240, 255, 5, 0, 250, 255,
                        ],
                    );
                }
            });
        }
    });
    format!("http://{addr}/v1")
}

pub(super) type Collector = Arc<Mutex<Vec<AppEvent>>>;

pub(super) fn wait_for(timeout: Duration, cond: impl Fn() -> bool) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if cond() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return cond();
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// 审计事实的计数器读取。
pub(super) fn count_fact(c: &Collector, pred: impl Fn(&RootFact) -> bool) -> usize {
    c.lock()
        .expect("poison")
        .iter()
        .filter(|e| matches!(e, AppEvent::RootAudit(f) if pred(f)))
        .count()
}

pub(super) fn has_dropped(c: &Collector) -> bool {
    c.lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::RootAudit(RootFact::Dropped)))
}

/// 收集所有 RootAudit 事实（按下标）。
pub(super) fn facts(c: &Collector) -> Vec<RootFact> {
    c.lock()
        .expect("poison")
        .iter()
        .filter_map(|e| match e {
            AppEvent::RootAudit(f) => Some(*f),
            _ => None,
        })
        .collect()
}

/// 找某事实在 collector 中的**首次出现下标**（无则 None）。
pub(super) fn first_index(c: &Collector, pred: impl Fn(&RootFact) -> bool) -> Option<usize> {
    c.lock()
        .expect("poison")
        .iter()
        .position(|e| matches!(e, AppEvent::RootAudit(f) if pred(f)))
}

/// detached PCM 生产者 + 模拟声卡回调消费核心（ring 极小以强制 WouldBlock）。
pub(super) fn detached_audio(
    cap: usize,
) -> (Box<dyn crate::audio::PcmProducer>, crate::audio::RenderCore) {
    let src = live2d_ai_runtime::AudioSpec::new(24_000, 1).expect("src");
    let dev = live2d_ai_runtime::AudioSpec::new(48_000, 2).expect("dev");
    let (h, r) = crate::audio::detached_pair_for_tests(cap, src, dev);
    (Box::new(h), r)
}

/// 故障注入 producer：当 `fault` 置位后 `try_enqueue_prepared` 立刻回
/// `WouldBlock{accepted:0}`、`is_drained/healthy` 一并返回 false——模拟
/// 声卡在播放过程中挂掉。让 `fatal_after_started` 与
/// `internal_audio_fault_yields_turn_completed_failed` 共用同一份实现。
pub(super) struct FaultProducer {
    pub(super) inner: crate::audio::PlaybackHandle,
    pub(super) fault: Arc<std::sync::atomic::AtomicBool>,
}

impl crate::audio::PcmProducer for FaultProducer {
    fn prepare_pcm_f32(
        &self,
        samples: &[f32],
        source: live2d_ai_runtime::AudioSpec,
    ) -> PreparedPcm {
        self.inner.prepare_pcm_f32(samples, source)
    }
    fn try_enqueue_prepared(&mut self, prepared: &mut PreparedPcm) -> TryEnqueue {
        if self.fault.load(std::sync::atomic::Ordering::SeqCst) {
            return TryEnqueue::WouldBlock { accepted: 0 };
        }
        self.inner.try_enqueue_prepared(prepared)
    }
    fn stop_and_clear(&self) -> u64 {
        self.inner.stop_and_clear()
    }
    fn is_drained(&self) -> bool {
        !self.fault.load(std::sync::atomic::Ordering::SeqCst) && self.inner.is_drained()
    }
    fn healthy(&self) -> bool {
        !self.fault.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// 构造一个 24k mono → 48k stereo 的 detached ring（cap=512）+ fault flag。
/// 返回 `(producer, fault_flag, _render)`；render 仅持有所有权，防止 drop 提前关环。
pub(super) fn make_fault_producer() -> (
    Box<dyn crate::audio::PcmProducer>,
    Arc<std::sync::atomic::AtomicBool>,
    crate::audio::RenderCore,
) {
    let fault = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (inner, render) = crate::audio::detached_pair_for_tests(
        512,
        live2d_ai_runtime::AudioSpec::new(24_000, 1).expect("s"),
        live2d_ai_runtime::AudioSpec::new(48_000, 2).expect("d"),
    );
    let producer = Box::new(FaultProducer {
        inner,
        fault: fault.clone(),
    }) as Box<dyn crate::audio::PcmProducer>;
    (producer, fault, render)
}

/// 永久满环 producer：`try_enqueue_prepared` 恒返回 `WouldBlock{accepted:0}`，
/// `healthy()` 恒 true，`is_drained()` 恒 false。模拟「声卡永远不消费、也不
/// 报 fault」的零进展场景——supervisor 必须依赖 `MAX_STALL` 哨兵而非声卡
/// 自报 fault 才能致命收尾。detach 出的 render 由测试自己持有。
pub(super) struct StallProducer {
    pub(super) inner: crate::audio::PlaybackHandle,
}

impl crate::audio::PcmProducer for StallProducer {
    fn prepare_pcm_f32(
        &self,
        samples: &[f32],
        source: live2d_ai_runtime::AudioSpec,
    ) -> PreparedPcm {
        self.inner.prepare_pcm_f32(samples, source)
    }
    fn try_enqueue_prepared(&mut self, _prepared: &mut PreparedPcm) -> TryEnqueue {
        // 永远满 + 不接：强制泵进入 WouldBlock{accepted:0} 零进展等待。
        TryEnqueue::WouldBlock { accepted: 0 }
    }
    fn stop_and_clear(&self) -> u64 {
        self.inner.stop_and_clear()
    }
    fn is_drained(&self) -> bool {
        false
    }
    fn healthy(&self) -> bool {
        true
    }
}

/// 构造一个永久满环 producer（与一个可独立 drop 的 render 句柄）。
pub(super) fn make_stall_producer() -> (Box<dyn crate::audio::PcmProducer>, crate::audio::RenderCore)
{
    let (inner, render) = crate::audio::detached_pair_for_tests(
        512,
        live2d_ai_runtime::AudioSpec::new(24_000, 1).expect("s"),
        live2d_ai_runtime::AudioSpec::new(48_000, 2).expect("d"),
    );
    let producer = Box::new(StallProducer { inner }) as Box<dyn crate::audio::PcmProducer>;
    (producer, render)
}
