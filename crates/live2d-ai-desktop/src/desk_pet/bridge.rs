//! 宠物窗 ↔ 桌面进程的**唯一接线点**（旁路，2026-11-17 长期陪伴）。
//!
//! # 纪律
//!
//! - **窗没开时不分配通道**：`std::sync::mpsc` 只在窗线程的
//!   [`open_events`] 里创建；此前 [`publish`] 是纯粹的 no-op。于是
//!   `/app/`（Flutter Web）与并入前逐字一致，不会为了一个不存在的窗
//!   去抢事件、抢麦克风、抢播放器。
//! - **不新开 HTTP 回路**：`say` 是启动期注入的同一个
//!   `SupervisorHandle::say` 闭包；识别走 `live2d_ai_runtime::asr::deepseek`
//!   的现成函数，不打自己的 `/api/v1/voice/asr`。
//! - **不新起播放器**：出声仍是桌面进程里那一路 cpal；这里只转发
//!   「这句开始 / 这句结束」两个事实给气泡。
//! - 只订阅四类 `AppEvent`：已交给合成的那一句（`TextDelta`，来源
//!   `EngineEvent::SentenceVoiced`）、`VoiceStarted`、`VoiceEnded`、
//!   `NewEpoch`（`/stop` 打断时也推进 epoch，用来立刻撤掉气泡）。

// 非 Windows：宿主出口与窗线程通道都不装配，纯映射函数仍供单测使用。
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

use crate::app_event::{AppEvent, ConversationUiEvent};
use crate::audio::MouthSnapshot;

/// 进程内「说一句话」的出口（启动期注入，等于 `SupervisorHandle::say`）。
pub type SayFn = Arc<dyn Fn(String) -> bool + Send + Sync>;

/// 从 supervisor 送进窗线程的事实。只有这三种，气泡不需要别的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PetEvent {
    /// 这一句**已经交给语音合成**的上台词（整块替换，不是增量）。
    /// 右键菜单的「AI 回复」开关管它。
    Sentence(String),
    /// 用户刚说的那句（网页里发的 / ASR 识别出来的）。
    /// 右键菜单的「输入消息」开关管它；两块气泡、会话框都读这一路。
    UserLine(String),
    /// 这句 PCM 在 cpal 里开始播。
    VoiceStarted,
    /// 这句播完 / 被 `/stop` 打断 / 声卡排空。
    VoiceEnded,
}

struct Host {
    say: Option<SayFn>,
    asr_key: Option<String>,
}

static HOST: OnceLock<Host> = OnceLock::new();
/// `None` = 宠物窗没开（此时连 `Sender` 都不存在）。
static EVENT_TX: Mutex<Option<mpsc::Sender<PetEvent>>> = Mutex::new(None);
/// 正在播自己的声音：按住说话在这段时间不听（与 Flutter
/// `suspendForPlayback` 同一条理由，避免把喇叭录回去）。
static PLAYING: AtomicBool = AtomicBool::new(false);
/// 当前 cpal facade 的实际输出快照；浏览器 / 无声卡时必须为 None。
static MOUTH_SNAPSHOT: Mutex<Option<MouthSnapshot>> = Mutex::new(None);

/// 启动期注入（见 `web_api::cli_entry::run_web_mode`）。重复调用只认第一次。
pub fn install_host(say: Option<SayFn>, asr_key: Option<String>) {
    let _ = HOST.set(Host { say, asr_key });
}

/// 非空文本 → 直接 `supervisor.say`（热槽 / 导演 / 逐句合成照旧在这一轮里）。
/// 没装 supervisor（纯控制平面）时返回 false，调用方只记日志。
pub fn say(text: &str) -> bool {
    HOST.get()
        .and_then(|h| h.say.as_ref())
        .is_some_and(|f| f(text.to_string()))
}

/// `[asr].api_key_env` 解析出来的钥匙；**空串视作没配**。
pub fn asr_key() -> Option<String> {
    HOST.get()
        .and_then(|h| h.asr_key.clone())
        .filter(|k| !k.is_empty())
}

pub fn is_playing() -> bool {
    PLAYING.load(Ordering::SeqCst)
}

/// cpal 打开成功后安装实际输出快照；切换到浏览器 / 无声卡时显式装 None，
/// 防止下一次开窗读到上一路播放器留下的陈旧 RMS。
pub fn install_mouth_snapshot(snapshot: Option<MouthSnapshot>) {
    let mut guard = MOUTH_SNAPSHOT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = snapshot;
}

