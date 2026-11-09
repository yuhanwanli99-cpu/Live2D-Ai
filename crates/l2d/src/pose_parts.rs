//! pose3 的 **part 层**（0.5.0 rc.4）：把 `PoseGroups` 变成 part 不透明度。
//!
//! # spike 结论（ayagami rev 640ae4b）
//!
//! ayagami 的 `Driver::apply_pose` / `Driver::set_pose` 对 `pose::Key::Part`
//! 走 `set_part_opacity_by_id`（`ayagami/src/driver/mod.rs:505-577`），即 part
//! 写入**是可见、可用的公开面**：`PoseStack::finalize` 里给 `Key::Part` 写
//! 0.0/1.0，`ModelRendererCore::update` 的 `set_pose` 就会把该 part 的
//! `user_opacity` 设成对应值——无需改 ayagami、无需碰骨骼矩阵。
//!
//! # 选择规则（本版：**静态**）
//!
//! - 每个 pose3 组选**第一个在 moc3 里真实存在**的 part 作为激活项（官方样本
//!   即下标 0），该组其余 part 写 0（隐藏）——修掉「没有 pose 层时两组手臂/
//!   腿同时显示」的画面错误；
//! - 激活项 `Link` 指向的同组 part 一起显示（`1.0`），其余隐藏；
//! - 模型里不存在的 part、越界 Link 下标一律忽略（不 panic、不发明 id）；
//! - 不在任何 pose3 组里的 part **不写**（保持默认 1.0）——无 pose3 的模型
//!   因此逐值无回归。
//!
//! **不做自动轮换**：pose3 作者通常让应用在组内切换（换手臂/腿姿势），但那属于
//! 动作层；本版不恢复 motion3/exp3、也没有切换驱动源，所以保持静态选择（可后续
//! 由导演层调用接口切换）。本模块只依赖 `PoseMap`，可在原生 `cargo test` 覆盖。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ayagami::pose;

use crate::asset::pose3::PoseGroups;

/// 激活 part 的不透明度。
pub const PART_VISIBLE: f32 = 1.0;
/// 被隐藏 part 的不透明度。
pub const PART_HIDDEN: f32 = 0.0;

/// part 层：按 pose3 组算出的 part → 不透明度表（不可变；装载时建一次）。
#[derive(Debug, Clone, Default)]
pub struct PartLayer {
    /// 每帧合进最终姿态的 part 值（空 = 不干预）。
    values: pose::Pose,
    /// 诊断 / 测试用：本层实际管理（写值）的 part → 是否可见。
    decisions: BTreeMap<String, bool>,
    /// 实际生效的 pose3 组数（组里至少有一个 part 在模型上存在才计数）。
    groups: usize,
}

impl PartLayer {
    /// 空层（无 pose3 / 不干预任何 part）。
    pub fn empty(map: Arc<pose::PoseMap>) -> Self {
        Self {
            values: pose::Pose::with_map(map),
            ..Self::default()
        }
    }

    /// 按 pose3 组构建 part 层（选择规则见模块头；模型里不存在的 id 全部忽略）。
    pub fn from_pose_groups(map: &Arc<pose::PoseMap>, groups: &PoseGroups) -> Self {
        let mut decisions: BTreeMap<String, bool> = BTreeMap::new();
        let mut applied_groups = 0_usize;
        for group in groups.groups() {
            if group.is_empty() {
                continue;
            }
            // 激活项 = 组内第一个模型里真实存在的 part（官方样本即下标 0）。
            let Some(active) = group.iter().position(|part| model_has_part(map, &part.id)) else {
                continue;
            };
            applied_groups += 1;
            let mut visible: BTreeSet<usize> = BTreeSet::new();
            visible.insert(active);
            if let Some(part) = group.get(active) {
                for &link in &part.link {
                    if link < group.len() && model_has_part(map, &group[link].id) {
                        visible.insert(link);
                    }
                }
            }
            for (index, part) in group.iter().enumerate() {
                if !model_has_part(map, &part.id) {
                    continue;
                }
                decisions.insert(part.id.clone(), visible.contains(&index));
            }
        }

        let mut values = pose::Pose::with_map(Arc::clone(map));
        for (id, visible) in &decisions {
            let opacity = if *visible { PART_VISIBLE } else { PART_HIDDEN };
            values.set(&pose::Key::part(id.as_str()), opacity);
        }
        Self {
            values,
            decisions,
            groups: applied_groups,
        }
    }

    /// 合进 `PoseStack::finalize` 的 part 值（空层 = 空 Pose，不改变任何 part）。
    pub fn values(&self) -> &pose::Pose {
        &self.values
    }

    /// 本层是否真的管理了 part（false = 无 pose3，或无一个 id 命中模型）。
    pub fn has_layer(&self) -> bool {
        !self.decisions.is_empty()
    }

    /// 实际生效的 pose3 组数。
    pub fn group_count(&self) -> usize {
        self.groups
    }

    /// 观测：本层给某 part 写入的不透明度（不管理该 part → `None`）。
    pub fn part_opacity(&self, id: &str) -> Option<f32> {
        let key = pose::Key::part(id);
        // `get_flattened` 对「表里有、值未写」也返回默认值；本层只报自己写过的。
        if !self.values.has_value(&key) {
            return None;
        }
        self.values.get_flattened(&key)
    }

