//! 离屏渲染 target 的离散档位（纹理分辨率上限）。
//!
//! 离屏渲染 target 按 [`RenderTier::side`] 设定；native 路径把
//! `max_texture_dimension_2d` 按档位构造以支持高档；wasm 路径受浏览器上限约束，
//! 用 `tier.side()` 作为目标侧边上钳，实际取 `min(tier.side(), canvas_pixel_size)`。
//!
//! 关联 ADR：`docs/architecture/renderer-texture-tier-adr.md`（§3 接口冻结）。

/// 离屏渲染 target 的离散档位（RGBA8 单张 ≈ 4096:64MB / 8192:256MB / 16384:1GB）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderTier {
    /// 4096×4096（默认；轻量、内存友好）。
    Tier4096 = 4096,
    /// 8192×8192（高档桌面 GPU 常规）。
    Tier8192 = 8192,
    /// 16384×16384（高端/独立 GPU；内存占用大，需真机确认）。
    Tier16384 = 16384,
}

impl RenderTier {
    /// 默认档位（4096）。
    pub const DEFAULT: Self = Self::Tier4096;

    /// 标称侧边像素数。
    pub fn side(self) -> u32 {
        self as u32
    }

    /// native 路径所需的最小 `max_texture_dimension_2d`。
    ///
    /// 档位只有 <=8192 时 wgpu `Limits::default()` 已满足（默认 8192）；档位=16384
    /// 需在 `request_device` 时显式把上限设为 16384。
    pub fn required_max_texture_dimension(self) -> u32 {
        self.side()
    }

    /// 从侧边像素数解析档位（4096/8192/16384）；非法值返回 `None`。
    pub fn from_side(side: u32) -> Option<Self> {
        match side {
            4096 => Some(Self::Tier4096),
            8192 => Some(Self::Tier8192),
            16384 => Some(Self::Tier16384),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_4096() {
        assert_eq!(RenderTier::DEFAULT, RenderTier::Tier4096);
        assert_eq!(RenderTier::DEFAULT.side(), 4096);
    }

    #[test]
    fn side_maps_each_tier() {
        assert_eq!(RenderTier::Tier4096.side(), 4096);
        assert_eq!(RenderTier::Tier8192.side(), 8192);
        assert_eq!(RenderTier::Tier16384.side(), 16384);
    }

    #[test]
    fn required_max_dimension_equals_side() {
        for tier in [
            RenderTier::Tier4096,
            RenderTier::Tier8192,
            RenderTier::Tier16384,
        ] {
            assert_eq!(tier.required_max_texture_dimension(), tier.side());
        }
    }

    #[test]
    fn from_side_parses_known_and_rejects_unknown() {
        assert_eq!(RenderTier::from_side(4096), Some(RenderTier::Tier4096));
        assert_eq!(RenderTier::from_side(8192), Some(RenderTier::Tier8192));
        assert_eq!(RenderTier::from_side(16384), Some(RenderTier::Tier16384));
        assert_eq!(RenderTier::from_side(2048), None);
        assert_eq!(RenderTier::from_side(0), None);
    }
}
