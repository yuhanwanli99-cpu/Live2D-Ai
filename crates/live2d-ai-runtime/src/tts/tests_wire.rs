//! 两档 TTS 的 **wire 形状**与 **整句失败语义**回归（本机假 HTTP）。
//!
//! 本文件守三件事，全部用 `127.0.0.1` 上的假上游，**从不连真的
//! `api.deepseek.com`**（门禁不许要外网）：
//!
//! 1. `openai` 档仍然是**逐块**转发（第一块到了就往下游送）——不因为
//!    `deepseek` 档改成整句缓冲而被拖成「收完才吐」；
//! 2. `deepseek` 档**中途断开 = 整句丢弃**：下游一个样本都拿不到
//!    （拿到的是错误，不是那半句 PCM）；
//! 3. `deepseek` 档正常结束时，整句 PCM 能进现有音频规格解析，且规格是
//!    **24 000 Hz / 单声道**（与 `[tts] sample_rate` 无关）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::config::{TtsConfig, TtsProvider};
use crate::secret::ApiSecret;

const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// 假上游记下的一条请求（只看本文件断言需要的两项）。
#[derive(Debug, Clone)]
struct Seen {
    head: String,
    body: Vec<u8>,
}

impl Seen {
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("请求体必须是 JSON")
    }
}

/// 读一整条请求（头 + Content-Length 决定的 body）。
async fn read_one(sock: &mut TcpStream) -> Seen {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        let n = timeout(READ_TIMEOUT, sock.read(&mut tmp))
            .await
            .expect("读头超时")
            .expect("读头失败");
        assert!(n > 0, "对端在头写完之前就关了");
        buf.extend_from_slice(&tmp[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let len = head
        .lines()
        .find_map(|line| {
            let (k, v) = line.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    let mut body = buf[header_end..].to_vec();
    while body.len() < len {
        let n = timeout(READ_TIMEOUT, sock.read(&mut tmp))
            .await
            .expect("读 body 超时")
            .expect("读 body 失败");
        assert!(n > 0, "对端在 body 写完之前就关了");
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(len);
    Seen { head, body }
}

/// 起一个假上游：每次连接跑一遍 `handler`，并把收到的请求记进 `seen`。
///
/// 返回 `(base_url, seen, 端点)`：`base_url` 是 `http://addr/v1` 形态（`openai`
/// 档拼 `/audio/speech` 用），`端点` 是完整 URL（`deepseek` 档直接指过去）。
async fn spawn_fake<F, Fut>(handler: F) -> (String, Arc<Mutex<Vec<Seen>>>, String)
where
    F: Fn(TcpStream, Seen) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(handler);
    let recorded = seen.clone();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let handler = handler.clone();
            let recorded = recorded.clone();
            tokio::spawn(async move {
                let req = read_one(&mut sock).await;
                recorded.lock().unwrap().push(req.clone());
                handler(sock, req).await;
            });
        }
    });
    (
        format!("http://{addr}/v1"),
        seen,
        format!("http://{addr}/v1/tools/tts"),
    )
}

async fn write_head(sock: &mut TcpStream, status: u16, reason: &str, len: usize) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/octet-stream\r\ncontent-length: {len}\r\nconnection: close\r\n\r\n"
    );
    sock.write_all(head.as_bytes()).await.expect("写头");
}

fn deepseek_tts(voice: &str) -> TtsConfig {
    let mut tts = TtsConfig::new("http://127.0.0.1:9/v1", voice);
    tts.provider = TtsProvider::DeepSeek;
    tts.api_key = Some(ApiSecret::new("sk-test-not-a-real-key"));
    tts
}

// ─────────────────────────────────────── ① openai 档：仍是逐块转发

