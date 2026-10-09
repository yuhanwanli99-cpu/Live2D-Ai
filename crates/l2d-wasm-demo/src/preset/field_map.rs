//! 三族表演字段（`body` / `head` / `expression`）→ 模型参数的**每皮套映射表**
//! （阶段4c，表演协议 v1 §5）。
//!
//! # 为什么单独一个模块
//!
//! v1 的核心是「LLM 只说语义字段，字段经映射表落到 `Param…`」——表是**数据**，
//! 不是提示词（§5.1 / §5.2）。表的内建默认值住在这里（原生可测），
//! 可选的单文件覆盖 `assets/actions/field_map.json`（O9：按 model id 分节）
//! 由 [`FieldMap::with_override_json`] 合并；本轮**不要求**每皮套一份，
//! 所以内建默认表就是 bai 皮套的完整映射。
//!
//! # 红线（协议 §4.6 / §5.5，回归逐条钉住）
//!
//! - `body` 只准写 `ParamBodyAngle*`；`head` 只准写 `ParamAngle*`；
//! - `expression` **只写五官**（[`FACIAL_PARAMS`]），绝不写 `ParamMouthOpenY`
//!   （口型归 TTS），也绝不写 `ParamAngle*` / `ParamBodyAngle*`（V3）；
//! - 白名单保持现状：所有参数必须落在 `ALLOWED_PARAMS` 的 13 个标准参数内（O11）。
//!
//! 轴的方向与量程都由表定义：`+x` / `+y` / `+z` 的目标参数与符号写在目标里，
//! LLM 只给归一化轴值（§5.2）。

use super::{ALLOWED_PARAMS, BODY_LIMIT, EXPRESSION_LIMIT, HEAD_LIMIT, ScaleClass, scale_class};

/// `expression` 字段允许写的**五官通道**（协议 §4.6）。
///
/// **不得**包含 `ParamMouthOpenY`（口型归 TTS），也**不得**包含任何
/// `ParamAngle*` / `ParamBodyAngle*`（V3：头身交给 body / head）。
pub const FACIAL_PARAMS: &[&str] = &[
    "ParamMouthForm",
    "ParamEyeLSmile",
    "ParamEyeRSmile",
    "ParamEyeLOpen",
    "ParamEyeROpen",
    "ParamBrowLY",
    "ParamBrowRY",
];

/// 三族表演字段（V2 / V3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// 半身摆动 / 倾斜（`ParamBodyAngle*`）。
    Body,
    /// 头部点头 / 摇头 / 歪头（`ParamAngle*`）。
    Head,
    /// 只写五官（V3）。
    Expression,
}

impl Field {
    /// 全部字段（固定顺序：body → head → expression）。
    pub const ALL: [Field; 3] = [Field::Body, Field::Head, Field::Expression];

    /// wire / JSON 名。
    pub const fn as_str(self) -> &'static str {
        match self {
            Field::Body => "body",
            Field::Head => "head",
            Field::Expression => "expression",
        }
    }

    /// 词表外 → `None`（调用方负责 warn / 丢弃）。
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "body" => Some(Field::Body),
            "head" => Some(Field::Head),
            "expression" => Some(Field::Expression),
            _ => None,
        }
    }

    /// 数组下标（状态机按字段分槽）。
    pub const fn index(self) -> usize {
        match self {
            Field::Body => 0,
            Field::Head => 1,
            Field::Expression => 2,
        }
    }
}

/// 一个字段在某条轴上的目标参数（`amount` 已含符号与量程）。
///
/// `amount` 的语义：归一化轴值 = 1.0、intensity = 1、通道倍率 = 1 时贡献的
/// **模型参数幅值**。body / head 的 amount 就是 scales.rs 的通道上限
/// （`BODY_LIMIT` / `HEAD_LIMIT`）；expression 的 amount 直接是现有表情包
/// 的五官表值（协议 §4.6：现有三个表情包的五官部分就是首版映射）。
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTarget {
    /// 目标参数名（必须在 `ALLOWED_PARAMS` 内）。
    pub param: String,
    /// 归一化幅值（含符号）。
    pub amount: f32,
}

