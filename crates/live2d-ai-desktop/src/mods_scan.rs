//! 工作目录 `mods/<目录名>/mod.toml` 的**只读清单预检**（0.3.12）。
//!
//! 执行单：`docs/plans/PLAN-0.3.12-mod-folder-scan-2026-10-20.md`。
//!
//! # 本版做什么 / 不做什么
//!
//! 只做**清单预检**：扫描根是**进程当前目录**下的 `mods/`，只认下一层的
//! `mods/<目录名>/mod.toml`，把「已发现 / 不可用 + 原因」交给启动器页
//! （`GET /api/v1/mods/discovered`）。**不执行** `entry` / `launch.program`，
//! **不 spawn** 任何进程，**不把文件夹写进** `AVAILABLE_MOD_FACTORIES`
//! （缺省工厂仍是 0），也不读 `[permissions]`（出现未知键 = 这份清单不可用）。
//!
//! 三个刻意的边界：
//!
//! - **不跟随符号链接**：目录本身或 `mod.toml` 是符号链接 → 该行不可用、原因写明；
//! - **只做词法 + 一次 metadata/规范化**：`entry` / `launch.program` 必须是该
//!   Mod 目录内的相对路径、且是普通文件——**不要求可执行位**，也**不打开它当程序跑**；
//! - **失败不阻塞**：没有 `mods/` 目录 = 已发现 0（**不创建**它）；扫描失败 =
//!   空列表 + 一行 warn。进程照常启动，照常能进 `/app/`。
//!
//! 字段表与「保留 id」名单见执行单 §3；解析器见 `mods_scan_toml`（严格 TOML
//! 子集，失败关闭）。

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use live2d_ai_mod_system::MOD_API_VERSION;

use crate::mods_scan_toml as toml_lite;
use crate::mods_scan_toml::Document;

/// 清单文件名（只看这一层）。
const MANIFEST_FILE: &str = "mod.toml";

/// 扫描根目录名（进程当前目录下）。
const ROOT_DIR: &str = "mods";

/// 保留 id：清单写得再对也不可选（执行单 §3）。
///
/// `director` 在核心里；`local-llm` / `wallpaper` / `pet-desktop` 已删除；
/// CosyVoice 的 id 是 `local-tts`（封存）。原因统一写「保留，不能当文件夹装入」。
const RESERVED_IDS: &[&str] = &[
    "director",
    "local-llm",
    "wallpaper",
    "pet-desktop",
    "local-tts",
];

/// 顶层允许的键（其余一律「未知键」→ 不可用）。
const TOP_KEYS: &[&str] = &[
    "schema",
    "id",
    "name",
    "version",
    "api_version",
    "kind",
    "description",
    "entry",
    "launch",
];

/// `[launch]` 表允许的键。
const LAUNCH_KEYS: &[&str] = &["program", "mode"];

/// 一行预检结果：启动器页「已发现 / 不可用」列表的元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredMod {
    /// 目录名（无效行也填它：用户在磁盘上看到的就是这个名字）。
    pub id: String,
    /// 清单里的 `name`（无效行为空串）。
    pub name: String,
    /// 清单里的 `version`（无效行为空串）。
    pub version: String,
    /// 清单里的 `api_version`（无效行为 0）。
    pub api_version: u32,
    /// `process` / `launcher`（无效行为空串）。
    pub kind: String,
    /// 是否可选：校验通过、不保留、不冲突、不重复。
    pub selectable: bool,
    /// 不可用的短原因（可选行为 `None`）。
    pub reason: Option<String>,
}

impl DiscoveredMod {
    /// 不可用行：只带目录名与原因（其余字段留空）。
    fn invalid(id: &str, reason: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            name: String::new(),
            version: String::new(),
            api_version: 0,
            kind: String::new(),
            selectable: false,
            reason: Some(reason.into()),
        }
    }
}

