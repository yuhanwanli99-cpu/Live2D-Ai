//! Windows：cpal 输入流 + 进程内 ASR worker + 全局按住状态。
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use live2d_ai_runtime::asr::{self, deepseek};

use super::*;
use crate::desk_pet::bridge;

struct Gate {
    hold: HoldState,
    recorder: Option<Recorder>,
    /// 这次按住因为没钥匙而没开麦，松手要写明原因。
    no_key: bool,
}

impl Gate {
    const fn new() -> Self {
        Self {
            hold: HoldState::new(),
            recorder: None,
            no_key: false,
        }
    }
}

static GATE: Mutex<Gate> = Mutex::new(Gate::new());
static NOTICES: Mutex<Vec<Notice>> = Mutex::new(Vec::new());

fn with_gate<R>(f: impl FnOnce(&mut Gate) -> R) -> R {
    match GATE.lock() {
        Ok(mut g) => f(&mut g),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

fn push_notice(text: &'static str, ttl: Duration) {
    if let Ok(mut q) = NOTICES.lock() {
        q.push(Notice { text, ttl });
    }
}

fn drain_notices() -> Vec<Notice> {
    match NOTICES.lock() {
        Ok(mut q) => std::mem::take(&mut *q),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    }
}

/// 一次录音：cpal 输入流活在一个专门的线程里，`Stream` 一 drop 就停麦。
struct Recorder {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<i16>>>,
    thread: Option<JoinHandle<()>>,
}

impl Recorder {
    fn start() -> Result<Self, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "没有默认麦克风".to_string())?;
        let rate: cpal::SampleRate = SAMPLE_RATE; // cpal 里是 u32 的类型别名
        let chosen = device
            .supported_input_configs()
            .map_err(|e| e.to_string())?
            .find(|r| {
                r.min_sample_rate() <= rate
                    && rate <= r.max_sample_rate()
                    && matches!(
                        r.sample_format(),
                        cpal::SampleFormat::I16 | cpal::SampleFormat::F32
                    )
            })
            .map(|r| r.with_sample_rate(rate))
            .ok_or_else(|| "麦克风不支持 16 kHz 输入".to_string())?;
        let config: cpal::StreamConfig = chosen.config();
        let channels = usize::from(config.channels).max(1);
        let stop = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
        // `StreamConfig` 是 Copy：两条分支各拿一份；`device` 被移进这个
        // 闭包，输入流连同回调一起只活在本函数起的那个线程里。
        let format = chosen.sample_format();
        let capture = {
            let samples = Arc::clone(&samples);
            move || -> Result<cpal::Stream, String> {
                match format {
                    cpal::SampleFormat::I16 => device
                        .build_input_stream::<i16, _, _>(
                            config,
                            {
                                let samples = Arc::clone(&samples);
                                move |data: &[i16], _| {
                                    push_samples(&samples, data.iter().step_by(channels).copied())
                                }
                            },
                            |e| tracing::warn!(target: "desk_pet", error = %e, "麦克风回调错误"),
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    cpal::SampleFormat::F32 => device
                        .build_input_stream::<f32, _, _>(
                            config,
                            {
                                let samples = Arc::clone(&samples);
                                move |data: &[f32], _| {
                                    let converted = data
                                        .iter()
                                        .step_by(channels)
                                        .map(|&v| (v.clamp(-1.0, 1.0) * 32767.0) as i16);
                                    push_samples(&samples, converted)
                                }
                            },
                            |e| tracing::warn!(target: "desk_pet", error = %e, "麦克风回调错误"),
                            None,
                        )
                        .map_err(|e| e.to_string()),
                    other => Err(format!("不支持的采样格式 {other:?}")),
                }
            }
        };
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("desk-pet-mic".to_string())
            .spawn(move || {
                match capture() {
                    Ok(stream) => {
                        if let Err(e) = stream.play() {
                            tracing::warn!(target: "desk_pet", error = %e, "麦克风启动失败");
                            push_notice(MIC_NOTICE, Duration::from_secs(2));
                            return;
                        }
                        // 流活在这个线程里；松手/拖拽把 stop 置位就退出并 drop。
                        while !thread_stop.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target: "desk_pet", error = %e, "麦克风输入流创建失败");
                        push_notice(MIC_NOTICE, Duration::from_secs(2));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            stop,
            samples,
            thread: Some(thread),
        })
    }

    /// 停麦并取回样本（会 join 那个短线程，最多十几毫秒）。
    fn stop(&mut self) -> Vec<i16> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        self.samples
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default()
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn push_samples(samples: &Arc<Mutex<Vec<i16>>>, values: impl Iterator<Item = i16>) {
    if let Ok(mut g) = samples.lock() {
        for value in values {
            if g.len() >= MAX_SAMPLES {
                break;
            }
            g.push(value);
        }
    }
}

/// 松手（或 120 秒到点）：停麦 → WAV → 后台线程调现有 ASR → `say`。
fn finish(mut recorder: Recorder) {
    let samples = recorder.stop();
    let wav = encode_wav(&samples);
    if wav.len() > MAX_WAV_BYTES {
        tracing::warn!(target: "desk_pet", bytes = wav.len(), "录音超过 8 MiB，本机丢弃不打上游");
        push_notice(EMPTY_NOTICE, Duration::from_secs(2));
        return;
    }
    let key = match plan_asr(
        bridge::asr_key().as_deref(),
        bridge::is_playing(),
        HoldOutcome::Record,
    ) {
        AsrPlan::Transcribe(key) => key,
        AsrPlan::Notice(text, ttl) => {
            push_notice(text, ttl);
            return;
        }
        AsrPlan::Skip => return,
    };
    std::thread::Builder::new()
            .name("desk-pet-asr".to_string())
            .spawn(move || {
                let upload = asr::deepseek::AsrUpload {
                    bytes: &wav,
                    container: asr::AudioContainer::Wav,
                };
                match transcribe_blocking(&key, &upload) {
                    Ok(response) => {
                        let text = response.text.trim();
                        if text.is_empty() {
                            push_notice(EMPTY_NOTICE, Duration::from_secs(2));
                            return;
                        }
                        if !bridge::say(text) {
                            tracing::warn!(target: "desk_pet", "识别有文字但本轮没能送进对话（supervisor 忙或未装配）");
                        }
                    }
                    Err(_) => {
                        // 不回显上游响应体（可能含 Authorization），只留一条短日志。
                        tracing::warn!(target: "desk_pet", "宠物窗识别失败（上游错误体不回显）");
                        push_notice(EMPTY_NOTICE, Duration::from_secs(2));
                    }
                }
            })
            .ok();
}

fn transcribe_blocking(
    key: &str,
    upload: &asr::deepseek::AsrUpload<'_>,
) -> Result<asr::deepseek::AsrResponse, ()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ())?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| ())?;
    rt.block_on(deepseek::transcribe(&http, key, upload, deepseek::ASR_URL))
        .map_err(|_| ())
}

