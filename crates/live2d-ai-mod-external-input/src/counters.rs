//! 外部接入的可观察计数（Wave 3，2026-09-14）。
//!
//! # 为什么是进程级 `AtomicU64`
//!
//! 计数要被**两个拿不到彼此引用**的地方同时读写：
//!
//! - **写**：web_api handler（`crates/live2d-ai-desktop/src/web_api/external_routes.rs`，
//!   HTTP 请求线程）只有 `ServerContext`，能借到注册表里的 config 快照
//!   （`ModRegistry::config`），**拿不到** `ExternalInputRuntime` 实例；
//! - **读**：`ModRuntime::state_json`（`GET /api/v1/mods/external-input/state`）
//!   在 runtime 实例上被调用。
//!
//! 所以计数住在本模块的进程级单例里，两边都经这里的自由函数读写——同一份数字，
//! 不引入第二处真相。计数只增不减（`v2_ignored` 例外，见下）、不做持久化：
//! 进程重启即归零，这是「本机运行观察值」的定位。
//!
//! `command reset_counters`（产品级加强波次）是**唯一**把它们清零的路径：见
//! [`ExternalInputCounters::reset`]——返回清零前的快照，面板据此说「清了哪些」。
//!
//! # 语义（每个计数对应 handler 的哪条分支）
//!
//! | 计数 | 含义 | 对应分支 |
//! |---|---|---|
//! | `accepts` | 文本已送进主链路（`supervisor.say` 返回 `true`） | 200 `ok:true` |
//! | `busy` | supervisor 忙碌，文本被丢弃 | 200 `ok:false`（code `busy`） |
//! | `rejects` | **策略拒绝**：token 鉴权失败（401）或 Mod 停用（403 `mod_disabled`） | 401 / 403 |
//! | `v2_ignored` | sidecar 上报的 `SEND_GIFT_V2` 忽略累计值 | body 可选字段 `v2_ignored` |
//!
//! `rejects` **不含** 400（负载非法 / 超长）与 503（supervisor 未就绪）：前者是
//! 调用方 bug、后者是服务未装配，都不是「一条合法事件被策略挡下」。`415` /
//! `405` 同理不计数。
use std::sync::atomic::{AtomicU64, Ordering};

/// 进程内计数快照的 JSON 键（`state_json` 与前端约定的契约）。
pub const COUNTER_KEYS: [&str; 4] = ["accepts", "rejects", "busy", "v2_ignored"];

/// 四个 `AtomicU64` 组成的计数集。
///
/// 单例见本模块底部的 `COUNTERS`；测试也可独立 `new()` 构造一个实例。
pub struct ExternalInputCounters {
    accepts: AtomicU64,
    rejects: AtomicU64,
    busy: AtomicU64,
    v2_ignored: AtomicU64,
}

impl ExternalInputCounters {
    /// 全零计数（`const`：可作进程级 `static` 初始化）。
    pub const fn new() -> Self {
        Self {
            accepts: AtomicU64::new(0),
            rejects: AtomicU64::new(0),
            busy: AtomicU64::new(0),
            v2_ignored: AtomicU64::new(0),
        }
    }

    /// 主链路接受了这条文本。
    pub fn record_accept(&self) {
        self.accepts.fetch_add(1, Ordering::Relaxed);
    }

    /// 策略拒绝：鉴权失败或 Mod 停用（文本未进入主链路）。
    pub fn record_reject(&self) {
        self.rejects.fetch_add(1, Ordering::Relaxed);
    }

    /// supervisor 忙碌，本条文本被丢弃。
    pub fn record_busy(&self) {
        self.busy.fetch_add(1, Ordering::Relaxed);
    }

    /// sidecar 上报 `SEND_GIFT_V2` 忽略数。
    ///
    /// **覆盖写**（`store`）而非累加：sidecar 报的是它自身的累计值，重复上报
    /// 同一累计值必须幂等，否则每次心跳都会把计数越加越大。
    pub fn record_v2_ignored(&self, total: u64) {
        self.v2_ignored.store(total, Ordering::Relaxed);
    }

    /// 只读快照（`state_json` 的内容；键见 [`COUNTER_KEYS`]）。
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({
            "accepts": self.accepts.load(Ordering::Relaxed),
            "rejects": self.rejects.load(Ordering::Relaxed),
            "busy": self.busy.load(Ordering::Relaxed),
            "v2_ignored": self.v2_ignored.load(Ordering::Relaxed),
        })
    }

    /// 清零四个计数，并返回**清零前**的快照（`reset_counters` 命令的唯一实现）。
    ///
    /// 逐字段 `swap(0)`：并发写不会丢——写线程要么在 swap 之前计入返回的
    /// 快照、要么在 swap 之后计入新一轮，两者都有归属；返回体也正好是
    /// 「清零那一刻的数字」，面板可以如实说「清了哪些」。
    pub fn reset(&self) -> serde_json::Value {
        serde_json::json!({
            "accepts": self.accepts.swap(0, Ordering::Relaxed),
            "rejects": self.rejects.swap(0, Ordering::Relaxed),
            "busy": self.busy.swap(0, Ordering::Relaxed),
            "v2_ignored": self.v2_ignored.swap(0, Ordering::Relaxed),
        })
    }
}