/// `openai` 档**第一块到达就往下游送**（第一块之后上游还挂着 200 ms）。
///
/// 这是 V2 的回归项：`deepseek` 档改成整句缓冲之后，`openai` 档**不得**
/// 跟着变成「收完才吐」——那会把本机 TTS 的首字延迟拉长到整句时长。
#[tokio::test]
async fn openai_tier_still_streams_the_first_chunk_before_the_response_ends() {
    let (base, _seen, _endpoint) = spawn_fake(|mut sock, _req| async move {
        // content-length = 3 块 × 2 字节；只在第一块之后停 200 ms。
        write_head(&mut sock, 200, "OK", 6).await;
        sock.write_all(&[1, 2]).await.expect("第一块");
        tokio::time::sleep(Duration::from_millis(200)).await;
        sock.write_all(&[3, 4]).await.expect("第二块");
        sock.write_all(&[5, 6]).await.expect("第三块");
        let _ = sock.shutdown().await;
    })
    .await;

    let client = crate::OpenAiClient::new(
        crate::LlmConfig::new("http://127.0.0.1:9/v1", "m"),
        TtsConfig::new(&base, "alloy"),
    )
    .expect("client");

    let mut stream = timeout(READ_TIMEOUT, client.synthesize_speech_openai("你好。"))
        .await
        .expect("建立请求不应超时")
        .expect("openai 档应当成功");

    // 关键断言：**在整条响应结束之前**就拿到了第一块。
    let first = timeout(Duration::from_millis(150), stream.next())
        .await
        .expect("第一块必须在响应结束前到达（逐块转发）")
        .expect("流里应有第一块")
        .expect("第一块应成功");
    assert_eq!(first, vec![1, 2], "送下来的必须是第一块本身，不得整段缓冲");

    let mut rest = Vec::new();
    while let Some(chunk) = stream.next().await {
        rest.extend_from_slice(&chunk.expect("后续块"));
    }
    assert_eq!(rest, vec![3, 4, 5, 6], "其余块仍按到达序转发");
}

/// `openai` 档的请求路径与字段一字不变：`/v1/audio/speech` + `input`。
///
/// （分档之前就是这样；这条回归钉住「加了 provider 没有改造 openai 那一档」。）
#[tokio::test]
async fn openai_tier_keeps_audio_speech_path_and_input_field() {
    let (base, seen, _endpoint) = spawn_fake(|mut sock, _req| async move {
        write_head(&mut sock, 200, "OK", 2).await;
        sock.write_all(&[0, 0]).await.expect("pcm");
        let _ = sock.shutdown().await;
    })
    .await;

    let client = crate::OpenAiClient::new(
        crate::LlmConfig::new("http://127.0.0.1:9/v1", "m"),
        TtsConfig::new(&base, "alloy"),
    )
    .expect("client");
    let mut stream = client.synthesize_speech("你好。").await.expect("speech");
    while stream.next().await.is_some() {}

    let seen = seen.lock().unwrap();
    let req = seen.first().expect("必须发过一次请求");
    assert!(
        req.head.starts_with("POST /v1/audio/speech "),
        "openai 档路径必须是 /audio/speech：{}",
        req.head
    );
    let body = req.json();
    assert_eq!(body["input"], "你好。");
    assert_eq!(body["voice"], "alloy");
    assert!(
        body.get("voice_id").is_none(),
        "openai 档不得带 deepseek 的 voice_id"
    );
    assert!(
        body.get("filter_markdown").is_none(),
        "openai 档不得带 deepseek 的 filter_markdown"
    );
}

// ─────────────────────────────────────── ② deepseek 档：中途断开丢整句

/// `deepseek` 档：上游吐了几个 PCM 块就断开 → **下游拿到的是错误，不是那些块**。
///
/// 「拿到错误」有两层含义，这里都钉住：
/// - `synthesize` 本身返回 `Err`（连流都没建起来），或
/// - 建起来的流只给 `Err`、**一个 `Ok` 块都没有**。
///
/// 两种都满足「口型不会跟着半句抖」。
#[tokio::test]
async fn deepseek_mid_stream_disconnect_delivers_no_partial_pcm() {
    let (_base, seen, endpoint) = spawn_fake(|mut sock, _req| async move {
        // 声明 6 字节，只写 4 字节就断——典型的中途断开。
        write_head(&mut sock, 200, "OK", 6).await;
        sock.write_all(&[1, 2]).await.expect("第一块");
        sock.write_all(&[3, 4]).await.expect("第二块");
        let _ = sock.shutdown().await;
    })
    .await;

    let http = reqwest::Client::builder()
        .pool_max_idle_per_host(0)
        .build()
        .expect("http client");
    let tts = deepseek_tts("stella");

    match super::deepseek::synthesize(&http, &tts, "这一句不该出声。", &endpoint).await {
        Err(e) => {
            // 期望路径：整句丢弃，流都没建起来。
            assert!(
                e.to_string().to_lowercase().contains("http") || e.to_string().contains("传输"),
                "中途断开必须报传输层错误：{e}"
            );
        }
        Ok(mut stream) => {
            // 另一种可能：流建起来了，但只能给错误。
            let mut oks: Vec<Vec<u8>> = Vec::new();
            let mut errs = 0usize;
            while let Some(item) = stream.next().await {
                match item {
                    Ok(bytes) => oks.push(bytes),
                    Err(_) => errs += 1,
                }
            }
            assert!(oks.is_empty(), "下游不得收到任何半句 PCM，实际收到 {oks:?}");
            assert!(errs >= 1, "中途断开必须以错误收场");
        }
    }

    // 前提核对：这一轮**真的**打到了假上游（否则上面的断言是空转）。
    assert_eq!(seen.lock().unwrap().len(), 1, "应当恰好发过一次请求");
}

