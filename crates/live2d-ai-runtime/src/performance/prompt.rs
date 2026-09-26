//! 表演层的提示词与响应后处理（**只属于表演层**，与主模型路径无关）。
//!
//! # 两条路，同一份 schema
//!
//! - **structured 路**（优先）：请求带 response_format={"type":"json_schema","strict":true}，
//!   provider 在解码层保证形状；system 只说明角色与切分纪律；
//! - **prompt 路**（模型不支持 structured 时的兜底）：system 里写死「只输出 JSON」，
//!   响应再经 [strip_code_fence] 剥掉围栏——**仍然必须过
//!   [crate::performance::plan::parse_plan] 同一个校验器**（宽容的是围栏，不是契约）。
//!
//! 红线（协议 §5.1）：这两个提示词**不得**出现任何 Param… 参数名——模型只写
//! 归一化语义字段（body / head / expression + 轴值），参数名到映射表为止。
//! 回归 performance_prompt_never_contains_param_names 扫本文件常量与 schema。
//! 这两个提示词也**不得**出现在主模型路径上。主模型（酒馆式角色扮演）只拿
//! 人设 + 剧情/记忆注入，不暴露任何工具、不要求任何 JSON / 舞台指示。

/// 表演层 system（structured 路）：交代角色与两条硬纪律。
pub const SYSTEM_STRUCTURED: &str = "你是 Live2D 皮套的表演导演。你会读到本轮用户输入与主模型写好的原文，\
你只做两件事。第一，把原文切成若干段 segments：只能切分，绝不能改写、缩写、\
增删字词或调整顺序；把所有段按顺序拼起来必须与原文逐字完全相同（含标点与空白）；\
原文为空就给空数组。第二，给若干条表演 cue：field 取 body（半身摆动/倾斜）、\
head（点头/摇头/歪头）、expression（只写五官）；body 与 head 可选归一化轴值 \
x/y（-1..1），head 另可给 z（歪头倾斜），expression 用 id 指定表情面板项（none 表示撤销）；\
intensity 取 1（轻微）/2（中）/3（强）；at 取 now（立即）或 seg:N（第 N 段音频开始，\
N 从 1 起、不超过段数）或 after_prev（上一条做完后）；hold 为 true 表示保持到下次指令、\
false 表示按 ttl_ms（可选，默认 body/head 900、expression 2600 毫秒）到点回落。\
只做表演，不改变剧情立场，不添加原文没有的事实。";

/// 表演层 system（prompt 路）：在角色之外**写死输出契约**。
///
/// 为什么不用 tool_choice / function calling：见任务书——**不采用 optional multi-tool
/// + tool_choice=auto**（那是「赌模型愿不愿意调工具」，缺席时整轮没有表演）。
pub const SYSTEM_JSON_ONLY: &str = "你是 Live2D 皮套的表演导演。你会读到本轮用户输入与主模型写好的原文。\
只输出 JSON，不要解释、不要 Markdown 围栏、不要多余字段。\
格式必须是：{\"segments\": [字符串, ...], \"cues\": [{\"field\": \"body|head|expression\", \
\"x\": 数字?, \"y\": 数字?, \"z\": 数字?, \"id\": 字符串?, \"intensity\": 整数, \
\"at\": \"now|seg:N|after_prev\", \"hold\": true|false, \"ttl_ms\": 整数?}]}。\
segments 只能切分原文，逐字不变，全部按顺序拼接必须与原文完全相同；原文为空给空数组。\
cues 为空数组表示本轮不做动作。body/head 用 x/y（可选 z 仅 head，取值 -1..1）；\
expression 必须给 id（面板项，none 表示撤销）。intensity 取 1/2/3；\
at 的 seg:N 从 1 起且不超过段数；hold=true 表示保持到下次指令，false 按 ttl_ms 到点回落。";

/// 组装 user 消息：本轮用户输入 + 主模型原文 + 能力集。
///
/// 不含思考；不修改任何文本（原文只作输入，切分方案由表演层给出）。
pub fn build_user_prompt(user_text: &str, assistant_text: &str, allow: &[String]) -> String {
    format!(
        "【用户】{user_text}\n【主模型原文】{assistant_text}\n【能力集】{}\n\
【切分纪律】segments 只能切分上面的主模型原文：逐字不变，拼接后必须与原文完全相同。",
        allow.join(", ")
    )
}

/// 剥掉 Markdown 代码围栏（prompt 路的健壮解析）。
///
/// 只处理「整份被围栏包住」与「围栏后跟少量解释」两种常见脏输出：
/// 取第一个 左花括号 到最后一个 右花括号 之间的子串。找不到花括号就原样返回
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

#[cfg(test)]
mod tests {
    use super::strip_code_fence;

    #[test]
    fn strip_code_fence_takes_the_json_body() {
        assert_eq!(
            strip_code_fence("{\"segments\":[],\"cues\":[]}"),
            r#"{"segments":[],"cues":[]}"#
        );
        assert_eq!(
            strip_code_fence("好的，这是结果：{\"segments\":[\"嗯。\"],\"cues\":[]} 以上"),
            r#"{"segments":["嗯。"],"cues":[]}"#
        );
        assert_eq!(strip_code_fence("  nope  "), "nope");
    }
}
