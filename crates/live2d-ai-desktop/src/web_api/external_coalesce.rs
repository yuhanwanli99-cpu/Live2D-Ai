//! 外部事件「忙时合并再注入」（3.0 队首第一片，2026-10-17）。
//!
//! # 背景
//!
//! `POST /api/v1/external/chat` 经 `supervisor.say` 注入主链路；`say` 走容量 1
//! 的通道。主链在飞一轮时第一条能排队，其后到达的会拿到 `busy`——旧实现直接
//! **丢弃**。直播弹幕 / 礼物是突发流量，丢弃 = 丢内容。
//!
//! # 现行语义（本模块）
//!
//! - **上一条还没处理完**（[crate::supervisor::SupervisorHandle::turn_in_flight]）：
//!   端口**收下、先不注入**，开合并窗口；窗口开着时后续到达的文本按到达顺序
//!   追加；等这一轮收口后，常驻线程把整窗文本用换行拼成**一条**注入（一次
//!   `say`），然后清窗；
//! - **主链空闲且窗口为空**：逐条直送（`say` 返回 `true`），与旧版一致；
//! - **直送失败**（`busy`，例如别的来源刚占满容量 1 通道）：同样开合并窗口；
//! - **窗口满兜底**（[MAX_WINDOW_CHARS]）：回退旧语义——`200 ok:false`
//!   code `busy`，本条丢弃，发送方退避重试。上限只防无界内存 / 超长输入，
//!   正常弹幕远到不了。
//!
//! # 生命周期
//!
//! 一个服务一个常驻 `external-coalesce` 线程：[spawn_flusher] 由
//! `run_request_loop` 起、[ExternalCoalescer::shutdown] + join 停。窗口状态
//! （`Mutex<Vec<String>> + Condvar`）随 [crate::web_api::ServerContext] 走，
//! 单测可直接 [flush_once] 驱动，不依赖线程调度。
//!
//! # 计数口径
//!
//! 端点收下即记 `accepts`（直送与合并都算，发送方拿到 `ok:true`）；忙时收下的
//! 那条另记 `busy`（面板读作「忙碌合并」）；窗口满兜底丢弃只记 `busy`。
//!
//! # 顺序
//!
//! 窗口开着时 handler **不尝试直送**、只追加；常驻线程**不把窗口取空**、
//! 只快照 + 成功后按条数移除。于是「到达顺序」在两条路径上都成立：新到的文本
//! 要么排在已有批次之后，要么在窗口清空后走直送。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::web_api::ServerContext;

/// 合并窗口内多条文本之间的分隔符（拼成一条时插入）。
pub(crate) const COALESCE_SEP: &str = "\n";

/// 合并窗口的字符上限（含分隔符）。超了回退旧的 `busy`（丢弃 + 退避重试）。
///
/// 与单条请求的 `MAX_TEXT_LEN`（2000）取同一个数：窗口不是第二个正文入口，
/// 它只是「把几条短消息攒成一条」。正常弹幕窗口远小于这个数。
pub(crate) const MAX_WINDOW_CHARS: usize = 2000;

/// 主链仍忙时的重试间隔（只在窗口非空时轮询，空闲线程阻塞在 Condvar 上）。
const RETRY_INTERVAL: Duration = Duration::from_millis(20);

/// 注入提供者：与 `external_routes` 的单测注入点同型。
///
/// `Some(true)` = 已接受；`Some(false)` = 忙碌；`None` = supervisor 未就绪。
pub(crate) type InjectFn<'a> = dyn Fn(&ServerContext, String) -> Option<bool> + 'a;

/// 生产注入路径：借出 supervisor 并 `say`。
pub(crate) fn supervisor_inject(ctx: &ServerContext, text: String) -> Option<bool> {
    ctx.try_get_supervisor().map(|s| s.say(text))
}

/// 窗口状态：按到达顺序的文本 + 已占字符数（含分隔符）。
#[derive(Default)]
struct State {
    texts: Vec<String>,
    chars: usize,
}

struct Inner {
    state: Mutex<State>,
    cv: Condvar,
    shutdown: AtomicBool,
}

/// 忙时合并窗口（`Clone` 廉价：内部 `Arc`，所有 handler / 线程共享同一份）。
#[derive(Clone)]
pub struct ExternalCoalescer {
    inner: Arc<Inner>,
}

impl Default for ExternalCoalescer {
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
                cv: Condvar::new(),
                shutdown: AtomicBool::new(false),
            }),
        }
    }
}

impl ExternalCoalescer {
    /// 窗口是否已有积压（= handler 应追加而不是尝试直送）。
    pub(crate) fn has_backlog(&self) -> bool {
        !self.lock().texts.is_empty()
    }

    /// 把一条文本追加进窗口。
    ///
    /// 超过 [MAX_WINDOW_CHARS] → [PushOutcome::Full]（调用方回退旧 `busy`）。
    pub(crate) fn push(&self, text: String) -> PushOutcome {
        let mut st = self.lock();
        let sep = if st.texts.is_empty() {
            0
        } else {
            COALESCE_SEP.chars().count()
        };
        let len = text.chars().count();
        if st.chars + sep + len > MAX_WINDOW_CHARS {
            return PushOutcome::Full;
        }
        st.chars += sep + len;
        st.texts.push(text);
        drop(st);
        self.inner.cv.notify_one();
        PushOutcome::Queued
    }