/// 扫描根 = 当前目录下的 `mods/`（执行单 §3：**进程当前目录**，不猜仓库根）。
pub fn scan_root() -> PathBuf {
    match std::env::current_dir() {
        Ok(dir) => dir.join(ROOT_DIR),
        Err(_) => PathBuf::from(ROOT_DIR),
    }
}

/// 扫描 `root`（只读、不创建目录）。`registered` = 已编进二进制的工厂 id。
///
/// 排序按**路径**（执行单 §3：重复 id 时路径排序后的第一份才可能可选）。
/// 目录不存在 / 不可读 → 空列表（只记一行 warn，不打断任何东西）。
pub fn scan(root: &Path, registered: &[&str]) -> Vec<DiscoveredMod> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => {
            tracing::warn!(target: "mod", path = %root.display(), "mods 扫描失败：{err}");
            return Vec::new();
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();

    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    let mut out = Vec::new();
    for dir in paths {
        let dir_name = dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        // 以 `.` 开头的目录跳过（执行单 §3）。
        if dir_name.starts_with('.') {
            continue;
        }
        let is_symlink = std::fs::symlink_metadata(&dir)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(true);
        let manifest = dir.join(MANIFEST_FILE);
        // 没有 mod.toml 的目录不是 Mod 文件夹：不列（也不当成「无效清单」）。
        if !dir_holds_manifest(&dir, &manifest, is_symlink) {
            continue;
        }
        if is_symlink {
            out.push(DiscoveredMod::invalid(&dir_name, "目录是符号链接，不跟随"));
            continue;
        }
        match std::fs::symlink_metadata(&manifest) {
            Ok(meta) if meta.file_type().is_file() => {}
            Ok(meta) if meta.file_type().is_symlink() => {
                out.push(DiscoveredMod::invalid(&dir_name, "清单是符号链接，不跟随"));
                continue;
            }
            Ok(_) => {
                out.push(DiscoveredMod::invalid(&dir_name, "清单不是普通文件"));
                continue;
            }
            Err(_) => continue, // 扫到一半被删：当作不存在。
        }
        let text = match std::fs::read_to_string(&manifest) {
            Ok(text) => text,
            Err(err) => {
                out.push(DiscoveredMod::invalid(
                    &dir_name,
                    format!("读清单失败：{err}"),
                ));
                continue;
            }
        };
        let doc = match toml_lite::parse(&text) {
            Ok(doc) => doc,
            Err(reason) => {
                out.push(DiscoveredMod::invalid(
                    &dir_name,
                    format!("清单解析失败：{reason}"),
                ));
                continue;
            }
        };
        // 去重键 = 清单声明的 id（没有就用目录名）。第一份（路径序）留下，
        // 其余一律「重复」——即使第一份本身因别的字段不可用（执行单 §3）。
        let dedupe_key = doc
            .top
            .get("id")
            .and_then(toml_lite::Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| dir_name.clone());
        if seen.contains_key(&dedupe_key) {
            out.push(DiscoveredMod::invalid(&dir_name, "重复"));
            continue;
        }
        seen.insert(dedupe_key, ());

        match validate(&dir, &dir_name, &doc, registered) {
            Ok(info) => out.push(DiscoveredMod {
                id: info.id,
                name: info.name,
                version: info.version,
                api_version: info.api_version,
                kind: info.kind,
                selectable: true,
                reason: None,
            }),
            Err(reason) => out.push(DiscoveredMod::invalid(&dir_name, reason)),
        }
    }
    out
}

/// 目录下是否「看起来有清单」：非符号链接目录直接看 `mod.toml` 是否存在；
/// 符号链接目录只做一次 metadata 判定（不读清单内容），以便把「目录是
/// 符号链接」作为**不可用行**列出来而不是静默丢掉。
fn dir_holds_manifest(dir: &Path, manifest: &Path, dir_is_symlink: bool) -> bool {
    if dir_is_symlink {
        std::fs::metadata(dir)
            .map(|meta| meta.is_dir())
            .unwrap_or(false)
            && manifest.exists()
    } else {
        std::fs::symlink_metadata(manifest).is_ok()
    }
}

