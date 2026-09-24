//! `tests.rs` 与 `quality_tests.rs` 共用的测试替身。
//!
//! 拆出来的原因：Wave 3 新增检索质量基线测试，两个模块都要「假 host +
//! 临时目录 + 组 runtime」这套脚手架；复制一份就是两份真相。
//!
//! L1（2026-09-15）新增 [FakeSessionPrompts]：会话分桶要验「A 的记忆不串到 B」，
//! 而 `ModServices::session_prompts` 缺省是 [live2d_ai_mod_system::NoSessionPrompts]
//! （写什么都不落地）——没有这个替身，会话注入看起来永远 no-op。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use live2d_ai_mod_system::*;

use crate::{MemoryConfig, MemoryRuntime};

/// 记录 `register_settings` / `subscribe` 调用的注册器。
#[derive(Default)]
pub(crate) struct RecordingRegistrar {
    pub(crate) specs: Vec<ModSettingsSpec>,
    pub(crate) topics: Vec<ModEventTopic>,
}

impl ModRegistrar for RecordingRegistrar {
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
        self.specs.push(spec);
        Ok(())
    }
    fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
        self.topics.push(topic);
        Ok(SubscriptionId(1))
    }
    fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
        Ok(())
    }
}

/// 内存版 `SessionPromptSink`（与宿主 `SessionScopeStore` 同口径：id 过
/// sanitize、BTreeMap、**按来源槽分组**、可开关）。
///
/// L1 修复（2026-09-15）起按 owner 分槽：host 的读取是「各槽按固定顺序组合」，
/// 因此这个替身也必须真的分槽，否则「清掉 memory 的槽之后 persona 的贡献还在」
/// 这类回归在单测里根本验不到。
#[derive(Clone)]
pub(crate) struct FakeSessionPrompts {
    inner: Arc<Mutex<FakeSessionInner>>,
}

struct FakeSessionInner {
    enabled: bool,
    /// 会话 id → (owner → 文本)。
    slots: BTreeMap<String, BTreeMap<String, String>>,
    active: Option<String>,
}

impl Default for FakeSessionInner {
    fn default() -> Self {
        // 缺省可用：宿主在真实装配里总会注入 `SessionScopeStore`。
        Self {
            enabled: true,
            slots: BTreeMap::new(),
            active: None,
        }
    }
}

impl FakeSessionPrompts {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FakeSessionInner::default())),
        }
    }

    /// 关掉能力（模拟「宿主没注入」的环境，测降级路径）。
    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.inner.lock().expect("session lock").enabled = enabled;
    }

    /// 读某会话的**组合结果**（不经过 trait 对象，测试断言用）。
    pub(crate) fn prompt_for(&self, session: &str) -> Option<String> {
        SessionPromptSink::get(self, session)
    }

    /// 读**单个来源槽**的文本（断言「本 Mod 只写了自己的槽」用）。
    pub(crate) fn slot(&self, owner: &str, session: &str) -> Option<String> {
        let id = sanitize_session_id(session)?;
        self.inner
            .lock()
            .expect("session lock")
            .slots
            .get(&id)?
            .get(owner)
            .cloned()
    }

    /// 直接写某个来源槽（模拟另一个 Mod，如 persona 写角色卡）。
    pub(crate) fn set_slot(&self, owner: &str, session: &str, prompt: &str) {
        SessionPromptSink::set_owned(self, owner, session, prompt);
    }

    pub(crate) fn stored_sessions(&self) -> Vec<String> {
        SessionPromptSink::sessions(self)
    }
}

impl SessionPromptSink for FakeSessionPrompts {
    fn enabled(&self) -> bool {
        self.inner.lock().expect("session lock").enabled
    }
    fn get(&self, session: &str) -> Option<String> {
        let id = sanitize_session_id(session)?;
        let inner = self.inner.lock().expect("session lock");
        let slots = inner.slots.get(&id)?;
        live2d_ai_mod_system::compose_session_prompt(
            slots
                .iter()
                .map(|(owner, text)| (owner.as_str(), text.as_str())),
        )
    }
    fn set_owned(&self, owner: &str, session: &str, prompt: &str) {
        let Some(id) = sanitize_session_id(session) else {
            return;
        };
        let mut inner = self.inner.lock().expect("session lock");
        if prompt.is_empty() {
            // 与宿主同口径：空串撤销该槽；会话空了整条删。
            let mut drop_session = false;
            if let Some(slots) = inner.slots.get_mut(&id) {
                slots.remove(owner);
                drop_session = slots.is_empty();
            }
            if drop_session {
                inner.slots.remove(&id);
            }
            return;
        }
        inner
            .slots
            .entry(id)
            .or_default()
            .insert(owner.to_string(), prompt.to_string());
    }
    fn clear_owned(&self, owner: &str, session: &str) -> bool {
        let Some(id) = sanitize_session_id(session) else {
            return false;
        };
        let mut inner = self.inner.lock().expect("session lock");
        let mut removed = false;
        let mut drop_session = false;
        if let Some(slots) = inner.slots.get_mut(&id) {
            removed = slots.remove(owner).is_some();
            drop_session = slots.is_empty();
        }
        if drop_session {
            inner.slots.remove(&id);
        }
        removed
    }
    fn clear_owner(&self, owner: &str) {
        let mut inner = self.inner.lock().expect("session lock");
        let ids: Vec<String> = inner.slots.keys().cloned().collect();
        for id in ids {
            let mut drop_session = false;
            if let Some(slots) = inner.slots.get_mut(&id) {
                slots.remove(owner);
                drop_session = slots.is_empty();
            }
            if drop_session {
                inner.slots.remove(&id);
            }
        }
    }
    fn contributions(&self, session: &str) -> Vec<(String, String)> {
        let Some(id) = sanitize_session_id(session) else {
            return Vec::new();
        };
        let inner = self.inner.lock().expect("session lock");
        let Some(slots) = inner.slots.get(&id) else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = slots
            .iter()
            .map(|(owner, text)| (owner.clone(), text.clone()))
            .collect();
        out.sort_by(|a, b| {
            live2d_ai_mod_system::owner_merge_rank(&a.0)
                .cmp(&live2d_ai_mod_system::owner_merge_rank(&b.0))
        });
        out
    }
    fn set(&self, session: &str, prompt: &str) {
        self.set_owned("", session, prompt);
    }
    fn clear(&self, session: &str) -> bool {
        let Some(id) = sanitize_session_id(session) else {
            return false;
        };
        self.inner
            .lock()
            .expect("session lock")
            .slots
            .remove(&id)
            .is_some()
    }
    fn clear_all(&self) {
        self.inner.lock().expect("session lock").slots.clear();
    }
    fn sessions(&self) -> Vec<String> {
        self.inner
            .lock()
            .expect("session lock")
            .slots
            .keys()
            .cloned()
            .collect()
    }
    fn active(&self) -> Option<String> {
        self.inner.lock().expect("session lock").active.clone()
    }
    fn set_active(&self, session: Option<&str>) {
        let normalized = session.and_then(sanitize_session_id);
        self.inner.lock().expect("session lock").active = normalized;
    }
}

