//! 通用参数绑定层（0.5.0 rc.1）：moc3 参数 id 表 → 「标准语义名 → 模型实际 id」。
//!
//! # 为什么需要
//!
//! 仓库里的写参数链路（动作预设、三字段、待机体征、口型）一律用 13 条标准
//! Cubism 语义名（`ParamAngleX` / `ParamMouthOpenY` / …）。写入端到端是
//! **逐字精确匹配**（[`crate::pose_stack::PoseStack`] → ayagami 的
//! `HashMap<Key>`），于是：
//! - 标准命名（`ParamAngleX`）的模型能用——bai 128 参里只命中 **11/13**
//!   （moc3 没有 `ParamEyeLSmile` / `ParamEyeRSmile`）；
//! - 大写 snake（`PARAM_ANGLE_X`）的旧样本（epsilon / haru / simple）**0/13**，
//!   连呼吸、眨眼、摆头一起失联。
//!
//! 本模块只做一件事：把「语义名」归一到 canonical，再在**该模型自己的 moc3
//! 参数表**里找同名 canonical 的实际 id。
//!
//! # canonical 规则（去前缀 → 小写 → 剔除 `_`）
//!
//! | 实际 id | canonical |
//! |---|---|
//! | `ParamAngleX` / `PARAM_ANGLE_X` | `anglex` |
//! | `ParamBrowLY` / `PARAM_BROW_L_Y` | `browly` |
//! | `ParamEyeLSmile` / `PARAM_EYE_L_SMILE` | `eyelsmile` |
//!
//! 前缀只认 `Param` / `Param_` / `PARAM_`；其余 id 原样参与归一。
//!
//! # 冲突策略（同一 canonical 多个实际 id）
//!
//! 精确标准名 > 大写 snake（`PARAM_*`）> 其它；同级按参数表出现顺序保留
//! 先出现的一个，其余记进 [`ParamBinder::conflicts`]（装载期 warning 的数据
//! 来源）。解析失败 → `None`，调用方保持「缺 id 静默 false」，不 panic。
//!
//! # 本模块**不做**
//!
//! 扩展 13 条白名单、`Param*` → `PARAM_*` 硬别名表、大小写以外的模糊匹配、
//! 跨语义猜测（`ParamA` 只能经 model3 `Groups` 显式绑定，见
//! [`super::lip_sync`]）。
//!
//! # 注意
//!
//! 本层只**解析 id**，不判定幅值 / 通道；白名单与幅值仍在
//! `l2d-wasm-demo::preset`（`ALLOWED_PARAMS` 仍是 13 条，本模块不改协议）。

use std::collections::BTreeMap;

/// 13 条标准语义参数名（与 `l2d-wasm-demo` 的 `ALLOWED_PARAMS` 同集合）。
///
/// 依赖方向所限（wasm-demo 仅在 wasm32 目标依赖本 crate）无法共享同一常量，
/// 两处都改才算改白名单；本表只参与**冲突定级**，不参与写入白名单判定。
pub const STANDARD_PARAM_IDS: &[&str] = &[
    "ParamMouthForm",
    "ParamEyeLSmile",
    "ParamEyeRSmile",
    "ParamEyeLOpen",
    "ParamEyeROpen",
    "ParamBrowLY",
    "ParamBrowRY",
    "ParamAngleX",
    "ParamAngleY",
    "ParamAngleZ",
    "ParamBodyAngleX",
    "ParamBodyAngleY",
    "ParamBodyAngleZ",
];

/// 同一 canonical 出现多个实际 id 时的冲突记录（装载期 warning 的数据来源）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindConflict {
    /// 归一后的键（如 `anglex`）。
    pub canonical: String,
    /// 按冲突策略保留的实际 id。
    pub kept: String,
    /// 被放弃的实际 id（按参数表出现顺序）。
    pub dropped: Vec<String>,
}

/// canonical → 实际 id 的绑定表（不可变；装载模型时重建）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParamBinder {
    map: BTreeMap<String, String>,
    conflicts: Vec<BindConflict>,
}

