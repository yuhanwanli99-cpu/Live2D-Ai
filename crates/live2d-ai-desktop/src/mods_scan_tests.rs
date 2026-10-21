//! `mods_scan` 的回归：扫描 / 校验 / 选择写回（0.3.12 执行单 `3 / `6）。
//!
//! 全部用例只用**临时目录**与**临时 `mods.json`**（执行单 `2）：不碰仓库根
//! 那三份本机配置，也不往仓库里放任何 `mods/<id>/` 样例。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 临时目录树（Drop 时删干净）。
struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(tag: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "l2d-mods-scan-{}-{}-{tag}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("临时目录");
        Self { root }
    }

    /// 写一个文件（自动建父目录）。
    fn write(&self, rel: &str, body: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("父目录");
        }
        std::fs::write(path, body).expect("写临时文件");
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 合法的 process 清单（entry 指向同目录的 run.sh）。
fn process_manifest(id: &str) -> String {
    format!(
        "schema = 1\nid = \"{id}\"\nname = \"{id} 示例\"\nversion = \"0.1.0\"\n\
         api_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n"
    )
}

/// 合法的 launcher 清单。
fn launcher_manifest(id: &str) -> String {
    format!(
        "schema = 1\nid = \"{id}\"\nname = \"{id} 示例\"\nversion = \"0.1.0\"\n\
         api_version = 1\nkind = \"launcher\"\n[launch]\nprogram = \"bin/start\"\n\
         mode = \"with_app\"\n"
    )
}

fn only(rows: &[DiscoveredMod]) -> &DiscoveredMod {
    assert_eq!(rows.len(), 1, "只应有一行：{rows:?}");
    &rows[0]
}

fn row<'a>(rows: &'a [DiscoveredMod], id: &str) -> &'a DiscoveredMod {
    rows.iter()
        .find(|row| row.id == id)
        .unwrap_or_else(|| panic!("缺少行 {id}：{rows:?}"))
}

fn reason_contains(row: &DiscoveredMod, needle: &str) {
    let reason = row.reason.as_deref().unwrap_or("");
    assert!(!row.selectable, "{} 不应可选：{row:?}", row.id);
    assert!(
        reason.contains(needle),
        "{} 的原因应包含 {needle:?}，实际 {reason:?}",
        row.id
    );
}

#[test]
fn missing_mods_dir_is_zero_and_not_created() {
    let tree = Tree::new("nods");
    let root = tree.join("mods");
    let rows = scan(&root, &[]);
    assert!(rows.is_empty(), "没有 mods/ 目录 = 已发现 0");
    assert!(!root.exists(), "扫描不得创建 mods/ 目录");
}

#[test]
fn valid_process_folder_is_selectable_and_entry_need_not_be_executable() {
    let tree = Tree::new("process");
    tree.write("mods/demo-mod/mod.toml", &process_manifest("demo-mod"));
    // 没有可执行位；内容是「被跑就留证」的脚本。
    tree.write("mods/demo-mod/run.sh", "#!/bin/sh\ntouch ../ran\n");
    let root = tree.join("mods");
    let rows = scan(&root, &[]);
    let row = only(&rows);
    assert_eq!(row.id, "demo-mod");
    assert_eq!(row.name, "demo-mod 示例");
    assert_eq!(row.version, "0.1.0");
    assert_eq!(row.api_version, 1);
    assert_eq!(row.kind, "process");
    assert!(row.selectable, "合法 process 清单必须可选：{row:?}");
    assert_eq!(row.reason, None);
    assert!(
        !tree.join("ran").exists(),
        "扫描绝不执行 entry（entry 没有被跑）"
    );
}

#[test]
fn valid_launcher_folder_is_selectable_and_program_never_runs() {
    let tree = Tree::new("launcher");
    tree.write("mods/win-mod/mod.toml", &launcher_manifest("win-mod"));
    tree.write("mods/win-mod/bin/start", "#!/bin/sh\ntouch ../../ran\n");
    let rows = scan(&tree.join("mods"), &[]);
    let row = only(&rows);
    assert_eq!(row.kind, "launcher");
    assert!(row.selectable, "合法 launcher 清单必须可选：{row:?}");
    assert!(!tree.join("ran").exists(), "扫描绝不执行 launch.program");
}