/// 假 host：`apply_settings` 会真的改「主链提示词」快照并记录 patch；
/// `settings` 读取同一份快照，从而复现「读 base → 剥旧块 → 重拼 → 写回」；
/// `session_prompts` 是一张内存覆盖表（[FakeSessionPrompts]）。
#[derive(Clone)]
pub(crate) struct FakeHost {
    settings: Arc<Mutex<serde_json::Value>>,
    patches: Arc<Mutex<Vec<serde_json::Value>>>,
    logs: Arc<Mutex<Vec<String>>>,
    accept: Arc<AtomicBool>,
    sessions: FakeSessionPrompts,
}

impl FakeHost {
    pub(crate) fn new(system_prompt: &str) -> Self {
        Self {
            settings: Arc::new(Mutex::new(serde_json::json!({
                "persona": {"system_prompt": system_prompt, "max_history_pairs": 8}
            }))),
            patches: Arc::new(Mutex::new(Vec::new())),
            logs: Arc::new(Mutex::new(Vec::new())),
            accept: Arc::new(AtomicBool::new(true)),
            sessions: FakeSessionPrompts::new(),
        }
    }

    pub(crate) fn services(&self, config_path: &str) -> ModServices {
        let settings_for_read = self.settings.clone();
        let settings_for_write = self.settings.clone();
        let patches = self.patches.clone();
        let logs = self.logs.clone();
        let accept = self.accept.clone();
        ModServices::new(
            ModActionSender::new(|_| false),
            SaySender::new(|_| true),
            ModEventSender::new(|_, _| true),
            ModLogger::new(move |_, msg| logs.lock().expect("logs lock").push(msg.to_string())),
        )
        .with_apply_settings(ModSettingsApplier::new(move |patch| {
            if !accept.load(Ordering::SeqCst) {
                return false;
            }
            if let Some(prompt) = patch
                .get("persona")
                .and_then(|p| p.get("system_prompt"))
                .and_then(serde_json::Value::as_str)
            {
                settings_for_write
                    .lock()
                    .expect("settings lock")
                    .as_object_mut()
                    .expect("object")
                    .insert(
                        "persona".to_string(),
                        serde_json::json!({"system_prompt": prompt}),
                    );
            }
            patches.lock().expect("patches lock").push(patch);
            true
        }))
        .with_settings_reader(ModSettingsReader::new(move || {
            settings_for_read.lock().expect("settings lock").clone()
        }))
        .with_session_prompts(ModSessionPrompts::new(self.sessions.clone()))
        .with_config_path(config_path)
    }

    pub(crate) fn main_prompt(&self) -> String {
        self.settings.lock().expect("settings lock")["persona"]["system_prompt"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    /// 模拟**另一个**写 `persona.system_prompt` 的 Mod（如 persona）落地一次写：
    /// 直接改快照，不经过本 Mod 的 `apply_settings` 记录——这是 last-writer-wins
    /// 的「前写者/后写者」注入点。
    pub(crate) fn set_main_prompt(&self, prompt: &str) {
        self.settings
            .lock()
            .expect("settings lock")
            .as_object_mut()
            .expect("object")
            .insert(
                "persona".to_string(),
                serde_json::json!({"system_prompt": prompt}),
            );
    }

    pub(crate) fn patches(&self) -> Vec<serde_json::Value> {
        self.patches.lock().expect("patches lock").clone()
    }

    pub(crate) fn logs(&self) -> Vec<String> {
        self.logs.lock().expect("logs lock").clone()
    }

    /// 会话覆盖表（断言 A/B 不串用）。
    pub(crate) fn sessions(&self) -> &FakeSessionPrompts {
        &self.sessions
    }
}

static SEQ: AtomicU64 = AtomicU64::new(0);

/// 每个测试一个独立临时目录（不引 tempfile 依赖）。
pub(crate) fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "l2d-memory-rt-{tag}-{}-{nanos}-{seq}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 建一个「配置文件在 `dir` 里」的 runtime（缺省 store_path → `dir/memory.jsonl`）。
pub(crate) fn runtime(dir: &Path, host: &FakeHost, config: serde_json::Value) -> MemoryRuntime {
    let config_path = dir.join("live2d-ai.toml").display().to_string();
    MemoryRuntime::new(
        host.services(&config_path),
        MemoryConfig::from_value(&config),
    )
}
