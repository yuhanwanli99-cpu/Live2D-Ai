//! `mod.toml` 的**严格 TOML 子集**解析（0.3.12 清单预检专用，只读）。
//!
//! # 为什么不是通用 TOML 解析器
//!
//! 本模块只读 `schema = 1` 用到的几个键（见 `mods_scan` 的字段表），而
//! `live2d-ai-desktop` 的顶层 `[dependencies]` 已顶格（18/18，执行单 §2 明令
//! 不放宽），所以这里**不引入 toml / toml_edit 依赖**，手写一个**故意窄**的
//! 解析器：
//!
//! - 只认 `键 = 字符串 / 整数 / 布尔` 与唯一的 `[launch]` 表；
//! - 注释 `#`（引号内的 `#` 不算），行尾空白忽略；
//! - 子集之外的一切（数组、内联表、多行字符串、日期、`[a.b]` 嵌套表、
//!   重复键、带点键）一律回 `Err`——调用方把这份清单判成「不可用」并写明原因。
//!
//! 这是**失败关闭**：不认识的写法不会被猜成合法值。多行字符串这类合法但
//! 本版不支持的 TOML，结论是「这份清单不可用（原因写明）」，不是「猜着读」。
//!
//! 解析只发生在内存里；**不执行**清单里的任何路径（执行单 §2）。

use std::collections::BTreeMap;

/// TOML 子集里允许的一个值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Value {
    /// 字符串（基本字符串 `"..."` 或字面量字符串 `'...'`）。
    Str(String),
    /// 64 位整数（本版只用到 `schema` / `api_version`）。
    Int(i64),
    /// 布尔。
    Bool(bool),
}

impl Value {
    /// 取字符串值；不是字符串回 `None`（调用方据此判「必须是字符串」）。
    pub(super) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// 取整数值；不是整数回 `None`（只有解析器自测用得到，故仅测试构建编译）。
    #[cfg(test)]
    pub(super) fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            _ => None,
        }
    }
}

/// 一份解析后的清单：顶层键 + 可选的 `[launch]` 表。
#[derive(Debug, Default)]
pub(super) struct Document {
    /// `[launch]` 表之前的顶层键。
    pub(super) top: BTreeMap<String, Value>,
    /// `[launch]` 表（**只允许**这一个表；出现别的表头即不可用）。
    pub(super) launch: Option<BTreeMap<String, Value>>,
}

/// 解析清单正文。错误信息带行号，供页面「不可用」的原因直接引用。
pub(super) fn parse(text: &str) -> Result<Document, String> {
    let mut doc = Document::default();
    let mut in_launch = false;
    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        // 表头：只允许 [launch]。
        if let Some(rest) = line.strip_prefix('[') {
            let Some(name) = rest.strip_suffix(']') else {
                return Err(format!("第 {line_no} 行：表头缺少 `]`"));
            };
            let name = name.trim();
            if name != "launch" {
                return Err(format!("第 {line_no} 行：未知表 `[{name}]`"));
            }
            if doc.launch.is_some() {
                return Err(format!("第 {line_no} 行：重复定义 `[launch]`"));
            }
            doc.launch = Some(BTreeMap::new());
            in_launch = true;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("第 {line_no} 行：缺少 `=`"));
        };
        let key = key.trim();
        if !is_bare_key(key) {
            return Err(format!("第 {line_no} 行：键名不合法 `{key}`"));
        }
        let parsed =
            parse_value(value.trim()).map_err(|reason| format!("第 {line_no} 行：{reason}"))?;
        let table = if in_launch {
            doc.launch.get_or_insert_with(BTreeMap::new)
        } else {
            &mut doc.top
        };
        if table.insert(key.to_string(), parsed).is_some() {
            return Err(format!("第 {line_no} 行：重复键 `{key}`"));
        }
    }
    Ok(doc)
}

/// 去掉行尾注释：引号内的 `#` 不算注释。基本字符串里 `\` 转义一个字符。
fn strip_comment(line: &str) -> &str {
    let mut basic = false;
    let mut literal = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if basic => escaped = true,
            '"' if !literal => basic = !basic,
            '\'' if !basic => literal = !literal,
            '#' if !basic && !literal => return &line[..index],
            _ => {}
        }
    }
    line
}

/// 裸键（TOML 的 `A-Za-z0-9_-`）；带引号键 / 带点键一律不支持。
fn is_bare_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// 解析一个值（已去掉行尾注释与两侧空白）。
fn parse_value(text: &str) -> Result<Value, String> {
    if let Some(rest) = text.strip_prefix('"') {
        return parse_basic_string(rest).map(Value::Str);
    }
    if let Some(rest) = text.strip_prefix('\'') {
        return parse_literal_string(rest).map(Value::Str);
    }
    if text == "true" {
        return Ok(Value::Bool(true));
    }
    if text == "false" {
        return Ok(Value::Bool(false));
    }
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '+')
    {
        return text
            .parse::<i64>()
            .map(Value::Int)
            .map_err(|_| format!("整数超出范围：`{text}`"));
    }
    Err(format!(
        "值类型不支持（本版只认字符串、整数、布尔）：`{text}`"
    ))
}

/// `"..."`：支持 `\\` `\"` `\n` `\t` `\r`；其余转义一律不支持。
fn parse_basic_string(text: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if !chars.as_str().trim().is_empty() {
                    return Err("字符串后面还有多余字符".to_string());
                }
                return Ok(out);
            }
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => return Err(format!("不支持的转义 `\\{other}`")),
                None => return Err("字符串没有收尾的引号".to_string()),
            },
            other => out.push(other),
        }
    }
    Err("字符串没有收尾的引号".to_string())
}

/// `'...'`：字面量字符串，没有任何转义。
fn parse_literal_string(text: &str) -> Result<String, String> {
    match text.split_once('\'') {
        Some((body, rest)) if rest.trim().is_empty() => Ok(body.to_string()),
        Some(_) => Err("字符串后面还有多余字符".to_string()),
        None => Err("字符串没有收尾的引号".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_manifest_schema_subset() {
        let doc = parse(
            r#"
# 注释
schema = 1
id = "hello-mod"   # 行尾注释
name = 'Héllo # 井号在字面量里'
version = "1.0.0"
api_version = 1
kind = "launcher"
description = "带 \"引号\" 的描述"

[launch]
program = "./bin/start"
mode = "with_app"
"#,
        )
        .expect("子集写法必须解析成功");
        assert_eq!(doc.top.get("schema").and_then(Value::as_int), Some(1));
        assert_eq!(
            doc.top.get("name").and_then(Value::as_str),
            Some("Héllo # 井号在字面量里")
        );
        assert_eq!(
            doc.top.get("description").and_then(Value::as_str),
            Some("带 \"引号\" 的描述")
        );
        let launch = doc.launch.expect("[launch] 表");
        assert_eq!(launch.get("mode").and_then(Value::as_str), Some("with_app"));
    }

    #[test]
    fn rejects_things_outside_the_subset() {
        for body in [
            "schema = 1\npermissions = []\n",
            "schema = [1]\n",
            "schema = 1\n[permissions]\nwrite = true\n",
            "schema = 1\nlaunch.mode = \"with_app\"\n",
            "schema = 1\nschema = 2\n",
            "schema = 1\n\"id\" = \"x\"\n",
            "schema = 1\n[launch]\n[launch]\n",
            "schema =\n",
        ] {
            assert!(parse(body).is_err(), "必须拒绝：{body:?}");
        }
    }
}
