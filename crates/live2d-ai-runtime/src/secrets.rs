//! `.env` = **唯一密钥真源**（rc.2 2026-09-12 用户口径）。
//!
//! # 契约
//!
//! - LLM / TTS 的 **API key 一律从 `.env` 读**；查找优先级写死为
//!   **`.env` 快照 > 进程环境 > 无（不鉴权）**。
//! - `.env` 由前端经 `PUT /api/v1/env` **写入**，写完**热重载**（不重启后端）。
//!   `live2d-ai.toml` 继续只持 `api_key_env`（环境变量**名**）——键名映射不变。
//!
//! # 为什么要有这个模块
//!
//! 以前密钥**只存在于后端进程环境**：`scripts/ignite.sh` 用
//! `set -a; . ./.env` 导入，Rust 侧从不读 `.env`。于是「界面上能改模型名、
//! 却改不了 key」，用户改完配置还是 401——查不到原因，因为配置里根本没有 key。
//!
//! # 为什么是**快照**而不是回灌进程环境
//!
//! `std::env::set_var` 在 Rust 2024 是 `unsafe`，且多线程下不可靠（别的线程可能
//! 正在读环境）。所以这里把 `.env` 解析成一张表存起来，查找时先查表——
//! 语义上等价于「`.env` 优先」，但没有任何全局可变环境。
//!
//! # 安全红线（沿用 AGENTS，不放松）
//!
//! 密钥不进 GET 响应 / 日志 / WS / 导出；只有 [`is_set`] 这类**布尔**可以出门。
//! `.env` 必须 `0600` 且留在 `.gitignore` 内（本模块写入时会强制 `0600`）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

/// `.env` 快照。`None` = 还没读过（首次查找时惰性加载）。
static SNAPSHOT: LazyLock<RwLock<Option<BTreeMap<String, String>>>> =
    LazyLock::new(|| RwLock::new(None));

/// `.env` 路径：`LIVE2D_AI_ENV_FILE` > `<cwd>/.env`。
///
/// 与 `live2d-ai.toml` 同处仓库根（`scripts/ignite.sh` 已把 cwd 固定在那里），
/// 也和 `.gitignore` 里那条 `/.env` 对得上。
pub fn env_file_path() -> PathBuf {
    if let Some(p) = std::env::var_os("LIVE2D_AI_ENV_FILE")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".env")
}

/// 解析 `.env` 文本 → 键值表（最小实现：`KEY=VALUE` + `#` 注释 + 可选引号）。
///
/// 刻意**只**支持这些（不引 `dotenvy`，按「宁删勿加」）：
///
/// - 空行 / 以 `#` 开头的行 → 跳过；
/// - 可选的 `export ` 前缀（`.env` 常见写法）→ 去掉；
/// - 第一个 `=` 切分（值里可以有 `=`）；
/// - 值两端空白去掉；成对的 `"` 或 `'` 去掉（**不**做转义展开）；
/// - 键名必须是合法环境变量名（见 [`is_valid_key`]），否则**整行跳过**
///   （宁可少一个变量，也不要让一行手写错的内容变成半截 key）。
///
/// 后出现的同名键覆盖先出现的（与 shell `source` 一致）。
pub fn parse(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !is_valid_key(key) {
            continue;
        }
        let value = value.trim();
        let value = unquote(value);
        out.insert(key.to_string(), value.to_string());
    }
    out
}

/// 去掉成对的引号（单层，不做转义展开）。
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[bytes.len() - 1] == bytes[0]
    {
        return &value[1..value.len() - 1];
    }
    value
}

/// 是否是可写入的环境变量名（与 `settings::validate_env_name` 同口径）。
pub fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 读取 `.env` 并替换快照，返回键数量。
///
/// 文件不存在 = 空快照（**不是错误**：本地首次启动就没有 `.env`）。
/// 读失败（权限等）→ `Err`，快照保持不变（不把好数据换成空）。
pub fn refresh_from_disk() -> Result<usize, String> {
    refresh_from(env_file_path())
}

fn refresh_from(path: impl AsRef<Path>) -> Result<usize, String> {
    let path = path.as_ref();
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("读取 {} 失败: {e}", path.display())),
    };
    let map = parse(&text);
    let n = map.len();
    if let Ok(mut g) = SNAPSHOT.write() {
        *g = Some(map);
    }
    Ok(n)
}

/// 当前快照（首次调用会惰性加载一次）。
fn snapshot() -> BTreeMap<String, String> {
    if let Ok(g) = SNAPSHOT.read()
        && let Some(map) = g.as_ref()
    {
        return map.clone();
    }
    let _ = refresh_from_disk();
    SNAPSHOT
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default()
}

