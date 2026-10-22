//! 多锚点文件夹 Mod 扫描（0.4.0 rc.1「Windows 包」）。
//!
//! 锚点由 [`crate::host_layout::mods_scan_roots`] 给出：**cwd 的 `mods/` 在前**，
//! exe 目录的 `mods/` 在后。**同一 id 以 cwd 为准**：后一个锚点里的同 id 行降级
//! 为「不可用（重复）」并保留在列表里（与 0.3.12 单锚点内的重复口径一致，页面
//! 不静默吞行）。单锚点内的全部校验（保留 id / 未知键 / `api_version` / 程序文件
//! 词法 / 符号链接）原样复用 [`crate::mods_scan::scan`]；这里只做跨锚点合并。
//!
//! **不执行**任何 `entry` / `launch.program`，也不写盘——与 0.3.12 的只读预检
//! 同一条边界；启动仍只由 `POST /api/v1/mods/discovered` 在勾选写盘之后触发。

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::mods_scan::{DiscoveredMod, scan};

/// 依序扫描 `roots`，按行 id 去重（**先出现者胜** = cwd 优先）。
pub fn scan_across(roots: &[PathBuf], registered: &[&str]) -> Vec<DiscoveredMod> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<DiscoveredMod> = Vec::new();
    for root in roots {
        for row in scan(root, registered) {
            if seen.insert(row.id.clone()) {
                out.push(row);
            } else {
                out.push(DiscoveredMod::invalid(
                    &row.id,
                    &row.dir,
                    "重复（同一 id 已在前面的 mods/ 里出现；以先出现的为准）",
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 临时目录树（Drop 时删干净）。
    struct Tree {
        root: PathBuf,
    }

    impl Tree {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "l2d-scan-across-{}-{}-{tag}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("临时目录");
            Self { root }
        }

        fn write(&self, rel: &str, body: &str) -> PathBuf {
            let path = self.root.join(rel);
            std::fs::create_dir_all(path.parent().expect("父目录")).expect("父目录");
            std::fs::write(&path, body).expect("写夹具");
            path
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn manifest(id: &str, version: &str) -> String {
        format!(
            "schema = 1\nid = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\n\
             api_version = 1\nkind = \"process\"\nentry = \"run.sh\"\n"
        )
    }

    #[test]
    fn cwd_root_wins_over_exe_root_for_the_same_id() {
        let tree = Tree::new("precedence");
        let cwd_root = tree.root.join("cwd/mods");
        let exe_root = tree.root.join("exe/mods");
        tree.write("cwd/mods/demo/mod.toml", &manifest("demo", "1.0.0"));
        tree.write("cwd/mods/demo/run.sh", "#!/bin/sh\n");
        tree.write("exe/mods/demo/mod.toml", &manifest("demo", "2.0.0"));
        tree.write("exe/mods/demo/run.sh", "#!/bin/sh\n");

        let rows = scan_across(&[cwd_root.clone(), exe_root.clone()], &[]);
        assert_eq!(rows.len(), 2, "两处各一行都要列出：{rows:?}");
        // 第一行 = cwd 那份（可选，版本 1.0.0，目录也在 cwd 下）。
        assert!(rows[0].selectable, "{rows:?}");
        assert_eq!(rows[0].version, "1.0.0");
        assert_eq!(rows[0].dir, cwd_root.join("demo"));
        // 第二行 = exe 那份降级为「重复」，不可选，但页面能看见原因。
        assert!(!rows[1].selectable, "同一 id 以 cwd 为准：{rows:?}");
        assert_eq!(rows[1].id, "demo");
        assert_eq!(rows[1].dir, exe_root.join("demo"));
        assert!(
            rows[1]
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("重复")),
            "{rows:?}"
        );
        // 程序路径来自 cwd 那份（启动时不会用到 exe 那份）。
        assert_eq!(rows[0].program, Some(cwd_root.join("demo/run.sh")));
    }

    #[test]
    fn distinct_ids_are_merged_and_reserved_ids_stay_unselectable() {
        let tree = Tree::new("merge");
        let cwd_root = tree.root.join("cwd/mods");
        let exe_root = tree.root.join("exe/mods");
        tree.write("cwd/mods/from-cwd/mod.toml", &manifest("from-cwd", "1.0.0"));
        tree.write("cwd/mods/from-cwd/run.sh", "#!/bin/sh\n");
        tree.write("exe/mods/from-exe/mod.toml", &manifest("from-exe", "1.0.0"));
        tree.write("exe/mods/from-exe/run.sh", "#!/bin/sh\n");
        // 保留 id 在两处都不可选（0.3.12 的名单不变）。
        tree.write(
            "exe/mods/local-tts/mod.toml",
            &manifest("local-tts", "1.0.0"),
        );
        tree.write("exe/mods/local-tts/run.sh", "#!/bin/sh\n");

        let rows = scan_across(&[cwd_root, exe_root], &[]);
        let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        assert_eq!(ids, vec!["from-cwd", "from-exe", "local-tts"], "{rows:?}");
        assert!(rows[0].selectable && rows[1].selectable, "{rows:?}");
        assert!(!rows[2].selectable, "保留 id 仍不可选：{rows:?}");
        assert!(
            rows[2]
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("保留")),
            "{rows:?}"
        );
    }

    #[test]
    fn empty_or_missing_roots_yield_an_empty_list() {
        let missing = std::env::temp_dir().join("l2d-scan-across-does-not-exist");
        assert!(scan_across(&[], &[]).is_empty());
        assert!(scan_across(std::slice::from_ref(&missing), &[]).is_empty());
        assert!(scan_across(&[missing.clone(), missing.clone()], &[]).is_empty());
        assert!(!missing.exists());
    }
}
