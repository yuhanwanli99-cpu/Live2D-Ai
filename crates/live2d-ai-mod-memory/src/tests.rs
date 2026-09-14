//! 运行时级回归：Mod 生命周期 + `TurnPrompt` 一轮的记/检索/注入 + 停用清残留。
//!
//! 纯逻辑（分词 / 打分 / top-k / marker / 路径）的单测在 `strategy.rs`，
//! JSONL 读写的单测在 `store.rs`；这里只测把它们接起来的行为。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::*;

// ---------------------------------------------------------------- 测试替身

/// 记录 `register_settings` / `subscribe` 调用的注册器。
#[derive(Default)]
struct RecordingRegistrar {
    specs: Vec<ModSettingsSpec>,
    topics: Vec<ModEventTopic>,
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
struct FakeHost {
    settings: Arc<Mutex<serde_json::Value>>,
    patches: Arc<Mutex<Vec<serde_json::Value>>>,
    logs: Arc<Mutex<Vec<String>>>,
    accept: Arc<AtomicBool>,
}

impl FakeHost {
    fn new(system_prompt: &str) -> Self {
        Self {
            settings: Arc::new(Mutex::new(serde_json::json!({
                "persona": {"system_prompt": system_prompt, "max_history_pairs": 8}
            }))),
            patches: Arc::new(Mutex::new(Vec::new())),
            logs: Arc::new(Mutex::new(Vec::new())),
            accept: Arc::new(AtomicBool::new(true)),
        }
    }