/// 查找密钥：**`.env` 快照 > 进程环境 > `None`**（空串一律视为未设置）。
///
/// 这是全项目**唯一**该被用来读密钥的函数——`std::env::var` 直接读会绕过
/// `.env`，于是「界面上写了 key 却还是 401」。
pub fn lookup(name: &str) -> Option<String> {
    if let Some(v) = snapshot().get(name)
        && !v.is_empty()
    {
        return Some(v.clone());
    }
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// 该键是否已设置（**布尔可以出门，值不可以**）。
pub fn is_set(name: &str) -> bool {
    lookup(name).is_some()
}

/// `.env` 里已有的键名（按字典序；**不含值**）。
pub fn known_keys() -> Vec<String> {
    snapshot().keys().cloned().collect()
}

/// 把 `KEY=VALUE` 写进 `.env`（就地改那一行，保留其它行与注释），并刷新快照。
///
/// # 为什么必须「就地改」而不是重新生成
///
/// 与 `live2d-ai.toml` 同一条教训（`merge_into_toml`）：`.env` 是**手写的**，
/// 里面有注释、空行、别人写的变量。整份重写会把这些抹掉——用户不会立刻发现，
/// 等发现时已经找不到原来的注释了。
///
/// 写盘走 [`crate::settings::patch::plan_atomic_write`]（tmp → fdatasync → rename），
/// 再把权限收紧到 `0600`。`value` 为空 = **删除该键**（与「设为空」同义：
/// `lookup` 本来就把空串当未设置，留着空行只会误导）。
pub fn write_key(key: &str, value: &str) -> Result<(), String> {
    write_key_to(env_file_path(), key, value)
}

fn write_key_to(path: impl AsRef<Path>, key: &str, value: &str) -> Result<(), String> {
    let path = path.as_ref();
    if !is_valid_key(key) {
        return Err(format!(
            "非法环境变量名 {key:?}（仅允许 ASCII 字母/数字/下划线，且不以数字开头）"
        ));
    }
    if value.contains('\n') || value.contains('\r') {
        return Err("密钥值不允许换行".to_string());
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let next = merge_env_line(&existing, key, value);
    if next.trim().is_empty() {
        return Err("拒绝写空的 .env".to_string());
    }
    // **回读校验**：写进去的东西必须能被 `parse` 读回同一个值。
    // 与其在手写解析器里穷举引号规则，不如让「写坏了」在落盘前就失败——
    // 密钥文件写坏是不可接受的。
    let readback = parse(&next).get(key).cloned().unwrap_or_default();
    if value.is_empty() {
        if parse(&next).contains_key(key) {
            return Err(format!("无法安全删除 {key}：回读仍在"));
        }
    } else if readback != value {
        return Err(format!(
            "无法安全写入 {key}：写入后回读不一致（值里含引号/特殊字符？）"
        ));
    }
    let tmp = crate::settings::patch::plan_atomic_write(path, &next)?;
    // 先收紧 **tmp** 的权限再 rename：否则会有一段「文件已是新密钥、权限还是
    // umask 默认（通常 0644）」的窗口。rename 之后收紧就晚了。
    restrict_to_owner(&tmp)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("rename {} 失败: {e}", tmp.display()));
    }
    refresh_from(path)?;
    Ok(())
}