/// `deepseek` 档：非 2xx 整句失败（不是空音频、也不是半句）。
#[tokio::test]
async fn deepseek_non_2xx_fails_the_sentence_without_pcm() {
    let (_base, seen, endpoint) = spawn_fake(|mut sock, _req| async move {
        let body = br#"{"error":"upstream busy"}"#;
        let head = format!(
            "HTTP/1.1 503 Service Unavailable\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        sock.write_all(head.as_bytes()).await.expect("写头");
        sock.write_all(body).await.expect("写体");
        let _ = sock.shutdown().await;
    })
    .await;

    let http = reqwest::Client::builder().build().expect("http client");
    let err = super::deepseek::synthesize(&http, &deepseek_tts("stella"), "你好。", &endpoint)
        .await
        .err()
        .expect("非 2xx 必须失败");
    assert_eq!(err.upstream_status(), Some(503), "{err}");
    assert_eq!(seen.lock().unwrap().len(), 1);
}

// ─────────────────────────────────────── ③ 正常结束：规格 24000 / 1

/// `deepseek` 档正常结束：整句 PCM 进现有规格解析，规格是 **24 000 / 单声道**。
///
/// 配置里故意写 44 100 / 2 声道——`deepseek` 档必须**忽略**它（上游固定 24k 单声道），
/// 否则口型时钟会按错倍率走。
#[tokio::test]
async fn deepseek_whole_sentence_decodes_as_24000_hz_mono() {
    // 2400 个样本 = 0.1 s @ 24 kHz；s16le = 4800 字节。
    let pcm: Vec<u8> = (0..2_400i16).flat_map(|i| i.to_le_bytes()).collect();
    let expected = pcm.clone();
    let (_base, seen, endpoint) = spawn_fake(move |mut sock, _req| {
        let pcm = pcm.clone();
        async move {
            let half = pcm.len() / 2;
            write_head(&mut sock, 200, "OK", pcm.len()).await;
            // 中途故意切两片：整句语义下不影响结果。
            sock.write_all(&pcm[..half]).await.expect("前半");
            tokio::time::sleep(Duration::from_millis(20)).await;
            sock.write_all(&pcm[half..]).await.expect("后半");
            let _ = sock.shutdown().await;
        }
    })
    .await;

    let http = reqwest::Client::builder().build().expect("http client");
    let mut tts = deepseek_tts("tide");
    tts.spec = crate::audio::AudioSpec::new(44_100, 2).expect("valid");

    let mut stream = super::deepseek::synthesize(&http, &tts, "你好。", &endpoint)
        .await
        .expect("整句应当成功");

    let mut collected = Vec::new();
    while let Some(item) = stream.next().await {
        collected.extend_from_slice(&item.expect("块应成功"));
    }
    assert_eq!(collected, expected, "整句字节必须原样交出");

    // 规格由 deepseek 档固定，不读配置里的 44100/2。
    let spec = super::deepseek::resolved_spec();
    assert_eq!(spec.sample_rate(), 24_000, "deepseek 档恒为 24 kHz");
    assert_eq!(spec.channels(), 1, "deepseek 档恒为单声道");

    // 进现有解码器：4800 字节 / 2 = 2400 个样本，能被 PCM 解码器吃干净。
    let mut decoder = crate::audio::PcmS16LeDecoder::new(spec);
    let samples = decoder.decode(&collected);
    assert_eq!(samples.len(), 2_400, "24000 Hz 单声道下应解出 2400 个样本");
    decoder.finish().expect("整句必须样本对齐");

    // wire 形状：路径 + voice_id + format=pcm，且不带 openai 的 input。
    let seen = seen.lock().unwrap();
    let req = seen.first().expect("必须发过一次请求");
    assert!(
        req.head.starts_with("POST /v1/tools/tts "),
        "deepseek 档路径必须是 /v1/tools/tts：{}",
        req.head
    );
    let body = req.json();
    assert_eq!(body["voice_id"], "tide");
    assert_eq!(body["format"], "pcm");
    assert_eq!(body["filter_markdown"], false);
    assert_eq!(body["text"], "你好。");
    assert!(body.get("input").is_none(), "不得带 openai 的 input");
}