    fn services(&self, config_path: &str) -> ModServices {
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

    fn main_prompt(&self) -> String {
        self.settings.lock().expect("settings lock")["persona"]["system_prompt"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    fn patches(&self) -> Vec<serde_json::Value> {
        self.patches.lock().expect("patches lock").clone()
    }

    fn logs(&self) -> Vec<String> {
        self.logs.lock().expect("logs lock").clone()
    }
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_dir(tag: &str) -> PathBuf {
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
fn runtime(dir: &std::path::Path, host: &FakeHost, config: serde_json::Value) -> MemoryRuntime {
    let config_path = dir.join("live2d-ai.toml").display().to_string();
    MemoryRuntime::new(
        host.services(&config_path),
        MemoryConfig::from_value(&config),
    )
}

// ---------------------------------------------------------------- 生命周期

#[test]
fn start_registers_schema_and_subscribes_turn_prompt() {
    let dir = temp_dir("start");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    let mut reg = RecordingRegistrar::default();
    rt.start(&mut reg).expect("start 应成功");
    assert!(rt.is_registered());
    assert_eq!(reg.specs.len(), 1);
    assert_eq!(reg.specs[0].mod_id, "memory");
    assert!(reg.specs[0].validate().is_ok(), "字段 key 必须唯一");
    assert_eq!(reg.topics, vec![ModEventTopic::TurnPrompt]);
    rt.shutdown().unwrap();
    assert!(!rt.is_registered());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn descriptor_and_settings_spec_declare_no_second_enabled() {
    assert_eq!(DESCRIPTOR.id, "memory");
    assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
    let spec = memory_settings_spec();
    let keys: Vec<&str> = spec.fields.iter().map(ModSettingField::key).collect();
    assert_eq!(
        keys,
        vec!["store_path", "top_k", "max_records", "enabled_injection"]
    );
    assert!(
        !keys.contains(&"enabled"),
        "启停唯一真源是 manifest，schema 不许有第二个 enabled"
    );
    assert_eq!(spec.mod_id, "memory");
}

#[test]
fn factory_creates_runtime_with_static_spec() {
    let host = FakeHost::new("基础人设");
    assert_eq!(FACTORY.descriptor().id, "memory");
    assert!(
        FACTORY.settings_spec().is_some(),
        "静态 schema 必须可取自未启用状态"
    );
    let rt: Box<dyn ModRuntime> = FACTORY
        .create(host.services(""), serde_json::json!({}))
        .expect("create 应成功");
    drop(rt);
}

#[test]
fn non_turn_prompt_topics_are_ignored() {
    let dir = temp_dir("ignore");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnStarted, "7").unwrap();
    rt.on_event(ModEventTopic::TextDelta, "你好").unwrap();
    assert!(rt.retrieve("你好").is_empty(), "非 TurnPrompt 不记不注入");
    assert!(host.patches().is_empty());
    assert!(!dir.join("memory.jsonl").exists());
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 记 / 检索

#[test]
fn first_turn_is_remembered_but_not_injected() {
    let dir = temp_dir("first");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    let lines = std::fs::read_to_string(dir.join("memory.jsonl")).unwrap();
    assert_eq!(lines.lines().count(), 1, "正文必须落盘");
    assert!(lines.contains("今天天气很好"));
    assert!(
        host.patches().is_empty(),
        "刚记住的当轮正文不得自命中并注入（无历史 = no-op）"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn related_second_turn_injects_previous_memory_into_next_round() {
    let dir = temp_dir("inject");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();

    let patches = host.patches();
    assert_eq!(patches.len(), 1, "只应有一次注入");
    let prompt = patches[0]["persona"]["system_prompt"]
        .as_str()
        .expect("patch 形状必须是 persona.system_prompt");
    assert!(prompt.starts_with("基础人设\n\n"), "{prompt}");
    assert!(prompt.contains(MEMORY_MARKER_BEGIN));
    assert!(prompt.contains("- 今天天气很好"));
    assert!(prompt.ends_with(MEMORY_MARKER_END));
    // 主链快照已被 change（下一轮请求体才会带上它）。
    assert_eq!(host.main_prompt(), prompt);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unrelated_turn_does_not_inject() {
    let dir = temp_dir("unrelated");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "量子力学导论")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错")
        .unwrap();
    assert!(host.patches().is_empty(), "零相关 = no-op，不写盘");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn injection_disabled_still_remembers_but_never_writes_config() {
    let dir = temp_dir("noinject");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"enabled_injection": false}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("memory.jsonl"))
            .unwrap()
            .lines()
            .count(),
        2,
        "关掉注入不影响记忆"
    );
    assert!(host.patches().is_empty());
    assert_eq!(host.main_prompt(), "基础人设");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn identical_repeat_turn_is_idempotent_no_second_write() {
    let dir = temp_dir("idem");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    for _ in 0..3 {
        rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
            .unwrap();
    }
    let patches = host.patches();
    assert_eq!(patches.len(), 1, "重拼结果与当前一致 → 第二次起不再写盘");
    let prompt = host.main_prompt();
    assert_eq!(
        prompt.matches("- 今天天气很好").count(),
        1,
        "同一句话不重复注入: {prompt}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn max_records_bounds_the_retrieval_window() {
    let dir = temp_dir("window");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"max_records": 1}));
    rt.on_event(ModEventTopic::TurnPrompt, "量子力学导论")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "相对论纲要")
        .unwrap();
    assert!(
        rt.retrieve("量子力学导论").is_empty(),
        "窗口=1 时最老的记录已不在检索面内"
    );
    assert_eq!(rt.retrieve("相对论纲要").len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 边界 / no-op

#[test]
fn empty_prompt_is_noop() {
    let dir = temp_dir("empty");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "   \n\t ").unwrap();
    assert!(!dir.join("memory.jsonl").exists(), "空正文不落盘");
    assert!(host.patches().is_empty());
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["turns_seen"], 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_store_path_is_warned_noop_not_panic() {
    let host = FakeHost::new("基础人设");
    // 无 config_path、store_path 空 → 路径不可解析：必须 no-op + warn。
    let mut rt = MemoryRuntime::new(
        host.services(""),
        MemoryConfig::from_value(&serde_json::json!({})),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    assert!(host.patches().is_empty());
    assert!(
        host.logs().iter().any(|l| l.contains("存储路径未配置")),
        "必须留下可排障的 warn: {:?}",
        host.logs()
    );
    assert_eq!(
        rt.state_json().unwrap()["store_path"],
        serde_json::Value::Null
    );
}

#[test]
fn rejected_apply_settings_is_warned_not_fatal() {
    let dir = temp_dir("reject");
    let host = FakeHost::new("基础人设");
    host.accept.store(false, Ordering::SeqCst);
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    // 第二次相关输入会尝试注入，但 host 拒绝。
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    assert!(host.patches().is_empty());
    assert_eq!(host.main_prompt(), "基础人设", "被拒后主链快照不变");
    assert!(
        host.logs()
            .iter()
            .any(|l| l.contains("apply_settings 拒绝")),
        "拒绝必须可见: {:?}",
        host.logs()
    );
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["injected"], 0, "被拒不计数");
    assert_eq!(snap["remembered"], 2, "记忆仍然照记");
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 停用 / 状态

#[test]
fn shutdown_strips_injected_block() {
    let dir = temp_dir("shutdown");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    assert!(host.main_prompt().contains(MEMORY_MARKER_BEGIN));
    rt.shutdown().unwrap();
    assert_eq!(host.main_prompt(), "基础人设", "停用必须清掉注入块");
    assert!(!rt.is_registered());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shutdown_without_block_does_not_write() {
    let dir = temp_dir("shutdown-clean");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.shutdown().unwrap();
    assert!(host.patches().is_empty(), "没有残留就不该写盘");
    assert_eq!(host.main_prompt(), "基础人设");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn state_json_snapshot_shape_is_read_only() {
    let dir = temp_dir("state");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(
        &dir,
        &host,
        serde_json::json!({"top_k": 5, "max_records": 42, "enabled_injection": false}),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    let snap = rt.state_json().expect("必须提供快照");
    assert_eq!(snap["top_k"], 5);
    assert_eq!(snap["max_records"], 42);
    assert_eq!(snap["enabled_injection"], false);
    assert_eq!(snap["turns_seen"], 1);
    assert_eq!(snap["remembered"], 1);
    assert!(
        snap["store_path"]
            .as_str()
            .unwrap()
            .ends_with("memory.jsonl"),
        "缺省路径必须落在配置文件同目录: {snap}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- 配置

#[test]
fn config_defaults_and_clamping() {
    assert_eq!(
        MemoryConfig::from_value(&serde_json::json!({})),
        MemoryConfig::default()
    );
    let clamped = MemoryConfig::from_value(&serde_json::json!({
        "top_k": 99,
        "max_records": 0,
        "enabled_injection": false,
        "store_path": "  /tmp/mem.jsonl  ",
    }));
    assert_eq!(clamped.top_k, MAX_TOP_K);
    assert_eq!(clamped.max_records, MIN_MAX_RECORDS, "0 → 钳到下限而非关掉");
    assert!(!clamped.enabled_injection);
    assert_eq!(
        clamped.store_path, "  /tmp/mem.jsonl  ",
        "路径原样保留，解析时 trim"
    );
    // 类型不对 → 用缺省，不失败。
    let bad = MemoryConfig::from_value(&serde_json::json!({
        "top_k": "3", "max_records": null, "enabled_injection": 1,
    }));
    assert_eq!(bad.top_k, DEFAULT_TOP_K);
    assert_eq!(bad.max_records, DEFAULT_MAX_RECORDS);
    assert!(bad.enabled_injection);
    // 负数钳到下限。
    let negative = MemoryConfig::from_value(&serde_json::json!({"top_k": -4}));
    assert_eq!(negative.top_k, MIN_TOP_K);
}

#[test]
fn config_store_path_resolution_matches_strategy() {
    let cfg = MemoryConfig::from_value(&serde_json::json!({"store_path": "sub/mem.jsonl"}));
    assert_eq!(
        cfg.resolve_store_path("/etc/live2d/live2d-ai.toml"),
        Some(PathBuf::from("/etc/live2d/sub/mem.jsonl"))
    );
    let default_cfg = MemoryConfig::default();
    assert_eq!(
        default_cfg.resolve_store_path("/etc/live2d/live2d-ai.toml"),
        Some(PathBuf::from("/etc/live2d/memory.jsonl"))
    );
    assert_eq!(default_cfg.resolve_store_path(""), None);
}