impl ParamBinder {
    /// 按模型 moc3 的参数 id 表建绑定（输入顺序 = 参数表顺序，冲突以此稳定排序）。
    pub fn from_ids<I, S>(ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut groups: BTreeMap<String, Vec<(u8, usize, String)>> = BTreeMap::new();
        for (idx, id) in ids.into_iter().enumerate() {
            let id = id.as_ref();
            let key = canonical(id);
            if key.is_empty() {
                continue;
            }
            groups
                .entry(key)
                .or_default()
                .push((id_rank(id), idx, id.to_owned()));
        }

        let mut map = BTreeMap::new();
        let mut conflicts = Vec::new();
        for (canonical, mut entries) in groups {
            // 冲突策略：精确标准名(0) > 大写 snake(1) > 其它(2)；同级按参数表顺序。
            entries.sort();
            let (_, _, kept) = entries.remove(0);
            map.insert(canonical.clone(), kept.clone());
            if !entries.is_empty() {
                conflicts.push(BindConflict {
                    canonical,
                    kept,
                    dropped: entries.into_iter().map(|(_, _, id)| id).collect(),
                });
            }
        }
        Self { map, conflicts }
    }

    /// 表是否为空（空表 = 所有语义名都解析为 `None`，模型没有可绑参数）。
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// 已绑定的 canonical 数量。
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// 装载期冲突（同一 canonical 多个实际 id 时的落选者，按 canonical 排序）。
    pub fn conflicts(&self) -> &[BindConflict] {
        &self.conflicts
    }

    /// 语义名 / 实际 id → 本模型上应写的实际 id；解析失败返回 `None`。
    ///
    /// 入参先按 [`canonical`] 归一，所以标准名（`ParamAngleX`）与大写 snake
    /// （`PARAM_ANGLE_X`）都查到同一个绑定；表里没有该 canonical → `None`
    /// （调用方保持「缺 id 静默 false」，不 panic、不报错）。
    pub fn resolve(&self, id: &str) -> Option<&str> {
        let key = canonical(id);
        if key.is_empty() {
            return None;
        }
        self.map.get(&key).map(String::as_str)
    }
}

