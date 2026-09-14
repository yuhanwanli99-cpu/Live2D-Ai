//! `tests.rs` 与 `quality_tests.rs` 共用的测试替身。
//!
//! 拆出来的原因：Wave 3 新增检索质量基线测试，两个模块都要「假 host +
//! 临时目录 + 组 runtime」这套脚手架；复制一份就是两份真相。

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

/// 假 host：`apply_settings` 会真的改「主链提示词」快照并记录 patch；
/// `settings` 读取同一份快照，从而复现「读 base → 剥旧块 → 重拼 → 写回」。
#[derive(Clone)]
pub(crate) struct FakeHost {
    settings: Arc<Mutex<serde_json::Value>>,
    patches: Arc<Mutex<Vec<serde_json::Value>>>,
    logs: Arc<Mutex<Vec<String>>>,
    accept: Arc<AtomicBool>,
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

    /// 让 `apply_settings` 开始拒绝写入（模拟配置不可写 / busy）。
    pub(crate) fn set_accept(&self, value: bool) {
        self.accept.store(value, Ordering::SeqCst);
    }

    pub(crate) fn patches(&self) -> Vec<serde_json::Value> {
        self.patches.lock().expect("patches lock").clone()
    }

    pub(crate) fn logs(&self) -> Vec<String> {
        self.logs.lock().expect("logs lock").clone()
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