fn mouth_open() -> f32 {
    let guard = MOUTH_SNAPSHOT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    mouth_open_from_snapshot(guard.as_ref(), is_playing())
}

fn mouth_open_from_snapshot(snapshot: Option<&MouthSnapshot>, playing: bool) -> f32 {
    if !playing {
        return 0.0;
    }
    mouth_open_from_rms(snapshot.map_or(0.0, MouthSnapshot::level))
}

/// 与 `l2d-wasm-demo/src/mouth.rs` 同一标定：-36..-6 dBFS → 0..1。
fn mouth_open_from_rms(level: f32) -> f32 {
    if !level.is_finite() || level <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * level.min(1.0).log10();
    ((db + 36.0) / 30.0).clamp(0.0, 1.0)
}

/// 原生渲染唯一口型入口：先写实际输出 RMS 的映射值，再推进 renderer。
#[cfg(windows)]
pub fn update_renderer(
    renderer: &mut l2d::renderer::OffscreenRenderer,
) -> Result<(), l2d::renderer::RenderError> {
    renderer.core().set_mouth_open(mouth_open());
    renderer.update(l2d::renderer::FIXED_DT_60HZ)
}

/// `AppEvent` → `PetEvent` 的纯映射（单测钉住；不碰全局状态）。
#[must_use]
pub fn map_app_event(event: &AppEvent) -> Option<PetEvent> {
    let AppEvent::Conversation(inner) = event else {
        return None;
    };
    match inner {
        // SentenceVoiced 的 text 就是「已经交给合成的那一句」的上屏文本。
        ConversationUiEvent::TextDelta { text, .. } => Some(PetEvent::Sentence(text.clone())),
        // 2026-11-17 纠正版：用户句要进「输入消息」气泡与简易会话框。
        ConversationUiEvent::UserLine { text, .. } => Some(PetEvent::UserLine(text.clone())),
        ConversationUiEvent::VoiceStarted { .. } => Some(PetEvent::VoiceStarted),
        ConversationUiEvent::VoiceEnded { .. } | ConversationUiEvent::NewEpoch { .. } => {
            Some(PetEvent::VoiceEnded)
        }
        // 思考 / cue / 失败兜底都不进这扇窗。
        _ => None,
    }
}

/// supervisor 的 `emit` 闭包每次发射后调一次；窗没开时是 no-op。
pub fn publish_from_app_event(event: &AppEvent) {
    let Some(mapped) = map_app_event(event) else {
        return;
    };
    match &mapped {
        PetEvent::VoiceStarted => PLAYING.store(true, Ordering::SeqCst),
        PetEvent::VoiceEnded => PLAYING.store(false, Ordering::SeqCst),
        PetEvent::Sentence(_) | PetEvent::UserLine(_) => {}
    }
    publish(mapped);
}

/// 只往「已经开着的宠物窗」投；满 / 无窗 / 毒锁一律静默丢弃（旁路纪律）。
pub fn publish(event: PetEvent) {
    if let Ok(guard) = EVENT_TX.lock()
        && let Some(tx) = guard.as_ref()
    {
        let _ = tx.send(event);
    }
}

/// 窗线程建窗成功后调：此刻才分配通道。返回接收端，由窗线程每帧抽干。
pub fn open_events() -> mpsc::Receiver<PetEvent> {
    let (tx, rx) = mpsc::channel();
    match EVENT_TX.lock() {
        Ok(mut guard) => *guard = Some(tx),
        Err(poisoned) => *poisoned.into_inner() = Some(tx),
    }
    rx
}

