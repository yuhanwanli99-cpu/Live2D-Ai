//! 表演层的提示词与响应后处理（**只属于表演层**，与主模型路径无关）。
//!
//! # 两条路，同一份 schema
//!
//! - **structured 路**（优先）：请求带 `response_format={"type":"json_schema","strict":true}`，
//!   provider 在解码层保证形状；system 只说明角色，不再重复 schema；
//! - **prompt 路**（模型不支持 structured 时的兜底）：system 里写死「只输出 JSON」，
//!   响应再经 [strip_code_fence] 剥掉 `json` 围栏——**仍然必须过
//!   [crate::performance::plan::parse_plan] 同一个校验器**（宽容的是围栏，不是契约）。
//!
//! 红线：这两个提示词**不得**出现在主模型路径上。主模型（酒馆式角色扮演）只拿
//! 人设 + 剧情/记忆注入，不暴露任何工具、不要求任何 JSON / 舞台指示
//!（见 docs/architecture/performance-layer-v0.md 的主模型审计一节）。

/// 表演层 system（structured 路）：只交代角色与产物用途。
pub const SYSTEM_STRUCTURED: &str = "你是 Live2D 皮套的表演导演。你会读到本轮用户输入与主模型写好的原文，\
你需要把它整理成这一轮真正要说的话（speak）与按句动作（cues）。\
只做表演，不改变剧情立场，不添加原文没有的事实。";

/// 表演层 system（prompt 路）：在角色之外**写死输出契约**。
///
/// 为什么不用 tool_choice / function calling：见任务书——**不采用 optional multi-tool
/// + tool_choice=auto**（那是「赌模型愿不愿意调工具」，缺席时整轮没有表演）。
pub const SYSTEM_JSON_ONLY: &str = "你是 Live2D 皮套的表演导演。你会读到本轮用户输入与主模型写好的原文，\
你需要把它整理成这一轮真正要说的话（speak）与按句动作（cues）。\
只输出 JSON，不要解释、不要 Markdown 围栏、不要多余字段。\
格式必须是：{\"speak\": string|null, \"cues\": [{\"sentence_seq\": 整数, \"preset_id\": 字符串, \"intensity\": 整数, \"ttl_ms\": 整数}]}。\
speak 为 null 或空串表示本轮不说话；cues 为空数组表示本轮不做动作。\
preset_id 只能取能力集里的值；sentence_seq 从 1 开始，对应 speak 切出的句子。";

/// 组装 user 消息：本轮用户输入 + 主模型原文 + 能力集。
///
/// 不含思考；不修改任何文本（speak 由表演层给出，原文只作输入）。
pub fn build_user_prompt(user_text: &str, assistant_text: &str, allow: &[String]) -> String {
    format!(
        "【用户】{user_text}\n【主模型原文】{assistant_text}\n【能力集】{}",
        allow.join(", ")
    )
}

/// 剥掉 Markdown 代码围栏（prompt 路的健壮解析）。
///
/// 只处理「整份被围栏包住」与「围栏后跟少量解释」两种常见脏输出：
/// 取第一个 `{` 到最后一个 `}` 之间的子串。找不到花括号就原样返回
///（交给校验器判失败，不在这里假装成功）。
pub fn strip_code_fence(raw: &str) -> String {
    let trimmed = raw.trim();
    let Some(start) = trimmed.find('{') else {
        return trimmed.to_string();
    };
    let Some(end) = trimmed.rfind('}') else {
        return trimmed.to_string();
    };
    if end <= start {
        return trimmed.to_string();
    }
    trimmed[start..=end].to_string()
}