#[test]
fn rows_are_sorted_by_path() {
    let tree = Tree::new("order");
    tree.write("mods/b-mod/mod.toml", &process_manifest("b-mod"));
    tree.write("mods/b-mod/run.sh", "");
    tree.write("mods/a-mod/mod.toml", &process_manifest("a-mod"));
    tree.write("mods/a-mod/run.sh", "");
    let rows = scan(&tree.join("mods"), &[]);
    let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, vec!["a-mod", "b-mod"], "按路径排序");
}

#[test]
fn invalid_folders_are_listed_with_reasons() {
    let tree = Tree::new("invalid");
    let mods = "mods";

    // 含 ..
    tree.write(
        &format!("{mods}/dotdot/mod.toml"),
        "schema = 1\nid = \"dotdot\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"../run.sh\"\n",
    );
    // 绝对路径
    tree.write(
        &format!("{mods}/abspath/mod.toml"),
        "schema = 1\nid = \"abspath\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"/bin/sh\"\n",
    );
    // 文件不存在
    tree.write(
        &format!("{mods}/missing-entry/mod.toml"),
        "schema = 1\nid = \"missing-entry\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"nope.sh\"\n",
    );
    // 不是普通文件
    tree.write(
        &format!("{mods}/entry-dir/mod.toml"),
        "schema = 1\nid = \"entry-dir\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"sub\"\n",
    );
    std::fs::create_dir_all(tree.join(&format!("{mods}/entry-dir/sub"))).expect("目录");

    tree.write(
        &format!("{mods}/api99/mod.toml"),
        "schema = 1\nid = \"api99\"\nname = \"x\"\nversion = \"1\"\napi_version = 99\nkind = \"process\"\nentry = \"run.sh\"\n",
    );
    tree.write(&format!("{mods}/api99/run.sh"), "");
    tree.write(
        &format!("{mods}/schema2/mod.toml"),
        "schema = 2\nid = \"schema2\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n",
    );
    tree.write(&format!("{mods}/schema2/run.sh"), "");
    tree.write(
        &format!("{mods}/string-schema/mod.toml"),
        "schema = \"1\"\nid = \"string-schema\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n",
    );
    tree.write(&format!("{mods}/string-schema/run.sh"), "");
    tree.write(
        &format!("{mods}/unknown-key/mod.toml"),
        "schema = 1\nid = \"unknown-key\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"run.sh\"\nfoo = 1\n",
    );
    tree.write(&format!("{mods}/unknown-key/run.sh"), "");
    tree.write(
        &format!("{mods}/permissions/mod.toml"),
        "schema = 1\nid = \"permissions\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n[permissions]\nwrite = true\n",
    );
    tree.write(&format!("{mods}/permissions/run.sh"), "");

    tree.write("mods/director/mod.toml", &process_manifest("director"));
    tree.write("mods/director/run.sh", "");
    tree.write(
        &format!("{mods}/mismatch/mod.toml"),
        &process_manifest("other-mod"),
    );
    tree.write(&format!("{mods}/mismatch/run.sh"), "");
    tree.write(
        &format!("{mods}/bad-kind/mod.toml"),
        "schema = 1\nid = \"bad-kind\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"daemon\"\n",
    );
    tree.write(
        &format!("{mods}/bad-mode/mod.toml"),
        "schema = 1\nid = \"bad-mode\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"launcher\"\n[launch]\nprogram = \"start\"\nmode = \"always\"\n",
    );
    tree.write(&format!("{mods}/bad-mode/start"), "");
    tree.write(
        &format!("{mods}/no-launch/mod.toml"),
        "schema = 1\nid = \"no-launch\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"launcher\"\n",
    );
    tree.write(
        &format!("{mods}/bad-id/mod.toml"),
        "schema = 1\nid = \"Bad_ID\"\nname = \"x\"\nversion = \"1\"\napi_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n",
    );
    tree.write(&format!("{mods}/bad-id/run.sh"), "");

    let rows = scan(&tree.join(mods), &[]);
    reason_contains(row(&rows, "dotdot"), "不允许");
    reason_contains(row(&rows, "abspath"), "相对路径");
    reason_contains(row(&rows, "missing-entry"), "不存在");
    reason_contains(row(&rows, "entry-dir"), "普通文件");
    reason_contains(row(&rows, "api99"), "api_version");
    reason_contains(row(&rows, "schema2"), "schema");
    reason_contains(row(&rows, "string-schema"), "schema");
    reason_contains(row(&rows, "unknown-key"), "未知键");
    reason_contains(row(&rows, "permissions"), "permissions");
    reason_contains(row(&rows, "director"), "保留");
    reason_contains(row(&rows, "mismatch"), "目录名");
    reason_contains(row(&rows, "bad-kind"), "kind");
    reason_contains(row(&rows, "bad-mode"), "launch.mode");
    reason_contains(row(&rows, "no-launch"), "launch");
    reason_contains(row(&rows, "bad-id"), "不符合");
    assert!(rows.iter().all(|row| !row.selectable), "全部不可选");
}

