//! `.env` 路径锚定（0.7.0 P1.4 收口）的回归单测。
//!
//! 经 `#[path = "secrets_anchor_tests.rs"]` 挂回 `secrets.rs`，守住父文件 ≤500 行硬门禁。
//!
//! # 为什么大多用「注入参数」而不是改全局锚定
//!
//! 全局锚定是进程级状态：一条测试改了它，同进程并行跑的其它测试就会看到不同
//! 的 `.env` 路径。所以解析逻辑全部走参数注入的 [`resolve_env_file_path`] /
//! [`env_anchor_for_config`]；唯一碰全局锚定的测试用 [`AnchorScope`] 串行化并在
//! Drop 时清掉，断言失败也不会把状态泄漏给别的测试。

use super::*;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

/// 串行化「碰全局锚定」的测试。
static ANCHOR_LOCK: Mutex<()> = Mutex::new(());

/// 进入作用域时拿锁，离开时（含 panic 展开）清掉全局锚定。
struct AnchorScope {
    _guard: MutexGuard<'static, ()>,
}

impl AnchorScope {
    fn new() -> Self {
        Self {
            _guard: ANCHOR_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        }
    }
}

impl Drop for AnchorScope {
    fn drop(&mut self) {
        reset_anchor_for_test();
    }
}

/// ① 锚定后 [`env_file_path`] 返回锚定值（`LIVE2D_AI_ENV_FILE` 显式覆盖除外）。
#[test]
fn env_file_path_returns_anchored_value() {
    let _scope = AnchorScope::new();
    let anchored = std::env::temp_dir().join(format!("l2d-anchored-env-{}", std::process::id()));
    anchor_env_file(anchored.clone()).expect("锚定 .env 路径");
    assert_eq!(anchored_env_file(), Some(anchored.clone()));
    match std::env::var_os("LIVE2D_AI_ENV_FILE").filter(|p| !p.is_empty()) {
        Some(p) => assert_eq!(
            env_file_path(),
            PathBuf::from(p),
            "LIVE2D_AI_ENV_FILE 是显式覆盖，优先级高于锚定"
        ),
        None => assert_eq!(
            env_file_path(),
            anchored,
            "锚定后必须读锚定文件，而不是 cwd 的 .env"
        ),
    }
}

/// ② 未锚定 = 旧行为：`<cwd>/.env`（环境变量显式指定时仍优先）。
#[test]
fn unanchored_env_file_path_keeps_cwd_join() {
    let _scope = AnchorScope::new();
    reset_anchor_for_test();
    assert_eq!(anchored_env_file(), None, "前置：全局必须未锚定");
    match std::env::var_os("LIVE2D_AI_ENV_FILE").filter(|p| !p.is_empty()) {
        Some(p) => assert_eq!(env_file_path(), PathBuf::from(p)),
        None => {
            let cwd = std::env::current_dir().expect("cwd");
            assert_eq!(env_file_path(), cwd.join(".env"), "未锚定保持旧口径");
        }
    }
}

/// 解析顺序：`LIVE2D_AI_ENV_FILE`（非空）> 锚定 > `<cwd>/.env`。
///
/// 用参数注入，完全不碰全局锚定（可并行）。
#[test]
fn resolve_order_is_override_then_anchor_then_cwd() {
    let cwd = PathBuf::from("/l2d/repo");
    let anchored = PathBuf::from("/l2d/home/.config/live2d-ai/.env");
    assert_eq!(
        resolve_env_file_path(None, None, Some(cwd.clone())),
        cwd.join(".env")
    );
    assert_eq!(
        resolve_env_file_path(Some(anchored.clone()), None, Some(cwd.clone())),
        anchored
    );
    assert_eq!(
        resolve_env_file_path(
            Some(PathBuf::from("/l2d/anchor/.env")),
            Some(OsString::from("/custom/keys.env")),
            Some(cwd.clone())
        ),
        PathBuf::from("/custom/keys.env"),
        "环境变量显式指定优先于锚定"
    );
    assert_eq!(
        resolve_env_file_path(None, Some(OsString::new()), Some(cwd.clone())),
        cwd.join(".env"),
        "空环境变量 = 未设置"
    );
    let fallback = resolve_env_file_path(None, None, None);
    assert!(!fallback.is_absolute(), "cwd 不可得 → 相对路径兜底");
    assert_eq!(fallback, PathBuf::from(".").join(".env"));
}

/// ③ 配置锚点：`LIVE2D_AI_ENV_FILE` 已设 → 不锚；否则配置父目录 `/.env`。
#[test]
fn env_anchor_follows_config_dir_unless_overridden() {
    let dir = std::env::temp_dir().join(format!("l2d-env-anchor-cfg-{}", std::process::id()));
    let config = dir.join("live2d-ai.toml");
    assert_eq!(
        env_anchor_for_config(&config, None),
        Some(dir.join(".env")),
        "无环境变量时锚到配置同目录"
    );
    assert_eq!(
        env_anchor_for_config(&config, Some(OsString::new())),
        Some(dir.join(".env")),
        "空环境变量 = 未设置"
    );
    assert_eq!(
        env_anchor_for_config(&config, Some(OsString::from("/custom/keys.env"))),
        None,
        "环境变量显式指定时不得改锚"
    );
    assert_eq!(
        env_anchor_for_config(&PathBuf::from("live2d-ai.toml"), None),
        None,
        "无父目录（裸文件名）不锚：保持旧口径"
    );
}

/// 锚到空路径必须拒绝——否则 [`env_file_path`] 会静默落回相对 `.env`。
#[test]
fn anchor_rejects_empty_path() {
    assert!(anchor_env_file(PathBuf::new()).is_err());
}