impl Default for ExternalInputCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// 进程级单例（handler 与 runtime 共用）。
static COUNTERS: ExternalInputCounters = ExternalInputCounters::new();

/// 直接拿进程级计数集（测试 / 需要自定义读写时）。
///
/// 名字避开模组路径歧义：模块叫 `counters`，故访问器不叫同名。
pub fn global() -> &'static ExternalInputCounters {
    &COUNTERS
}

/// handler 分支：文本已进主链路。
pub fn record_accept() {
    COUNTERS.record_accept();
}

/// handler 分支：策略拒绝（401 / 403 `mod_disabled`）。
pub fn record_reject() {
    COUNTERS.record_reject();
}

/// handler 分支：supervisor 忙碌。
pub fn record_busy() {
    COUNTERS.record_busy();
}

/// handler 分支：记录 sidecar 上报的 `SEND_GIFT_V2` 忽略累计值。
pub fn record_v2_ignored(total: u64) {
    COUNTERS.record_v2_ignored(total);
}

/// 当前计数快照（`state_json` / 测试断言用）。
pub fn counters_snapshot() -> serde_json::Value {
    COUNTERS.snapshot()
}

/// 进程级单例：清零并返回清零前快照（`ModRuntime::command("reset_counters")` 调它）。
pub fn reset() -> serde_json::Value {
    COUNTERS.reset()
}

/// **测试专用**串行锁：`reset` 会把别的测试的 delta 基准打乱，凡改进程级计数的
/// 测试都要先拿它。生产路径不碰这把锁。
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 进程级计数的 delta 断言必须在 [`test_lock`] 下串行：本模块与 `lib.rs` 的
    /// `command` 回归都会改进程级计数，`reset` 会把别人的基准打乱。
    #[test]
    fn record_and_snapshot_are_consistent() {
        let _guard = test_lock();
        let before = counters_snapshot();
        let base = |k: &str| before[k].as_u64().expect("计数为无符号整数");

        record_accept();
        record_accept();
        record_reject();
        record_busy();
        record_v2_ignored(7);

        let after = counters_snapshot();
        assert_eq!(after["accepts"], base("accepts") + 2);
        assert_eq!(after["rejects"], base("rejects") + 1);
        assert_eq!(after["busy"], base("busy") + 1);
        assert_eq!(after["v2_ignored"], 7, "v2_ignored 是覆盖写（累计值）");
    }

    /// `v2_ignored` 覆盖写幂等：重复上报同一累计值不膨胀。
    #[test]
    fn v2_ignored_report_is_idempotent() {
        let c = ExternalInputCounters::new();
        c.record_v2_ignored(3);
        c.record_v2_ignored(3);
        assert_eq!(c.snapshot()["v2_ignored"], 3);
        c.record_v2_ignored(5);
        assert_eq!(c.snapshot()["v2_ignored"], 5);
    }

    /// 快照形状钉死：四个契约键都在且都是数字。
    #[test]
    fn snapshot_has_exactly_the_contract_keys() {
        let snap = ExternalInputCounters::new().snapshot();
        let obj = snap.as_object().expect("快照是对象");
        assert_eq!(obj.len(), COUNTER_KEYS.len(), "不多不少四个键");
        for k in COUNTER_KEYS {
            assert!(obj[k].is_u64(), "{k} 必须是数字");
        }
    }

    /// 独立实例的默认值全零（`new` / `Default` 一致）。
    #[test]
    fn fresh_instance_is_zeroed() {
        let snap = ExternalInputCounters::default().snapshot();
        for k in COUNTER_KEYS {
            assert_eq!(snap[k], 0, "{k} 初始为 0");
        }
    }

    /// 独立实例 `reset`：返回清零前的四个数字，随后全部归零。
    #[test]
    fn reset_returns_pre_reset_snapshot_and_zeroes() {
        let c = ExternalInputCounters::new();
        c.record_accept();
        c.record_accept();
        c.record_reject();
        c.record_busy();
        c.record_v2_ignored(4);

        let before = c.reset();
        assert_eq!(before["accepts"], 2);
        assert_eq!(before["rejects"], 1);
        assert_eq!(before["busy"], 1);
        assert_eq!(before["v2_ignored"], 4);

        let after = c.snapshot();
        for k in COUNTER_KEYS {
            assert_eq!(after[k], 0, "{k} 清零后为 0");
        }
        // 清零后还能继续记（不是把计数器废掉）。
        c.record_accept();
        assert_eq!(c.snapshot()["accepts"], 1);
    }

    /// 进程级 `reset()`：清零并返回快照（`command reset_counters` 的真实路径）。
    #[test]
    fn global_reset_zeroes_process_counters() {
        let _guard = test_lock();
        record_accept();
        record_reject();
        record_busy();
        record_v2_ignored(9);

        let before = reset();
        assert!(before["accepts"].as_u64().unwrap_or(0) >= 1);
        assert!(before["rejects"].as_u64().unwrap_or(0) >= 1);
        assert!(before["busy"].as_u64().unwrap_or(0) >= 1);
        assert_eq!(before["v2_ignored"], 9);

        let after = counters_snapshot();
        for k in COUNTER_KEYS {
            assert_eq!(after[k], 0, "{k} 进程级清零");
        }
    }
}