/// 把 `KEY=VALUE` 合并进 `.env` 文本：命中已有行 → 就地替换，未命中 → 追加。
///
/// `value` 为空 = 删掉那一行（连同它上面紧邻的注释块？**不**——注释可能解释的是
/// 下一段内容，删掉会误伤；只删键行本身）。
pub fn merge_env_line(text: &str, key: &str, value: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let candidates = body.split_once('=').map(|(k, _)| k.trim());
        if candidates == Some(key) {
            if value.is_empty() {
                replaced = true;
                continue; // 删除该键
            }
            // 保留原行的缩进与 `export ` 前缀（尽量少动别人的手写格式）。
            let indent = &line[..line.len() - trimmed.len()];
            let export = if trimmed.starts_with("export ") {
                "export "
            } else {
                ""
            };
            out.push(format!("{indent}{export}{key}={}", quote_if_needed(value)));
            replaced = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !replaced && !value.is_empty() {
        out.push(format!("{key}={}", quote_if_needed(value)));
    }
    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

/// 值里含空白或 `#` 时加引号（否则会被 `parse` 截断或误当注释）。
///
/// 值里已有 `"` 时改用单引号：`parse` 不做转义展开，双引号里再放双引号会读不回来。
/// 两种引号都有的病态取值由 [`write_key_to`] 的**回读校验**兜住（宁拒绝，不写坏）。
fn quote_if_needed(value: &str) -> String {
    if value.contains(char::is_whitespace) || value.contains('#') {
        if value.contains('"') {
            format!("'{value}'")
        } else {
            format!("\"{value}\"")
        }
    } else {
        value.to_string()
    }
}

/// 权限收紧到 `0600`（Unix；其它平台是 no-op）。
#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("设置 {} 权限为 0600 失败: {e}", path.display()))
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_handles_comments_quotes_and_export() {
        let text = "\
# 注释
DEEPSEEK_API_KEY=sk-abc
export ZHIPU_API_KEY='zk 1#2'
  SPACED = \"quoted value\"  
EMPTY=
ALSO_EMPTY=''
WITH_EQUALS=a=b=c
not a var line
1BAD=x
";
        let m = parse(text);
        assert_eq!(
            m.get("DEEPSEEK_API_KEY").map(String::as_str),
            Some("sk-abc")
        );
        assert_eq!(m.get("ZHIPU_API_KEY").map(String::as_str), Some("zk 1#2"));
        assert_eq!(m.get("SPACED").map(String::as_str), Some("quoted value"));
        assert_eq!(m.get("EMPTY").map(String::as_str), Some(""));
        assert_eq!(m.get("ALSO_EMPTY").map(String::as_str), Some(""));
        assert_eq!(m.get("WITH_EQUALS").map(String::as_str), Some("a=b=c"));
        assert!(!m.contains_key("1BAD"), "非法键名必须整行跳过");
    }

    #[test]
    fn parse_last_wins() {
        let m = parse("A=1\nA=2\n");
        assert_eq!(m.get("A").map(String::as_str), Some("2"));
    }

    #[test]
    fn valid_key_shape() {
        assert!(is_valid_key("DEEPSEEK_API_KEY"));
        assert!(is_valid_key("_x1"));
        assert!(!is_valid_key(""));
        assert!(!is_valid_key("1A"));
        assert!(!is_valid_key("A-B"));
        assert!(!is_valid_key("A B"));
    }

    #[test]
    fn merge_replaces_in_place_and_keeps_comments() {
        let before = "# 说明\nDEEPSEEK_API_KEY=old\nOTHER=keep\n";
        let after = merge_env_line(before, "DEEPSEEK_API_KEY", "new");
        assert!(after.contains("# 说明"), "注释必须保留：{after}");
        assert!(after.contains("OTHER=keep"));
        assert!(after.contains("DEEPSEEK_API_KEY=new"));
        assert!(!after.contains("old"));
    }

    #[test]
    fn merge_preserves_export_prefix_and_indent() {
        let after = merge_env_line("  export A=1\n", "A", "2");
        assert_eq!(after, "  export A=2\n");
    }

    #[test]
    fn merge_appends_when_missing() {
        let after = merge_env_line("A=1\n", "B", "2");
        assert_eq!(after, "A=1\nB=2\n");
    }

    #[test]
    fn merge_with_empty_value_deletes_the_key_only() {
        let after = merge_env_line("# why\nA=1\nB=2\n", "A", "");
        assert_eq!(after, "# why\nB=2\n");
    }

    #[test]
    fn merge_quotes_values_with_spaces_or_hash() {
        assert_eq!(merge_env_line("", "A", "has space"), "A=\"has space\"\n");
        assert_eq!(merge_env_line("", "A", "has#hash"), "A=\"has#hash\"\n");
        assert_eq!(merge_env_line("", "A", "plain"), "A=plain\n");
        // 引号值必须能被 parse 读回同一个值。
        let round = parse(&merge_env_line("", "A", "has space"));
        assert_eq!(round.get("A").map(String::as_str), Some("has space"));
    }

    /// 写盘：值读得回来、权限是 `0600`、其它行与注释不动。
    #[test]
    #[cfg(unix)]
    fn write_key_to_disk_is_atomic_and_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("l2d_env_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(".env");
        std::fs::write(&path, "# keep me\nA=1\n").unwrap();

        write_key_to(&path, "DEEPSEEK_API_KEY", "sk-test").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep me"), "手写注释必须保留：{text}");
        assert!(text.contains("DEEPSEEK_API_KEY=sk-test"));
        assert!(text.contains("A=1"));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "`.env` 必须是 0600，实际 {mode:o}");
        // 没有留下 tmp 文件。
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "原子写回不该留 tmp：{leftovers:?}");

        // 删除：写空值 → 行消失。
        write_key_to(&path, "DEEPSEEK_API_KEY", "").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("DEEPSEEK_API_KEY"));
        assert!(text.contains("A=1") && text.contains("# keep me"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_key_rejects_bad_names_and_multiline_values() {
        let path = std::env::temp_dir().join("l2d_env_should_not_exist");
        let _ = std::fs::remove_file(&path);
        assert!(write_key_to(&path, "A-B", "x").is_err());
        assert!(write_key_to(&path, "A", "line1\nline2").is_err());
        assert!(!path.exists(), "校验失败不得留下文件");
    }
}
