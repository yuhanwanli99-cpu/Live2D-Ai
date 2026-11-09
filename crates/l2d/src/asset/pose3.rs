//! pose3.json 解析（0.5.0 rc.4）：model3 `FileReferences.Pose` 指向的「骨骼/部件组」。
//!
//! # 形状（Cubism 官方样本实测，haru.pose3.json）
//!
//! ```json
//! {
//!   "Type": "Live2D Pose",
//!   "Groups": [
//!     [
//!       {"Id": "PARTS_01_ARM_R_A_001", "Link": []},
//!       {"Id": "PARTS_01_ARM_R_B_001", "Link": []}
//!     ],
//!     [{"Id": "PARTS_01_ARM_L_A_001", "Link": []}]
//!   ]
//! }
//! ```
//!
//! - `Groups` 是**组**的数组；同一组内的 part 互斥（正常只显示其中一个）；
//! - 每个 part 是 `{Id, Link}`：`Id` 是 moc3 part id；`Link` 是同组内
//!   **陪显**的 part 下标数组（缺省 / 空 = 没有联动）。haru 样本两个组的 Link
//!   都是空数组；非空 Link 的语义按 Cubism 公开格式实现为「激活 part 时同组
//!   Link 指向的 part 一起显示」（本机样本不足，非空形态只有合成单测覆盖）。
//! - `Version` 字段在官方样本里**不存在**（只有 `Type`），因此不要求、不校验；
//!   其它未知字段（`Meta` 等）忽略。
//!
//! # 边界
//!
//! 本模块只**解析**，不决定显示谁（part 层在 `crate::pose_parts`）；不读
//! exp3 / motion3，也不解释 part 与骨骼的对应关系（moc3 里 part 本来就连着
//! 网格，写 part 不透明度即隐藏/显示那一组）。
//!
//! # 错误
//!
//! 清单声明了 `Pose` 就按**严格**读盘（与 physics3 / cdi3 同一条纪律）：
//! 不是合法 JSON、缺 `Groups`、组不是数组、part 缺 `Id` → 类型化错误；
//! `Link` 里的非整数项**跳过**（宽容，不整份失败）。

use serde_json::Value;

/// pose3.json 解析错误（字符串携带原因，不 panic）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pose3Error {
    /// 不是合法 JSON。
    Json(String),
    /// 结构不符合 pose3 约定（缺 Groups / 组不是数组 / part 缺 Id 等）。
    Shape(String),
}

impl std::fmt::Display for Pose3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "pose3.json 不是合法 JSON: {e}"),
            Self::Shape(e) => write!(f, "pose3.json 结构非法: {e}"),
        }
    }
}

impl std::error::Error for Pose3Error {}

/// pose3 里的一个 part：moc3 part id + 同组陪显下标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PosePart {
    /// moc3 part id（原样，不作大小写/前缀归一——part id 不是参数）。
    pub id: String,
    /// 同组内陪显 part 的下标（越界项在使用侧忽略）。
    pub link: Vec<usize>,
}

/// pose3.json 的 `Groups`（外层 = 组，内层 = 该组互斥 part）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoseGroups {
    groups: Vec<Vec<PosePart>>,
}

impl PoseGroups {
    /// 从 pose3.json 字节解析（严格结构；见模块头）。
    pub fn from_json(bytes: &[u8]) -> Result<Self, Pose3Error> {
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| Pose3Error::Json(e.to_string()))?;
        let raw_groups = value
            .get("Groups")
            .and_then(Value::as_array)
            .ok_or_else(|| Pose3Error::Shape("缺少 Groups 数组".to_owned()))?;