/// 校验通过后的字段（不可用行不保留这些）。
struct ModInfo {
    id: String,
    name: String,
    version: String,
    api_version: u32,
    kind: String,
}

/// 字段校验（执行单 §3 的字段表 + §2 的禁止项）。`Err` = 一句短原因。
fn validate(
    dir: &Path,
    dir_name: &str,
    doc: &Document,
    registered: &[&str],
) -> Result<ModInfo, String> {
    for key in doc.top.keys() {
        if !TOP_KEYS.contains(&key.as_str()) {
            return Err(format!("未知键 `{key}`"));
        }
    }
    if let Some(launch) = &doc.launch {
        for key in launch.keys() {
            if !LAUNCH_KEYS.contains(&key.as_str()) {
                return Err(format!("未知键 `launch.{key}`"));
            }
        }
    }

    match doc.top.get("schema") {
        Some(toml_lite::Value::Int(1)) => {}
        Some(_) => return Err("`schema` 必须是整数 1".to_string()),
        None => return Err("缺少 `schema`".to_string()),
    }

    let id = match doc.top.get("id").and_then(toml_lite::Value::as_str) {
        Some(id) => id,
        None => return Err("缺少字符串 `id`".to_string()),
    };
    if !valid_id(id) {
        return Err("`id` 不符合 `^[a-z][a-z0-9-]{0,63}$`".to_string());
    }
    if id != dir_name {
        return Err("`id` 必须等于目录名".to_string());
    }
    let name = non_empty("name", doc)?;
    let version = non_empty("version", doc)?;

    let api_version = match doc.top.get("api_version") {
        Some(toml_lite::Value::Int(value)) => *value,
        Some(_) => return Err("`api_version` 必须是整数".to_string()),
        None => return Err("缺少 `api_version`".to_string()),
    };
    if api_version != i64::from(MOD_API_VERSION) {
        return Err(format!("`api_version` 必须等于 {MOD_API_VERSION}"));
    }
    if let Some(value) = doc.top.get("description")
        && value.as_str().is_none()
    {
        return Err("`description` 必须是字符串".to_string());
    }

    let kind = match doc.top.get("kind").and_then(toml_lite::Value::as_str) {
        Some(kind @ ("process" | "launcher")) => kind.to_string(),
        Some(_) => return Err("`kind` 只能是 process 或 launcher".to_string()),
        None => return Err("缺少 `kind`".to_string()),
    };
    if kind == "process" {
        let entry = doc
            .top
            .get("entry")
            .and_then(toml_lite::Value::as_str)
            .ok_or_else(|| "`kind = process` 时必须有字符串 `entry`".to_string())?;
        check_relative_file(dir, entry, "entry")?;
    } else if let Some(entry) = doc.top.get("entry") {
        let entry = entry
            .as_str()
            .ok_or_else(|| "`entry` 必须是字符串".to_string())?;
        check_relative_file(dir, entry, "entry")?;
    }

    if kind == "launcher" {
        let launch = doc
            .launch
            .as_ref()
            .ok_or_else(|| "`kind = launcher` 时必须有 `[launch]` 表".to_string())?;
        let program = launch
            .get("program")
            .and_then(toml_lite::Value::as_str)
            .ok_or_else(|| "`launch.program` 必填".to_string())?;
        check_relative_file(dir, program, "launch.program")?;
        let mode = launch
            .get("mode")
            .and_then(toml_lite::Value::as_str)
            .ok_or_else(|| "`launch.mode` 必填".to_string())?;
        check_mode(mode)?;
    } else if let Some(launch) = &doc.launch {
        // process 清单若也写了 [launch]：按同一套词法校验（program 可省）。
        if let Some(program) = launch.get("program") {
            let program = program
                .as_str()
                .ok_or_else(|| "`launch.program` 必须是字符串".to_string())?;
            check_relative_file(dir, program, "launch.program")?;
        }
        if let Some(mode) = launch.get("mode") {
            let mode = mode
                .as_str()
                .ok_or_else(|| "`launch.mode` 必须是字符串".to_string())?;
            check_mode(mode)?;
        }
    }

    if RESERVED_IDS.contains(&id) {
        return Err("保留，不能当文件夹装入".to_string());
    }
    if registered.contains(&id) {
        return Err("与已编进二进制的 Mod 冲突".to_string());
    }

    Ok(ModInfo {
        id: id.to_string(),
        name: name.to_string(),
        version: version.to_string(),
        api_version: u32::try_from(api_version).unwrap_or(0),
        kind,
    })
}