/// `WM_LBUTTONDOWN`：只记账，不开麦。
pub fn on_down() {
    with_gate(|g| {
        if let Some(mut old) = g.recorder.take() {
            let _ = old.stop();
        }
        g.no_key = false;
        g.hold.down(Instant::now());
    });
}

/// `WM_MOUSEMOVE`：相对按下点的位移；超过 8 像素就归拖拽。
pub fn on_move(dx: i32, dy: i32) {
    with_gate(|g| g.hold.moved(dx, dy));
}

/// `WM_LBUTTONUP` / `WM_CAPTURECHANGED`：结束这次按住。
pub fn on_up() {
    let (outcome, recorder, no_key) = with_gate(|g| {
        let outcome = g.hold.up(Instant::now());
        (outcome, g.recorder.take(), std::mem::take(&mut g.no_key))
    });
    match (recorder, outcome) {
        (Some(rec), HoldOutcome::Record) => finish(rec),
        (Some(mut rec), _) => {
            let _ = rec.stop(); // 拖拽：录到的也丢掉
        }
        (None, HoldOutcome::Record) if no_key => {
            push_notice(NO_KEY_NOTICE, Duration::from_secs(3));
        }
        _ => {}
    }
}

/// 每帧一次：到 300 ms 判定、120 秒自动松手、播放中停听、收提示。
pub fn poll() -> Vec<Notice> {
    let now = Instant::now();
    let should_start = with_gate(|g| g.hold.should_start(now));
    if should_start {
        if bridge::is_playing() {
            with_gate(|g| g.hold.cancel());
        } else if bridge::asr_key().is_none() {
            with_gate(|g| {
                g.no_key = true;
                g.hold.mark_no_record();
            });
        } else {
            match Recorder::start() {
                Ok(rec) => with_gate(|g| {
                    g.recorder = Some(rec);
                    g.hold.mark_recording(now);
                }),
                Err(e) => {
                    tracing::warn!(target: "desk_pet", error = %e, "开麦失败（不打上游）");
                    push_notice(MIC_NOTICE, Duration::from_secs(2));
                    with_gate(|g| g.hold.cancel());
                }
            }
        }
    }
    // 播自己的声音时不听：出声开始后把已开的麦撤掉。
    if bridge::is_playing() {
        let rec = with_gate(|g| {
            let rec = g.recorder.take();
            if g.hold.is_recording() {
                g.hold.cancel();
            }
            rec
        });
        if let Some(mut r) = rec {
            let _ = r.stop();
        }
    }
    if with_gate(|g| g.hold.auto_stop_due(now)) {
        let rec = with_gate(|g| g.recorder.take());
        if let Some(r) = rec {
            finish(r);
        }
        with_gate(|g| g.hold.cancel());
        tracing::info!(target: "desk_pet", "按住说话到 120 秒上限，自动松手");
    }
    if with_gate(|g| g.hold.is_drag()) {
        let rec = with_gate(|g| g.recorder.take());
        if let Some(mut r) = rec {
            let _ = r.stop();
        }
    }
    drain_notices()
}
