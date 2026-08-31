//! Mod 描述符与 id（静态身份，编译期已知）。

/// Mod 稳定唯一 id（编译期静态字符串）。
pub type ModId = &'static str;

/// Mod 描述符（静态元数据）。
///
/// `api_version` 表示该 Mod 声明的 Mod 系统 API 版本；host 通过它判断是否兼容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDescriptor {
    /// 唯一 id（snake_case；与 registry 配置命名空间对应）。
    pub id: ModId,
    /// 显示名。
    pub name: &'static str,
    /// 语义版本。
    pub version: &'static str,
    /// 声明的 Mod 系统 API 版本（应与 `crate::MOD_API_VERSION` 对齐）。
    pub api_version: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_fields() {
        let d = ModDescriptor {
            id: "external-input",
            name: "外部事件接入",
            version: "0.1.0",
            api_version: 1,
        };
        assert_eq!(d.id, "external-input");
        assert_eq!(d.api_version, 1);
    }
}