/// `^[a-z][a-z0-9-]{0,63}$`（整串为 ASCII）。
fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    bytes.len() <= 64
        && first.is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// 取非空字符串键（`name` / `version`）。
fn non_empty<'a>(key: &str, doc: &'a Document) -> Result<&'a str, String> {
    match doc.top.get(key) {
        Some(toml_lite::Value::Str(value)) if !value.trim().is_empty() => Ok(value),
        Some(toml_lite::Value::Str(_)) => Err(format!("`{key}` 不能为空")),
        Some(_) => Err(format!("`{key}` 必须是字符串")),
        None => Err(format!("缺少 `{key}`")),
    }
}

/// `launch.mode` 只允许两值。
fn check_mode(mode: &str) -> Result<(), String> {
    if mode == "with_app" || mode == "on_demand" {
        Ok(())
    } else {
        Err("`launch.mode` 只能是 with_app 或 on_demand".to_string())
    }
}

/// `entry` / `launch.program` 的检查：相对路径、无 `..`、**不跟随符号链接**、
/// 规范化后仍在本 Mod 目录内、且是普通文件。**不要求可执行位**，也不执行它。
fn check_relative_file(dir: &Path, rel: &str, label: &str) -> Result<(), String> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Err(format!("`{label}` 不能为空"));
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(format!("`{label}` 必须是相对路径"));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(format!(
                    "`{label}` 必须是 Mod 目录内的相对路径（不允许 `..`）"
                ));
            }
        }
    }
    let target = dir.join(path);
    let meta =
        std::fs::symlink_metadata(&target).map_err(|_| format!("`{label}` 指向的文件不存在"))?;
    if !meta.file_type().is_file() {
        return Err(format!("`{label}` 必须是普通文件"));
    }
    let Ok(canon_dir) = std::fs::canonicalize(dir) else {
        return Err(format!("`{label}` 所在目录不可规范化"));
    };
    let Ok(canon) = std::fs::canonicalize(&target) else {
        return Err(format!("`{label}` 指向的文件不存在"));
    };
    if !canon.starts_with(&canon_dir) {
        return Err(format!("`{label}` 规范化后不在 Mod 目录内"));
    }
    Ok(())
}

/// `GET /api/v1/mods/discovered` 的响应体。
///
/// 每行带 `enabled`（该 id 在 `mods.json` 里是否 `enabled = true`）：
/// 启动器页的**初始勾选**按执行单 §4 用「可选 + enabled」决定，不额外读盘。
pub fn discovered_json(
    entries: &[DiscoveredMod],
    manifest: &serde_json::Value,
) -> serde_json::Value {
    let mods = manifest.get("mods");
    let rows: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            let enabled = mods
                .and_then(|mods| mods.get(&entry.id))
                .and_then(|slot| slot.get("enabled"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            serde_json::json!({
                "id": entry.id,
                "name": entry.name,
                "version": entry.version,
                "api_version": entry.api_version,
                "kind": entry.kind,
                "selectable": entry.selectable,
                "reason": entry.reason,
                "enabled": enabled,
            })
        })
        .collect();
    serde_json::json!({ "discovered": rows })
}

#[cfg(test)]
#[path = "mods_scan_tests.rs"]
mod tests;