/// 一个字段的三条轴（x / y / z）；`expression` 只用 `id` 选择，不走轴。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldAxes {
    pub x: Vec<AxisTarget>,
    pub y: Vec<AxisTarget>,
    pub z: Vec<AxisTarget>,
}

impl FieldAxes {
    /// 按轴下标（0/1/2）取目标。
    pub fn axis(&self, index: usize) -> &[AxisTarget] {
        match index {
            0 => &self.x,
            1 => &self.y,
            _ => &self.z,
        }
    }

    fn axis_mut(&mut self, index: usize) -> &mut Vec<AxisTarget> {
        match index {
            0 => &mut self.x,
            1 => &mut self.y,
            _ => &mut self.z,
        }
    }
}

/// `expression.id` → 五官通道表值。
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionEntry {
    pub id: String,
    pub targets: Vec<AxisTarget>,
}

/// 三字段 → 模型参数的映射表（每皮套一份）。
#[derive(Debug, Clone, PartialEq)]
pub struct FieldMap {
    model: String,
    body: FieldAxes,
    head: FieldAxes,
    expressions: Vec<ExpressionEntry>,
    warnings: Vec<String>,
}

/// `Param` 是否允许出现在该字段（协议 §4.6 / §5.5 的通道红线）。
///
/// 这是**唯一**的字段通道判据：建表（覆盖 JSON）与运行期落地都调它，
/// 因此「expression 只写五官」不可能只在一处生效。
pub fn field_allows(field: Field, param: &str) -> bool {
    match field {
        Field::Body => scale_class(param) == ScaleClass::Body,
        Field::Head => scale_class(param) == ScaleClass::Head,
        // 顺序要紧：FACIAL_PARAMS 是显式小词表——不能只判
        // scale_class == Expression（那会把 ParamMouthOpenY / ParamBreath 放进来）。
        Field::Expression => FACIAL_PARAMS.contains(&param),
    }
}

/// 从 model3 URL 取**皮套 / 模型 id**（`/models/bai/runtime/bai.model3.json` → `bai`）。
///
/// 覆盖文件按这个 id 分节（O9）。取不到 → 空串（覆盖层会退到 `default` 节）。
pub fn model_id_from_url(url: &str) -> String {
    let tail = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .unwrap_or("");
    let tail = tail
        .strip_suffix(".model3.json")
        .or_else(|| tail.strip_suffix(".json"))
        .unwrap_or(tail);
    tail.trim().to_string()
}

impl FieldMap {
    /// 内建默认表（= 本轮「每皮套」的完整映射；O9 内建默认 + 可选覆盖）。
    pub fn builtin() -> Self {
        Self {
            model: "builtin".to_string(),
            body: FieldAxes {
                x: vec![target("ParamBodyAngleX", BODY_LIMIT)],
                y: vec![target("ParamBodyAngleY", BODY_LIMIT)],
                z: vec![target("ParamBodyAngleZ", BODY_LIMIT)],
            },
            head: FieldAxes {
                x: vec![target("ParamAngleX", HEAD_LIMIT)],
                y: vec![target("ParamAngleY", HEAD_LIMIT)],
                z: vec![target("ParamAngleZ", HEAD_LIMIT)],
            },
            // 现有三个表情包（smile / unhappy / surprised）的**五官部分**：
            // 表值与 assets/actions/presets.json 逐值一致；头身分量按 V3 不在这里。
            expressions: vec![
                ExpressionEntry {
                    id: "smile".to_string(),
                    targets: vec![
                        target("ParamMouthForm", 1.0),
                        target("ParamEyeLSmile", 1.0),
                        target("ParamEyeRSmile", 1.0),
                        target("ParamBrowLY", 0.5),
                        target("ParamBrowRY", 0.5),
                    ],
                },
                ExpressionEntry {
                    // unhappy 的 low 极（难过相）；morph 是包时代的概念，
                    // v1 expression 字段取首版映射 = low 极。
                    id: "unhappy".to_string(),
                    targets: vec![
                        target("ParamMouthForm", -1.0),
                        target("ParamBrowLY", 0.4),
                        target("ParamBrowRY", 0.4),
                        target("ParamEyeLOpen", 0.65),
                        target("ParamEyeROpen", 0.65),
                    ],
                },
                ExpressionEntry {
                    // T9（2026-10-07）：问句补丁的思考表情。**只抄五官五行**——
                    // 预设包里的 AngleZ / BodyAngleZ 走 preset_id 通道；字段通道的
                    // 歪头由补丁的 head.z 负责，避免两路叠加。
                    id: "thinking".to_string(),
                    targets: vec![
                        target("ParamMouthForm", 0.0),
                        target("ParamEyeLOpen", 0.55),
                        target("ParamEyeROpen", 0.55),
                        target("ParamBrowLY", -0.45),
                        target("ParamBrowRY", -0.45),
                    ],
                },
                ExpressionEntry {
                    id: "surprised".to_string(),
                    targets: vec![
                        target("ParamEyeLOpen", 1.26),
                        target("ParamEyeROpen", 1.26),
                        target("ParamBrowLY", 0.8),
                        target("ParamBrowRY", 0.8),
                        target("ParamMouthForm", 0.2),
                    ],
                },
            ],
            warnings: Vec::new(),
        }
    }

