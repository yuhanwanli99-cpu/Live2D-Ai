//! Broadcaster：活跃 WS 连接的注册表 + 广播（P1WS-2 拆分承载）。
//!
//! - Vec<ConnectionHandle> 携带稳定 conn_id，unsubscribe(id) 按 id 移除
//!   （O(n) 但 n ≤ 32 可接受）；
//! - broadcast(typed_event) 广播**未**含 seq/ts 的 typed event，seq/ts 由各连接
//!   actor（super::connection）包装（每连接独立 seq）；
//! - 死 sender 由下次 broadcast try_send 失败回收（仍保留向后兼容）；
//! - muted：服务端**输出静音**（默认出声，2026-09-10 用户裁决；跑测试用
//!   LIVE2D_AI_MUTE_AUDIO=1）——语义见 super::audio::build_audio_frames。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::audio::build_audio_frames;
use super::connection::WsWriter;

/// WS 连接并发上限（防线程泄漏 + DoS）。32 = 前端 ≤ 3 标签 + chat overlay；
/// 再高通常是滥用。超额时 `handle_ws_request` 回 503 不进入升级流程。
pub const WS_CONN_LIMIT: usize = 32;

/// 单连接在 broadcaster 侧的句柄。
///
/// `unsubscriber` 在 actor 退出时被调用（关闭 ws 后），让 broadcaster
/// 立即回收槽位，**不**依赖下次 broadcast try_send 失败。
#[derive(Clone)]
struct ConnectionHandle {
    conn_id: u64,
    /// 文本帧 sender（actor 收 + 包装 seq/ts 后写入 ws；M0-7 单线程直连）。
    tx: WsWriter,
}

/// 广播器：所有活跃 WS 连接的 writer 集合 + 订阅计数 + 稳定 conn_id。
///
/// **P1WS-2 改进**：
/// - `Vec<ConnectionHandle>` 携带稳定 `conn_id`，`unsubscribe(id)` 按 id
///   移除（O(n) 但 n ≤ 32 可接受）；
/// - `broadcast(typed_event)` 广播**未**含 seq/ts 的 typed event，seq/ts
///   由各连接的 actor 包装（每连接独立 seq）。
/// - 死 sender 由下次 broadcast try_send 失败回收（仍保留向后兼容）。
#[derive(Clone)]
pub struct Broadcaster {
    inner: Arc<Mutex<Vec<ConnectionHandle>>>,
    subscribed_count: Arc<AtomicUsize>,
    /// **输出静音**（默认 `false` = 出声，2026-09-10 用户裁决）。
    ///
    /// 为 true 时下发给所有客户端的 PCM 全部置零、`volume` 仍为真实值 ——
    /// 即「谁都发不出声音，但口型照常」。语义与理由见
    /// [`build_audio_frames`] 的 `mute` 参数。
    ///
    /// **默认出声**，但保留这个服务端开关：跑测试 / 无人值守时用
    /// `LIVE2D_AI_MUTE_AUDIO=1` 打开它，任何客户端都发不出声，不必依赖前端自觉。
    /// 放在服务端而不是客户端：客户端侧的静音绕不过 Service Worker 缓存的
    /// 旧前端、遗留 JS 前端与第三方客户端。
    muted: Arc<AtomicBool>,
}