/// 计算参数 id 的 canonical（去前缀 → 小写 → 剔除 `_`）。
///
/// 例：`ParamAngleX` / `PARAM_ANGLE_X` → `anglex`；
/// `ParamBrowLY` / `PARAM_BROW_L_Y` → `browly`；`ParamA` → `a`。
pub fn canonical(id: &str) -> String {
    let rest = id
        .strip_prefix("PARAM_")
        .or_else(|| id.strip_prefix("Param_"))
        .or_else(|| id.strip_prefix("Param"))
        .unwrap_or(id);
    rest.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// 冲突定级：精确标准名 0 < 大写 snake 1 < 其它 2。
fn id_rank(id: &str) -> u8 {
    if STANDARD_PARAM_IDS.contains(&id) {
        0
    } else if is_upper_snake(id) {
        1
    } else {
        2
    }
}

/// `PARAM_*` 全大写 + 数字 + 下划线（epsilon / haru / simple 等旧样本的命名）。
fn is_upper_snake(id: &str) -> bool {
    id.starts_with("PARAM_")
        && id
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 仓库内 bai 的真实 moc3 参数表（白模型随仓库分发，读不到直接失败）。
    fn bai_parameter_ids() -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("assets/models/bai/runtime/bai.model3.json");
        let package = crate::asset::ModelPackage::load(&path)
            .unwrap_or_else(|e| panic!("读不到 bai 皮套包 {}: {e}", path.display()));
        let handle = crate::model::ModelHandle::from_package(&package)
            .unwrap_or_else(|e| panic!("bai moc3 解析失败: {e}"));
        handle.parameter_ids()
    }

    /// epsilon 旧样本的 13 条大写 snake 参数（与 [`STANDARD_PARAM_IDS`] 同序）。
    const UPPERCASE_IDS: &[&str] = &[
        "PARAM_MOUTH_FORM",
        "PARAM_EYE_L_SMILE",
        "PARAM_EYE_R_SMILE",
        "PARAM_EYE_L_OPEN",
        "PARAM_EYE_R_OPEN",
        "PARAM_BROW_L_Y",
        "PARAM_BROW_R_Y",
        "PARAM_ANGLE_X",
        "PARAM_ANGLE_Y",
        "PARAM_ANGLE_Z",
        "PARAM_BODY_ANGLE_X",
        "PARAM_BODY_ANGLE_Y",
        "PARAM_BODY_ANGLE_Z",
    ];

    /// 标准名 13 条去掉指定项后的合成参数表（构造 Mark / Rice / Mao 的缺项形态）。
    fn binder_without(missing: &[&str]) -> ParamBinder {
        ParamBinder::from_ids(
            STANDARD_PARAM_IDS
                .iter()
                .copied()
                .filter(|id| !missing.contains(id)),
        )
    }

    #[test]
    fn canonical_strips_prefix_lowercases_and_drops_underscores() {
        for (raw, want) in [
            ("ParamAngleX", "anglex"),
            ("PARAM_ANGLE_X", "anglex"),
            ("ParamBrowLY", "browly"),
            ("PARAM_BROW_L_Y", "browly"),
            ("ParamEyeLSmile", "eyelsmile"),
            ("PARAM_EYE_L_SMILE", "eyelsmile"),
            ("ParamBodyAngleZ", "bodyanglez"),
            ("ParamA", "a"),
            ("Unrelated", "unrelated"),
            ("", ""),
        ] {
            assert_eq!(canonical(raw), want, "canonical({raw:?})");
        }
    }

    #[test]
    fn bai_moc3_binds_eleven_of_thirteen_and_keeps_standard_ids() {
        let ids = bai_parameter_ids();
        let binder = ParamBinder::from_ids(&ids);
        let missing: Vec<&str> = STANDARD_PARAM_IDS
            .iter()
            .copied()
            .filter(|id| binder.resolve(id).is_none())
            .collect();
        assert_eq!(
            missing,
            vec!["ParamEyeLSmile", "ParamEyeRSmile"],
            "bai 128 参只命中 11/13（moc3 没有两个 eye smile）"
        );
        // 其余 11 条必须逐字保留 → 与 0.4.x 的精确写入逐值相同（bai 行为不变）。
        for id in STANDARD_PARAM_IDS.iter().copied() {
            if missing.contains(&id) {
                continue;
            }
            assert_eq!(
                binder.resolve(id),
                Some(id),
                "bai 标准名必须绑定到同名字面 id"
            );
        }
    }

    #[test]
    fn uppercase_snake_model_binds_all_thirteen() {
        let binder = ParamBinder::from_ids(UPPERCASE_IDS.iter().copied());
        for (semantic, actual) in STANDARD_PARAM_IDS.iter().zip(UPPERCASE_IDS) {
            assert_eq!(
                binder.resolve(semantic),
                Some(*actual),
                "{semantic} 应绑到大写 snake {actual}"
            );
        }
        // 拿实际 id 回查也必须命中（Groups 名单已在写实际 id 的场景）。
        assert_eq!(binder.resolve("PARAM_ANGLE_X"), Some("PARAM_ANGLE_X"));
        assert!(binder.conflicts().is_empty(), "一一对应时不应有冲突");
    }

    #[test]
    fn mao_missing_mouth_form_resolves_none_and_param_a_stays_semantic_free() {
        let ids: Vec<&str> = STANDARD_PARAM_IDS
            .iter()
            .copied()
            .filter(|id| *id != "ParamMouthForm")
            .chain(["ParamA"])
            .collect();
        let binder = ParamBinder::from_ids(ids);
        assert_eq!(binder.resolve("ParamMouthForm"), None, "Mao 缺嘴形参数");
        // ParamA 只是普通 id：能按字面写（Groups 名单），但不会被当成任何语义名。
        assert_eq!(binder.resolve("ParamA"), Some("ParamA"));
        let param_a_canonical = canonical("ParamA");
        assert!(
            STANDARD_PARAM_IDS
                .iter()
                .all(|id| canonical(id) != param_a_canonical),
            "绝不跨语义猜：ParamA 的 canonical 不可能是任何标准名"
        );
    }

    #[test]
    fn mark_and_rice_missing_channels_resolve_none() {
        // Mark：缺 MouthForm + 两个 Smile。
        let mark = binder_without(&["ParamMouthForm", "ParamEyeLSmile", "ParamEyeRSmile"]);
        for missing in ["ParamMouthForm", "ParamEyeLSmile", "ParamEyeRSmile"] {
            assert_eq!(mark.resolve(missing), None, "Mark 缺 {missing}");
        }
        assert_eq!(
            mark.resolve("ParamMouthOpenY"),
            None,
            "Mark 的嘴在 Groups 里"
        );

        // Rice：缺 AngleY、两个 Brow、MouthForm、两个 Smile（只剩 7/13）。
        let rice = binder_without(&[
            "ParamAngleY",
            "ParamBrowLY",
            "ParamBrowRY",
            "ParamMouthForm",
            "ParamEyeLSmile",
            "ParamEyeRSmile",
        ]);
        let resolved = STANDARD_PARAM_IDS
            .iter()
            .filter(|id| rice.resolve(id).is_some())
            .count();
        assert_eq!(resolved, 7, "Rice 只有 7/13 可绑，其余如实 None");
        for missing in [
            "ParamAngleY",
            "ParamBrowLY",
            "ParamBrowRY",
            "ParamMouthForm",
        ] {
            assert_eq!(rice.resolve(missing), None, "Rice 缺 {missing}");
        }
    }

    #[test]
    fn empty_table_resolves_everything_to_none() {
        let binder = ParamBinder::from_ids(std::iter::empty::<&str>());
        assert!(binder.is_empty());
        assert_eq!(binder.len(), 0);
        for id in STANDARD_PARAM_IDS {
            assert_eq!(binder.resolve(id), None, "空表不得凭空解析 {id}");
        }
        assert_eq!(binder.resolve("ParamBreath"), None);
        assert_eq!(binder.resolve(""), None, "空 id 不得解析");
    }

    #[test]
    fn conflict_prefers_exact_standard_name_and_is_stable() {
        let ids = ["PARAM_ANGLE_X", "ParamAngleX", "ParamAngle_X"];
        let first = ParamBinder::from_ids(ids);
        let second = ParamBinder::from_ids(ids);
        assert_eq!(first.resolve("ParamAngleX"), Some("ParamAngleX"));
        assert_eq!(
            first.resolve("PARAM_ANGLE_X"),
            Some("ParamAngleX"),
            "大写 snake 语义名与标准名归到同一绑定"
        );
        assert_eq!(first, second, "同输入两次构建逐位相等（稳定排序）");
        assert_eq!(first.conflicts().len(), 1);
        let conflict = &first.conflicts()[0];
        assert_eq!(conflict.canonical, "anglex");
        assert_eq!(conflict.kept, "ParamAngleX");
        assert_eq!(
            conflict.dropped,
            vec!["PARAM_ANGLE_X", "ParamAngle_X"],
            "落选者按定级 + 出现顺序稳定记录"
        );
    }

    #[test]
    fn conflict_without_exact_name_prefers_uppercase_snake() {
        let binder = ParamBinder::from_ids(["ParamAngle_X", "PARAM_ANGLE_X"]);
        assert_eq!(binder.resolve("ParamAngleX"), Some("PARAM_ANGLE_X"));
        assert_eq!(
            binder.conflicts()[0].dropped,
            vec!["ParamAngle_X"],
            "非标准命名时大写 snake 赢，其它命名进冲突记录"
        );
    }
}