/// 窗关掉时撤掉通道，之后的 `publish` 重新变回 no-op。
pub fn close_events() {
    match EVENT_TX.lock() {
        Ok(mut guard) => *guard = None,
        Err(poisoned) => *poisoned.into_inner() = None,
    }
    PLAYING.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(text: &str) -> AppEvent {
        AppEvent::Conversation(ConversationUiEvent::TextDelta {
            epoch: 7,
            ts_ms: 1,
            sentence_seq: 2,
            text: text.to_string(),
        })
    }

    #[test]
    fn only_the_voiced_sentence_and_voice_edges_reach_the_pet_window() {
        assert_eq!(
            map_app_event(&delta("今天想聊点什么")),
            Some(PetEvent::Sentence("今天想聊点什么".to_string()))
        );
        assert_eq!(
            map_app_event(&AppEvent::Conversation(ConversationUiEvent::VoiceStarted {
                epoch: 7
            })),
            Some(PetEvent::VoiceStarted)
        );
        assert_eq!(
            map_app_event(&AppEvent::Conversation(ConversationUiEvent::VoiceEnded {
                epoch: 7
            })),
            Some(PetEvent::VoiceEnded)
        );
        // /stop 推进 epoch；用它兜住「播到一半被打断」也要撤气泡。
        assert_eq!(
            map_app_event(&AppEvent::Conversation(ConversationUiEvent::NewEpoch {
                epoch: 8
            })),
            Some(PetEvent::VoiceEnded)
        );
    }

    #[test]
    fn user_line_feeds_the_input_bubble_but_reasoning_stays_out() {
        let reasoning = AppEvent::Conversation(ConversationUiEvent::ReasoningDelta {
            epoch: 7,
            ts_ms: 1,
            text: "内心独白".to_string(),
        });
        let user = AppEvent::Conversation(ConversationUiEvent::UserLine {
            epoch: 7,
            text: "用户说的".to_string(),
            session: None,
        });
        assert_eq!(
            map_app_event(&user),
            Some(PetEvent::UserLine("用户说的".to_string()))
        );
        assert_eq!(map_app_event(&reasoning), None);
        assert_eq!(map_app_event(&AppEvent::ShutdownReady), None);
    }

    /// 窗没开时 `publish` 不得 panic（这条通道本身也不该被分配）。
    #[test]
    fn publish_without_an_open_window_is_a_noop() {
        publish(PetEvent::Sentence("没人收".to_string()));
        publish(PetEvent::UserLine("没人听".to_string()));
        publish(PetEvent::VoiceStarted);
        publish(PetEvent::VoiceEnded);
        assert!(!is_playing(), "窗没开时不应留下播放态");
    }

    #[test]
    fn no_snapshot_or_not_playing_means_closed_mouth() {
        assert_eq!(mouth_open_from_snapshot(None, true), 0.0);
        assert_eq!(mouth_open_from_snapshot(None, false), 0.0);
    }

    #[test]
    fn rms_mapping_covers_calibration_boundaries_and_bad_input() {
        let floor = 10.0_f32.powf(-36.0 / 20.0);
        let ceiling = 10.0_f32.powf(-6.0 / 20.0);
        let midpoint = 10.0_f32.powf(-21.0 / 20.0);
        assert_eq!(mouth_open_from_rms(floor / 2.0), 0.0);
        assert!(mouth_open_from_rms(floor).abs() < 1e-6);
        assert!((mouth_open_from_rms(midpoint) - 0.5).abs() < 1e-5);
        assert_eq!(mouth_open_from_rms(ceiling), 1.0);
        assert_eq!(mouth_open_from_rms(1.0), 1.0);
        for silent in [0.0, -0.5, f32::NAN, f32::INFINITY] {
            assert_eq!(mouth_open_from_rms(silent), 0.0);
        }
    }

    #[test]
    fn detached_snapshot_uses_actual_output_rms_and_silence() {
        use crate::audio::ring::detached_pair_with;
        use live2d_ai_runtime::AudioSpec;

        let spec = AudioSpec::new(48_000, 1).expect("valid spec");
        let (mut handle, mut render) =
            detached_pair_with(4096, spec, spec, 10.0, 0.0, 0.0).expect("detached pair");
        let snapshot = handle.mouth_snapshot();
        assert_eq!(mouth_open_from_snapshot(Some(&snapshot), true), 0.0);

        let tone = [0.5_f32, -0.5].repeat(480);
        handle.enqueue_pcm_f32(&tone);
        render.render_f32(&mut vec![0.0; tone.len()]);
        assert!(mouth_open_from_snapshot(Some(&snapshot), true) > 0.99);
        assert_eq!(mouth_open_from_snapshot(Some(&snapshot), false), 0.0);

        let silence = vec![0.0_f32; 960];
        handle.enqueue_pcm_f32(&silence);
        render.render_f32(&mut vec![0.0; silence.len()]);
        assert_eq!(mouth_open_from_snapshot(Some(&snapshot), true), 0.0);
    }
}
