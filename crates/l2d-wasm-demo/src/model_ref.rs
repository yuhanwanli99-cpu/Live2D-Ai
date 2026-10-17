//! model3 清单相对引用的**校验 + URL 拼接**（平台无关，原生 cargo test 可测）。
//!
//! 2026-10-17（0.3.5，P1）：旧 web/net.rs::resolve_relative 只做字符串拼接，
//! 对 FileReferences 里的值**零校验**——一个恶意/损坏的 model3 可以让渲染面
//! fetch 任意绝对路径、file: / https: scheme，或用 ../ 跳出模型目录。
//!
//! 这里只做**引用形态**校验，不改变资源根：通过后仍然拼在 model3 URL 所在目录下。
//! 拒绝：
//! - 空串；
//! - 以 / 或 \ 开头的绝对路径；
//! - 任何 .. 路径段（/ 与 \ 都当分隔符）；
//! - 带 scheme 的串（https: / file: / data: / Windows C:）；
//! - 含 NUL。

/// 校验一条 model3 相对引用；通过返回原值（仅去掉前导 ./）。
pub(crate) fn validate_reference(rel: &str) -> Result<&str, String> {
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    if rel.is_empty() {
        return Err("引用为空".to_string());
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(format!("拒绝绝对路径引用：{rel:?}"));
    }
    if rel.contains('\0') {
        return Err(format!("引用含 NUL：{rel:?}"));
    }
    if rel.split(['/', '\\']).any(|part| part == "..") {
        return Err(format!("拒绝 .. 越界引用：{rel:?}"));
    }
    if has_scheme(rel) {
        return Err(format!("拒绝带 scheme 的引用：{rel:?}"));
    }
    Ok(rel)
}

fn has_scheme(rel: &str) -> bool {
    let Some((head, _)) = rel.split_once(':') else {
        return false;
    };
    // scheme 不能含路径分隔符；首字符必须是 ASCII 字母，其余为
    // 字母 / 数字 / + / - / .（RFC 3986）。C:\ 也在这里被拒。
    if head.is_empty() || head.contains(['/', '\\']) {
        return false;
    }
    let mut chars = head.chars();
    let first = chars.next().expect("non-empty");
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// 把清单相对引用拼到 model3 URL 所在目录下（校验失败 → Err）。
pub(crate) fn resolve_relative(model_url: &str, rel: &str) -> Result<String, String> {
    let rel = validate_reference(rel)?;
    let dir = match model_url.rsplit_once('/') {
        Some((dir, _)) => dir,
        None => "",
    };
    Ok(if dir.is_empty() {
        rel.to_owned()
    } else {
        format!("{dir}/{rel}")
    })
}

#[cfg(test)]
mod tests {
    use super::{resolve_relative, validate_reference};

    #[test]
    fn normal_relative_references_are_joined_to_the_model_dir() {
        assert_eq!(
            resolve_relative("/models/bai/runtime/bai.model3.json", "bai.moc3").unwrap(),
            "/models/bai/runtime/bai.moc3"
        );
        assert_eq!(
            resolve_relative(
                "/models/bai/runtime/bai.model3.json",
                "./bai.16384/texture_00_4096.png"
            )
            .unwrap(),
            "/models/bai/runtime/bai.16384/texture_00_4096.png"
        );
    }

    #[test]
    fn parent_traversal_is_rejected() {
        for rel in ["..", "../x", "a/../../b", "a\\..\\b"] {
            assert!(
                validate_reference(rel).is_err(),
                "必须拒绝越界引用：{rel:?}"
            );
        }
    }

    #[test]
    fn absolute_paths_and_schemes_are_rejected() {
        for rel in [
            "/etc/passwd",
            "\\\\server\\share\\x",
            "C:\\Windows\\x",
            "https://evil.example/x",
            "file:///etc/passwd",
            "data:text/plain,x",
        ] {
            assert!(
                validate_reference(rel).is_err(),
                "必须拒绝绝对/scheme 引用：{rel:?}"
            );
        }
    }

    #[test]
    fn empty_and_nul_references_are_rejected() {
        assert!(validate_reference("").is_err());
        assert!(validate_reference("./").is_err());
        assert!(validate_reference("a\0b").is_err());
    }
}