    /// 可见 part id（测试 / 诊断用，字典序）。
    pub fn visible_ids(&self) -> Vec<&str> {
        self.decisions
            .iter()
            .filter(|(_, visible)| **visible)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// 被隐藏 part id（测试 / 诊断用，字典序）。
    pub fn hidden_ids(&self) -> Vec<&str> {
        self.decisions
            .iter()
            .filter(|(_, visible)| !**visible)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// `(可见数, 隐藏数)`。
    pub fn counts(&self) -> (usize, usize) {
        (self.visible_ids().len(), self.hidden_ids().len())
    }
}

fn model_has_part(map: &pose::PoseMap, id: &str) -> bool {
    map.has_key(&pose::Key::part(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ayagami::pose::Descriptor;

    /// 合成 part 描述表（min 0 / max 1 / default 1，与 moc3 part 语义一致）。
    fn parts_map(ids: &[&str]) -> Arc<pose::PoseMap> {
        let mut map = pose::PoseMap::new();
        for (index, id) in ids.iter().enumerate() {
            map.add(Descriptor {
                key: pose::Key::from_part((*id).to_owned()),
                uid: index as u64 + 1,
                name: None,
                min: 0.0,
                max: 1.0,
                default: 1.0,
            });
        }
        Arc::new(map)
    }

    fn groups(raw: &str) -> PoseGroups {
        PoseGroups::from_json(raw.as_bytes()).expect("合成 pose3 JSON 必须合法")
    }

    #[test]
    fn selects_first_part_and_hides_siblings_per_group() {
        let map = parts_map(&["A", "B", "C", "D"]);
        let layer = PartLayer::from_pose_groups(
            &map,
            &groups(r#"{"Groups":[[{"Id":"A"},{"Id":"B"}],[{"Id":"C"},{"Id":"D"}]]}"#),
        );
        assert!(layer.has_layer());
        assert_eq!(layer.group_count(), 2);
        assert_eq!(layer.part_opacity("A"), Some(PART_VISIBLE));
        assert_eq!(layer.part_opacity("B"), Some(PART_HIDDEN));
        assert_eq!(layer.part_opacity("C"), Some(PART_VISIBLE));
        assert_eq!(layer.part_opacity("D"), Some(PART_HIDDEN));
        assert_eq!(layer.counts(), (2, 2));
        assert_eq!(layer.visible_ids(), vec!["A", "C"]);
        assert_eq!(layer.hidden_ids(), vec!["B", "D"]);
    }

    #[test]
    fn link_parts_show_together_with_active_part() {
        let map = parts_map(&["A", "B", "C"]);
        let layer = PartLayer::from_pose_groups(
            &map,
            &groups(r#"{"Groups":[[{"Id":"A","Link":[2]},{"Id":"B"},{"Id":"C"}]]}"#),
        );
        assert_eq!(layer.part_opacity("A"), Some(PART_VISIBLE), "激活项可见");
        assert_eq!(layer.part_opacity("C"), Some(PART_VISIBLE), "Link 陪显");
        assert_eq!(layer.part_opacity("B"), Some(PART_HIDDEN), "未联动项隐藏");
        assert_eq!(layer.counts(), (2, 1));
    }

    #[test]
    fn missing_parts_are_skipped_and_active_moves_to_first_present() {
        // 模型只有 B：A 不是激活项，B 顶上来当激活项。
        let map = parts_map(&["B", "X"]);
        let layer =
            PartLayer::from_pose_groups(&map, &groups(r#"{"Groups":[[{"Id":"A"},{"Id":"B"}]]}"#));
        assert_eq!(layer.group_count(), 1);
        assert_eq!(layer.part_opacity("A"), None, "模型没有的 part 不得被写");
        assert_eq!(layer.part_opacity("B"), Some(PART_VISIBLE));
        assert_eq!(
            layer.part_opacity("X"),
            None,
            "组外 part 不由本层写（合成时保持默认 1.0）"
        );
    }

    #[test]
    fn group_without_any_present_part_is_ignored() {
        let map = parts_map(&["X"]);
        let layer =
            PartLayer::from_pose_groups(&map, &groups(r#"{"Groups":[[{"Id":"A"},{"Id":"B"}]]}"#));
        assert!(!layer.has_layer());
        assert_eq!(layer.group_count(), 0);
        assert_eq!(layer.part_opacity("X"), None);
        assert_eq!(layer.visible_ids(), Vec::<&str>::new());
    }

    #[test]
    fn out_of_range_or_missing_link_targets_are_ignored() {
        let map = parts_map(&["A", "B", "C"]);
        let layer = PartLayer::from_pose_groups(
            &map,
            &groups(r#"{"Groups":[[{"Id":"A","Link":[99,1]},{"Id":"B"},{"Id":"C"}]]}"#),
        );
        assert_eq!(layer.part_opacity("A"), Some(PART_VISIBLE));
        assert_eq!(layer.part_opacity("B"), Some(PART_VISIBLE), "组内 1 是 B");
        assert_eq!(layer.part_opacity("C"), Some(PART_HIDDEN), "越界 99 忽略");
    }

    #[test]
    fn empty_groups_leave_every_part_at_default() {
        let map = parts_map(&["A", "B"]);
        let layer = PartLayer::from_pose_groups(&map, &groups(r#"{"Groups":[]}"#));
        assert!(!layer.has_layer());
        assert_eq!(layer.part_opacity("A"), None);
        assert_eq!(layer.part_opacity("B"), None);
    }
}