#[test]
fn hidden_dirs_and_dirs_without_manifest_are_skipped() {
    let tree = Tree::new("skip");
    tree.write("mods/.hidden/mod.toml", &process_manifest("hidden"));
    tree.write("mods/.hidden/run.sh", "");
    std::fs::create_dir_all(tree.join("mods/plain")).expect("目录");
    let rows = scan(&tree.join("mods"), &[]);
    assert!(
        rows.is_empty(),
        "点开头 / 没有 mod.toml 的目录不列：{rows:?}"
    );
}

#[test]
fn duplicate_id_keeps_only_the_first_path_and_marks_the_rest() {
    let tree = Tree::new("dup");
    // 两份清单都声明同一个 id（都因此「id 不等于目录名」不可用）；
    // 路径序里的第二份原因必须是「重复」。
    tree.write("mods/dup-a/mod.toml", &process_manifest("same-id"));
    tree.write("mods/dup-a/run.sh", "");
    tree.write("mods/dup-b/mod.toml", &process_manifest("same-id"));
    tree.write("mods/dup-b/run.sh", "");
    let rows = scan(&tree.join("mods"), &[]);
    reason_contains(row(&rows, "dup-a"), "目录名");
    reason_contains(row(&rows, "dup-b"), "重复");
}

#[test]
fn registered_factory_id_conflicts_with_a_folder() {
    let tree = Tree::new("conflict");
    tree.write(
        "mods/external-input/mod.toml",
        &process_manifest("external-input"),
    );
    tree.write("mods/external-input/run.sh", "");
    let rows = scan(&tree.join("mods"), &["external-input"]);
    reason_contains(only(&rows), "冲突");
}

#[cfg(unix)]
#[test]
fn symlinked_folder_or_manifest_is_unusable() {
    use std::os::unix::fs::symlink;

    let tree = Tree::new("symlink");
    tree.write("external/real/mod.toml", &process_manifest("linked"));
    tree.write("external/real/run.sh", "");
    std::fs::create_dir_all(tree.join("mods")).expect("目录");
    symlink(tree.join("external/real"), tree.join("mods/linked")).expect("目录软链");

    tree.write("external/file.toml", &process_manifest("linkfile"));
    std::fs::create_dir_all(tree.join("mods/linkfile")).expect("目录");
    symlink(
        tree.join("external/file.toml"),
        tree.join("mods/linkfile/mod.toml"),
    )
    .expect("文件软链");

    let rows = scan(&tree.join("mods"), &[]);
    reason_contains(row(&rows, "linked"), "符号链接");
    reason_contains(row(&rows, "linkfile"), "符号链接");
}