    /// 快照当前窗口（拼好的正文 + 参与拼接的条数）；窗口空 → `None`。
    ///
    /// **不取走**：注入成功后才 [ExternalCoalescer::drain]，于是重试期间新到的
    /// 文本仍排在这批之后，顺序不变。
    pub(crate) fn snapshot(&self) -> Option<(String, usize)> {
        let st = self.lock();
        if st.texts.is_empty() {
            return None;
        }
        Some((st.texts.join(COALESCE_SEP), st.texts.len()))
    }

    /// 注入成功后移除刚注入的前 `count` 条（重试期间追加的保留）。
    pub(crate) fn drain(&self, count: usize) {
        let mut st = self.lock();
        let n = count.min(st.texts.len());
        st.texts.drain(0..n);
        st.chars = window_chars(&st.texts);
    }

    /// 丢弃窗口里全部文本，返回丢弃条数（supervisor 不在时用）。
    pub(crate) fn clear(&self) -> usize {
        let mut st = self.lock();
        let n = st.texts.len();
        st.texts.clear();
        st.chars = 0;
        n
    }

    /// 常驻线程：阻塞等到窗口非空（`true`）或服务停机（`false`）。
    fn wait_backlog(&self) -> bool {
        let mut st = self.lock();
        loop {
            if self.inner.shutdown.load(Ordering::Acquire) {
                return false;
            }
            if !st.texts.is_empty() {
                return true;
            }
            st = match self.inner.cv.wait(st) {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }

    /// 主链仍忙：最多等 `RETRY_INTERVAL` 再试；`false` = 已停机。
    fn sleep_retry(&self) -> bool {
        let st = self.lock();
        if self.inner.shutdown.load(Ordering::Acquire) {
            return false;
        }
        let _ = self.inner.cv.wait_timeout(st, RETRY_INTERVAL);
        !self.inner.shutdown.load(Ordering::Acquire)
    }

    /// 服务停机：唤醒常驻线程并让它退出。
    pub(crate) fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Release);
        self.inner.cv.notify_all();
    }

    /// 测试专用：窗口里当前有几条。
    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.lock().texts.len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        match self.inner.state.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// [ExternalCoalescer::push] 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PushOutcome {
    /// 已进窗口，等主链空出来后合并注入。
    Queued,
    /// 窗口满：本条没进窗口（调用方回退旧的 `busy`）。
    Full,
}

/// [flush_once] 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlushOutcome {
    /// 已把整窗拼成一条注入（成功）。
    Injected,
    /// 窗口空（无文本可注入）。
    Empty,
    /// 主链仍忙，下一轮再试。
    Busy,
    /// supervisor 不在（未装配 / 已回收）。
    Unavailable,
}

/// 尝试把当前窗口合并注入一次（**单次尝试**，不重试）。
///
/// 生产路径由 [spawn_flusher] 循环调用；单测直接调用即可确定性地驱动
/// 「忙 → 空闲 → 注入一次」的整条语义。
pub(crate) fn flush_once(ctx: &ServerContext, inject: &InjectFn<'_>) -> FlushOutcome {
    let coalescer = &ctx.external_coalescer;
    let Some((joined, count)) = coalescer.snapshot() else {
        return FlushOutcome::Empty;
    };
    // 上一条还没处理完：**先不注入**（这一批要和窗口后续到达的文本合成一条）。
    if ctx.try_get_supervisor().is_some_and(|s| s.turn_in_flight()) {
        return FlushOutcome::Busy;
    }
    match inject(ctx, joined) {
        Some(true) => {
            coalescer.drain(count);
            FlushOutcome::Injected
        }
        Some(false) => FlushOutcome::Busy,
        None => FlushOutcome::Unavailable,
    }
}

/// 起常驻合并线程（每个 `run_request_loop` 一个；停机时 shutdown + join）。
///
/// 线程只做三件事：等窗口非空 → [flush_once] → 忙则短睡重试。窗口空时
/// **不轮询**（阻塞在 Condvar 上）。
pub(crate) fn spawn_flusher(ctx: ServerContext) -> std::thread::JoinHandle<()> {
    let coalescer = ctx.external_coalescer.clone();
    std::thread::Builder::new()
        .name("external-coalesce".to_string())
        .spawn(move || {
            loop {
                if !coalescer.wait_backlog() {
                    return;
                }
                loop {
                    match flush_once(&ctx, &supervisor_inject) {
                        FlushOutcome::Injected | FlushOutcome::Empty => {
                            if !coalescer.has_backlog() {
                                break;
                            }
                        }
                        FlushOutcome::Busy => {
                            if !coalescer.sleep_retry() {
                                return;
                            }
                        }
                        FlushOutcome::Unavailable => {
                            let dropped = coalescer.clear();
                            tracing::warn!(
                                dropped,
                                "external-coalesce: supervisor 不在，合并窗口的待注入文本已丢弃"
                            );
                            break;
                        }
                    }
                }
            }
        })
        .expect("spawn external-coalesce thread")
}

/// 窗口文本的字符数（含分隔符），用于 push 上限与 drain 后重算。
fn window_chars(texts: &[String]) -> usize {
    let body: usize = texts.iter().map(|t| t.chars().count()).sum();
    body + texts.len().saturating_sub(1) * COALESCE_SEP.chars().count()
}