        let mut groups = Vec::with_capacity(raw_groups.len());
        for (gi, raw_group) in raw_groups.iter().enumerate() {
            let entries = raw_group
                .as_array()
                .ok_or_else(|| Pose3Error::Shape(format!("Groups[{gi}] 不是数组")))?;
            let mut parts = Vec::with_capacity(entries.len());
            for (pi, entry) in entries.iter().enumerate() {
                let id = entry
                    .get("Id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| {
                        Pose3Error::Shape(format!("Groups[{gi}][{pi}].Id 缺失或不是非空字符串"))
                    })?;
                let link = match entry.get("Link") {
                    None | Some(Value::Null) => Vec::new(),
                    Some(Value::Array(items)) => items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|v| v as usize)
                        .collect(),
                    Some(other) => {
                        return Err(Pose3Error::Shape(format!(
                            "Groups[{gi}][{pi}].Link 必须是数组（实得 {other}）"
                        )));
                    }
                };
                parts.push(PosePart {
                    id: id.to_owned(),
                    link,
                });
            }
            groups.push(parts);
        }
        Ok(Self { groups })
    }

    /// 全部组（外层组、内层 part，保持 JSON 顺序）。
    pub fn groups(&self) -> &[Vec<PosePart>] {
        &self.groups
    }

    /// 组数。
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// part 总数（所有组求和）。
    pub fn part_count(&self) -> usize {
        self.groups.iter().map(Vec::len).sum()
    }

    /// 没有组 / 组全空 ⇒ 没有可应用的骨骼层。
    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(Vec::is_empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> PoseGroups {
        PoseGroups::from_json(raw.as_bytes()).expect("样例必须可解析")
    }

    #[test]
    fn parses_official_haru_shape() {
        // 与 /tmp 官方样本 haru.pose3.json 逐字段一致（无 Version、Type 忽略）。
        let groups = parse(
            r#"{
                "Type": "Live2D Pose",
                "Groups": [
                    [
                        {"Id": "PARTS_01_ARM_R_A_001", "Link": []},
                        {"Id": "PARTS_01_ARM_R_B_001", "Link": []}
                    ],
                    [
                        {"Id": "PARTS_01_ARM_L_A_001", "Link": []},
                        {"Id": "PARTS_01_ARM_L_B_001", "Link": []}
                    ]
                ]
            }"#,
        );
        assert_eq!(groups.group_count(), 2);
        assert_eq!(groups.part_count(), 4);
        assert!(!groups.is_empty());
        assert_eq!(groups.groups()[0][0].id, "PARTS_01_ARM_R_A_001");
        assert!(groups.groups()[0][0].link.is_empty());
        assert_eq!(groups.groups()[1][1].id, "PARTS_01_ARM_L_B_001");
    }

    #[test]
    fn parses_non_empty_link_and_skips_non_integer_items() {
        let groups =
            parse(r#"{"Groups":[[{"Id":"A","Link":[2,1,"x",null,-1]},{"Id":"B"},{"Id":"C"}]]}"#);
        assert_eq!(groups.groups()[0][0].link, vec![2, 1], "非整数项跳过");
        assert!(groups.groups()[0][1].link.is_empty(), "缺 Link = 空");
    }

    #[test]
    fn missing_or_malformed_shape_is_typed_error() {
        for (raw, kind) in [
            ("{oops", "json"),
            ("[]", "shape"),
            (r#"{"Type":"Live2D Pose"}"#, "shape"),
            (r#"{"Groups":{}}"#, "shape"),
            (r#"{"Groups":[{}]}"#, "shape"),
            (r#"{"Groups":[[{}]]}"#, "shape"),
            (r#"{"Groups":[[{"Id":""}]]}"#, "shape"),
            (r#"{"Groups":[[{"Id":"A","Link":1}]]}"#, "shape"),
        ] {
            let err = PoseGroups::from_json(raw.as_bytes()).expect_err(raw);
            match kind {
                "json" => assert!(matches!(err, Pose3Error::Json(_)), "{raw}: {err}"),
                _ => assert!(matches!(err, Pose3Error::Shape(_)), "{raw}: {err}"),
            }
        }
    }

    #[test]
    fn empty_groups_is_empty_pose() {
        let groups = parse(r#"{"Groups":[]}"#);
        assert!(groups.is_empty());
        assert_eq!(groups.group_count(), 0);
        let groups = parse(r#"{"Groups":[[]]}"#);
        assert!(groups.is_empty(), "组全空同样没有可应用项");
    }
}