#[test]
fn discovered_json_carries_selectable_reason_and_enabled() {
    let tree = Tree::new("json");
    tree.write("mods/demo-mod/mod.toml", &process_manifest("demo-mod"));
    tree.write("mods/demo-mod/run.sh", "");
    tree.write("mods/director/mod.toml", &process_manifest("director"));
    tree.write("mods/director/run.sh", "");
    let rows = scan(&tree.join("mods"), &[]);
    let manifest = serde_json::json!({
        "mods": { "demo-mod": { "enabled": true } }
    });
    let json = discovered_json(&rows, &manifest);
    let list = json["discovered"].as_array().expect("discovered 数组");
    let demo = list
        .iter()
        .find(|row| row["id"] == "demo-mod")
        .expect("demo-mod 行");
    assert_eq!(demo["selectable"], serde_json::json!(true));
    assert_eq!(demo["enabled"], serde_json::json!(true));
    assert_eq!(demo["reason"], serde_json::Value::Null);
    let director = list
        .iter()
        .find(|row| row["id"] == "director")
        .expect("director 行");
    assert_eq!(director["selectable"], serde_json::json!(false));
    assert!(
        director["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("保留"))
    );
    assert_eq!(director["enabled"], serde_json::json!(false));
}

// ---------------------------------------------------------------- 选择写回

/// 临时 mods.json + 已注入 manifest 路径的 registry（不碰仓库根那份）。
fn registry_with_manifest(path: &Path) -> crate::mod_registry::ModRegistry {
    crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({})).with_manifest_path(path)
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("读 mods.json")).expect("JSON")
}

#[test]
fn folder_selection_writes_only_those_ids_and_keeps_everything_else() {
    let tree = Tree::new("select");
    let manifest = tree.join("mods.json");
    tree.write(
        "mods.json",
        r#"{
  "mods": {
    "unknown-mod": {"enabled": true, "config": {"keep": 1}},
    "folder-a": {"enabled": true, "config": {"k": "v"}}
  },
  "other": {"keep": true}
}"#,
    );
    let registry = registry_with_manifest(&manifest);
    let selectable = vec!["folder-a".to_string(), "folder-b".to_string()];
    let enabled = vec!["folder-b".to_string()];
    let changed = registry
        .persist_folder_selection(&selectable, &enabled)
        .expect("写回成功");
    assert_eq!(changed, 2, "folder-a true→false、folder-b 新增");

    let doc = read_json(&manifest);
    assert_eq!(doc["mods"]["folder-a"]["enabled"], serde_json::json!(false));
    assert_eq!(
        doc["mods"]["folder-a"]["config"]["k"],
        serde_json::json!("v"),
        "已有 config 原样保留"
    );
    assert_eq!(doc["mods"]["folder-b"]["enabled"], serde_json::json!(true));
    assert_eq!(
        doc["mods"]["unknown-mod"],
        serde_json::json!({"enabled": true, "config": {"keep": 1}}),
        "不认识的老 id 原样保留"
    );
    assert_eq!(doc["other"], serde_json::json!({"keep": true}));

    // 同一份勾选再写一次 = 无变更，不重写文件。
    let before = std::fs::read_to_string(&manifest).expect("读文件");
    let changed = registry
        .persist_folder_selection(&selectable, &enabled)
        .expect("写回成功");
    assert_eq!(changed, 0, "没有变化不写盘");
    assert_eq!(std::fs::read_to_string(&manifest).expect("读文件"), before);
}

#[test]
fn folder_selection_is_a_noop_without_manifest_path_or_ids() {
    let registry = crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({}));
    assert_eq!(
        registry
            .persist_folder_selection(&["folder-a".to_string()], &[])
            .expect("无 manifest 路径 = no-op"),
        0
    );
    let tree = Tree::new("select-noop");
    let registry = registry_with_manifest(&tree.join("mods.json"));
    assert_eq!(
        registry
            .persist_folder_selection(&[], &[])
            .expect("空列表 = no-op"),
        0
    );
    assert!(
        !tree.join("mods.json").exists(),
        "没有要写的 id 时不创建文件"
    );
}