impl Default for Broadcaster {
    /// 默认**出声**（见 [`Self::muted`]）。
    ///
    /// 手写而不是 `derive`：`AtomicBool` 的 `Default` 恰好也是 `false`，但两者
    /// 语义不同（一个是「类型默认值」，一个是「产品默认出声」），不靠巧合。
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
            subscribed_count: Arc::new(AtomicUsize::new(0)),
            muted: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Broadcaster {
    /// 新建空广播器（订阅上限 = [`WS_CONN_LIMIT`]）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前是否静音输出。
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Acquire)
    }

    /// 设置静音输出（服务端权威，立即对后续分片生效）。
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
    }

    /// 尝试注册一个新连接；超额时返回 `Err(())`。
    ///
    /// 成功时把 sender 推入内部 vec，计数 +1，并返回稳定 conn_id。
    /// 失败时 vec/计数都不变——caller 决定是否回 503。
    pub fn try_subscribe(&self, ws: WsWriter) -> Result<u64, ()> {
        let mut guard = self.inner.lock().expect("broadcaster mutex");
        let prev = self.subscribed_count.load(Ordering::Acquire);
        if prev >= WS_CONN_LIMIT {
            return Err(());
        }
        let conn_id = super::connection::next_connection_id();
        guard.push(ConnectionHandle { conn_id, tx: ws });
        self.subscribed_count.store(prev + 1, Ordering::Release);
        Ok(conn_id)
    }

    /// 按稳定 conn_id 注销订阅（**新**：P1WS-2）。
    ///
    /// 显式回收槽位，**不**等下次 broadcast try_send 失败。返回是否真的
    /// 移除了某条（false = id 不存在，可能是双重 unsubscribe）。
    pub fn unsubscribe(&self, conn_id: u64) -> bool {
        let mut guard = self.inner.lock().expect("broadcaster mutex");
        let before = guard.len();
        guard.retain(|h| h.conn_id != conn_id);
        let removed = before != guard.len();
        if removed {
            let prev = self.subscribed_count.load(Ordering::Acquire);
            if prev > 0 {
                self.subscribed_count.store(prev - 1, Ordering::Release);
            }
        }
        removed
    }

    /// 当前订阅者数量（仅供测试与观测；生产不开销）。
    pub fn subscriber_count(&self) -> usize {
        self.subscribed_count.load(Ordering::Acquire)
    }

    /// 广播一条**已序列化**的 JSON 帧（**不**含 seq/ts）。seq/ts 由
    /// 各连接 actor 在 [`connection::wrap_preserialized_with_seq`] 处
    /// 解析 + 包装（每连接独立 seq）。
    ///
    /// **不**阻塞：每个订阅者用 `try_send`（失败即移除该订阅者，计数 -1）。
    /// 死 sender 由下一次 broadcast try_send 失败回收（延迟回收）。
    pub fn broadcast(&self, frame: &str) {
        let mut guard = self.inner.lock().expect("broadcaster mutex");
        let mut idx = 0;
        while idx < guard.len() {
            if guard[idx].tx.try_send(frame.to_string()).is_err() {
                let removed = guard.swap_remove(idx);
                let prev = self.subscribed_count.load(Ordering::Acquire);
                if prev > 0 {
                    self.subscribed_count.store(prev - 1, Ordering::Release);
                }
                tracing::debug!(
                    conn_id = removed.conn_id,
                    "broadcaster: 慢订阅者 try_send 失败 → 移除槽位"
                );
            } else {
                idx += 1;
            }
        }
    }

    /// 测试用：强制清空广播器（模拟所有订阅者掉线）。
    #[cfg(test)]
    pub fn clear(&self) {
        self.inner.lock().expect("broadcaster mutex").clear();
        self.subscribed_count.store(0, Ordering::Release);
    }

    /// 构造音频帧并逐帧广播（封装 [`build_audio_frames`] + [`broadcast`]）。
    ///
    /// **接线便利**：调用方（如 cli_entry emit 闭包）在拿到
    /// broadcaster + 一段 s16le PCM 时可直接调。注意：当前 v1 路径下 PCM
    /// 不经 AppEvent（详见 supervisor.rs 文档约束），因此**需要在 PCM 流经处**
    /// 显式调用本方法——见报告“接线点结论”。
    ///
    /// `sentence_seq` / `first_chunk` / `final_chunk` 由**引擎**给出
    /// （`EngineEvent::AudioChunk`）。
    ///
    /// **2026-09-11 修**：这里过去用 `audio_epoch_seen` 按 epoch 记账去推 `start`，
    /// 把「一轮语音的首片」当成了句子边界。一句音频本来就会被切成十几块，按
    /// epoch 记账永远推不出句界——实测 WS 上出现「0 个 start、13 个 end」，
    /// 前端按 `start`→`end` 攒句会攒错、播出来断断续续。现在句界直接来自引擎，
    /// 本方法**不再做任何推断**，那份 epoch 记账状态也随之删除。
    #[allow(dead_code)]
    pub fn broadcast_audio_frames(
        &self,
        epoch: u64,
        pcm: &[u8],
        sample_rate: u32,
        sentence_seq: u64,
        first_chunk: bool,
        final_chunk: bool,
    ) {
        let muted = self.muted.load(Ordering::Acquire);
        for frame in build_audio_frames(
            epoch,
            pcm,
            sample_rate,
            sentence_seq,
            first_chunk,
            final_chunk,
            muted,
        ) {
            self.broadcast(&frame);
        }
    }
}

impl Drop for Broadcaster {
    /// broadcaster drop 时所有 ConnectionHandle 的 tx 被 drop → 各 ws-actor
    /// 线程 rx 断开 → actor 优雅退出（close + unsubscriber）→ 槽位回收。
    ///
    /// **不**在 Drop 里**主动**调 unsubscribe（unsubscribe 闭包本身依赖
    /// broadcaster 状态——已经 drop，再调会死锁）。
    fn drop(&mut self) {
        // 故意 no-op：依赖 mpsc 断链传播。日志便于排障。
        let n = self.subscribed_count.load(Ordering::Acquire);
        if n > 0 {
            tracing::debug!(
                remaining = n,
                "Broadcaster drop：等待剩余 ws-actor 线程自行退出（rx 断链传播）"
            );
        }
    }
}