    /// 用**可选**的单文件覆盖 JSON 合并内建表（O9）。
    ///
    /// 形状（`models` 可省略成扁平对象；取不到本 model 就退 `default` 节）：
    /// ```jsonc
    /// { "version": 1, "models": {
    ///     "bai": {
    ///       "body": { "x": [{"param":"ParamBodyAngleX","amount":10.0}] },
    ///       "head": { "y": [{"param":"ParamAngleY","amount":30.0}] },
    ///       "expression": { "smile": [{"param":"ParamMouthForm","amount":1.0}] }
    ///     },
    ///     "default": { }
    /// } }
    /// ```
    ///
    /// 合并粒度：**按轴 / 按 expression id 整体替换**；未提到的轴 / id 保持内建。
    /// 校验规则（宽容但不撒谎）：通道越界 → 丢该目标 + warning；amount 非有限 /
    /// 超通道上限 → 钳位 + warning。一条有效目标都没有的轴/id → 保持原值 + warning。
    pub fn with_override_json(model_id: &str, json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("不是合法 JSON：{e}"))?;
        let mut map = Self::builtin();
        map.model = if model_id.is_empty() {
            "default".to_string()
        } else {
            model_id.to_string()
        };
        let section = value
            .get("models")
            .and_then(|models| models.get(model_id).or_else(|| models.get("default")))
            .or_else(|| {
                if value.get("models").is_none() {
                    Some(&value)
                } else {
                    None
                }
            });
        let Some(section) = section.filter(|s| s.is_object()) else {
            map.warnings.push(format!(
                "field_map：没有 {model_id:?} / default 节，保持内建表"
            ));
            return Ok(map);
        };
        map.merge_field(Field::Body, section.get("body"));
        map.merge_field(Field::Head, section.get("head"));
        map.merge_expressions(section.get("expression"));
        Ok(map)
    }

    fn merge_field(&mut self, field: Field, raw: Option<&serde_json::Value>) {
        let Some(obj) = raw.filter(|v| v.is_object()) else {
            return;
        };
        for (axis, name) in ["x", "y", "z"].into_iter().enumerate() {
            let Some(arr) = obj.get(name) else { continue };
            let targets = self.parse_targets(field, arr);
            if targets.is_empty() {
                self.warnings.push(format!(
                    "field_map: {}({}) 的 {name} 轴没有有效目标，保持内建",
                    field.as_str(),
                    self.model
                ));
                continue;
            }
            let axes = self.axes_mut(field);
            *axes.axis_mut(axis) = targets;
        }
    }

    fn merge_expressions(&mut self, raw: Option<&serde_json::Value>) {
        let Some(obj) = raw.filter(|v| v.is_object()) else {
            return;
        };
        let Some(entries) = obj.as_object() else {
            return;
        };
        for (id, arr) in entries {
            let targets = self.parse_targets(Field::Expression, arr);
            if targets.is_empty() {
                self.warnings.push(format!(
                    "field_map: expression {id} 没有有效五官目标，保持内建"
                ));
                continue;
            }
            match self.expressions.iter_mut().find(|e| &e.id == id) {
                Some(entry) => entry.targets = targets,
                None => self.expressions.push(ExpressionEntry {
                    id: id.clone(),
                    targets,
                }),
            }
        }
    }

    /// 解析一组目标（`[{"param":…,"amount":…}]`），只留该字段允许的通道。
    fn parse_targets(&mut self, field: Field, raw: &serde_json::Value) -> Vec<AxisTarget> {
        let Some(arr) = raw.as_array() else {
            self.warnings
                .push(format!("field_map: {} 的某组目标不是数组", field.as_str()));
            return Vec::new();
        };
        let mut out: Vec<AxisTarget> = Vec::new();
        for item in arr {
            let param = item
                .get("param")
                .or_else(|| item.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if param.is_empty() {
                self.warnings
                    .push(format!("field_map: {} 有一条目标缺 param", field.as_str()));
                continue;
            }
            if !ALLOWED_PARAMS.contains(&param) {
                self.warnings
                    .push(format!("field_map: {param} 不在 13 参白名单内，已丢弃"));
                continue;
            }
            if !field_allows(field, param) {
                self.warnings.push(format!(
                    "field_map: {} 字段不允许写 {param}（通道红线），已丢弃",
                    field.as_str()
                ));
                continue;
            }
            let Some(amount) = item.get("amount").and_then(|v| v.as_f64()) else {
                self.warnings
                    .push(format!("field_map: {param} 的 amount 不是数字，已丢弃"));
                continue;
            };
            let mut amount = amount as f32;
            if !amount.is_finite() {
                self.warnings
                    .push(format!("field_map: {param} 的 amount 非有限，已丢弃"));
                continue;
            }
            let limit = channel_amount_limit(param);
            if amount.abs() > limit {
                self.warnings.push(format!(
                    "field_map: {param} 的 amount {amount} 超通道上限 {limit}，已钳位"
                ));
                amount = amount.clamp(-limit, limit);
            }
            out.push(target(param, amount));
        }
        out
    }

    fn axes(&self, field: Field) -> &FieldAxes {
        match field {
            Field::Body => &self.body,
            Field::Head => &self.head,
            Field::Expression => &self.body, // expression 不走轴；占位（不会命中）
        }
    }

    fn axes_mut(&mut self, field: Field) -> &mut FieldAxes {
        match field {
            Field::Body => &mut self.body,
            Field::Head => &mut self.head,
            Field::Expression => &mut self.body, // 占位；expression 改走 merge_expressions
        }
    }

    /// 皮套 id（诊断 / HUD）。
    pub fn model(&self) -> &str {
        &self.model
    }

    /// 加载期告警。
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// `body` / `head` 某条轴的目标（expression → 空）。
    pub fn axis_targets(&self, field: Field, axis: usize) -> &[AxisTarget] {
        if field == Field::Expression {
            return &[];
        }
        self.axes(field).axis(axis)
    }

    /// `expression.id` 的五官目标；未知 → `None`。
    pub fn expression_targets(&self, id: &str) -> Option<&[AxisTarget]> {
        self.expressions
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.targets.as_slice())
    }

    /// 全部已知 expression id（HUD / 诊断）。
    pub fn expression_ids(&self) -> Vec<&str> {
        self.expressions.iter().map(|e| e.id.as_str()).collect()
    }

    /// 表里声明的全部参数（回归用：证明不越 13 参白名单）。
    pub fn declared_params(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for axe in [&self.body, &self.head] {
            for list in [&axe.x, &axe.y, &axe.z] {
                for t in list {
                    if !out.contains(&t.param.as_str()) {
                        out.push(t.param.as_str());
                    }
                }
            }
        }
        for e in &self.expressions {
            for t in &e.targets {
                if !out.contains(&t.param.as_str()) {
                    out.push(t.param.as_str());
                }
            }
        }
        out
    }
}

/// 通道幅值上限（表内 amount 的钳位口径；与 scales.rs 同源）。
pub fn channel_amount_limit(param: &str) -> f32 {
    match scale_class(param) {
        ScaleClass::Head => HEAD_LIMIT,
        ScaleClass::Body => BODY_LIMIT,
        ScaleClass::Expression => EXPRESSION_LIMIT,
    }
}

fn target(param: &str, amount: f32) -> AxisTarget {
    AxisTarget {
        param: param.to_string(),
        amount,
    }
}
