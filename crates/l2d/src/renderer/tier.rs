//! 离屏渲染 target 的离散档位（**画布最长边**目标分辨率）。
//!
//! # 2026-10-10：档位 = 画布**最长边**（不是渲染比例，也不是上钳）
//!
//! 前三档（4K / 8K / 16K）的最长边就是 [`RenderTier::side`]（4096 / 8192 /
//! 16384），另一边按舞台宽高比跟着走（**不撑成正方形**）。native 路径把
//! `max_texture_dimension_2d` 按档位构造以支持高档。
//!
//! 2026-10-09 曾用一条 `render_scale()`（1.0 / 1.5 / 2.0）乘在「CSS × DPR」上，
//! 让普通窗口也能看出三档差别；**该比例已于 2026-10-10 删除**——它让「档位」
//! 名不副实（8K 实际只画到显示盒的 1.5 倍）。现在档位说的就是最长边。
//!
//! wasm 路径受浏览器/适配器上限约束：实际可达的边长由 `adapter
//! max_texture_dimension_2d` 决定，申请不到就 16384→8192→4096 逐级回落
//! （见 `l2d-wasm-demo` 的 `web/surface/gpu.rs`）。
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

    /// 在 adapter `max_texture_dimension_2d` 约束下可用的**最高档位**。
    ///
    /// 从本档开始逐级向下找（本档 → 8192 → 4096）；`max_dim` 连 4096 都放不下
    /// 时返回 `None`（wasm 侧据此退回「CSS × DPR」兜底）。**不向上抬**——
    /// 用户选 8K 就最多给 8K，即使 adapter 能到 16384。
    pub fn highest_within(self, max_dim: u32) -> Option<Self> {
        [self, Self::Tier8192, Self::Tier4096]
            .into_iter()
            .find(|t| t.side() <= max_dim)
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

    /// 三档的最长边必须**两两不同**（否则界面上三档看起来一样 = 失效）。
    #[test]
    fn sides_are_strictly_increasing() {
        assert!(
            RenderTier::Tier4096.side() < RenderTier::Tier8192.side()
                && RenderTier::Tier8192.side() < RenderTier::Tier16384.side(),
            "最长边必须严格递增，否则三档画出来一样"
        );
        assert_eq!(RenderTier::Tier16384.side(), 16384);
    }

    #[test]
    fn from_side_parses_known_and_rejects_unknown() {
        assert_eq!(RenderTier::from_side(4096), Some(RenderTier::Tier4096));
        assert_eq!(RenderTier::from_side(8192), Some(RenderTier::Tier8192));
        assert_eq!(RenderTier::from_side(16384), Some(RenderTier::Tier16384));
        assert_eq!(RenderTier::from_side(2048), None);
        assert_eq!(RenderTier::from_side(0), None);
    }

    /// adapter 上限回落：够就原样、不够就逐级降、连 4096 都没有就 `None`；
    /// **绝不向上抬**（选 8K 不会被 adapter 的 16K 抬成 16K）。
    #[test]
    fn highest_within_falls_back_and_never_upgrades() {
        assert_eq!(
            RenderTier::Tier16384.highest_within(16384),
            Some(RenderTier::Tier16384)
        );
        assert_eq!(
            RenderTier::Tier16384.highest_within(8192),
            Some(RenderTier::Tier8192),
            "16K 申请不到应降到 8K"
        );
        assert_eq!(
            RenderTier::Tier16384.highest_within(4096),
            Some(RenderTier::Tier4096)
        );
        assert_eq!(RenderTier::Tier16384.highest_within(2048), None);
        // 选 8K：即使 adapter 报 16K，也停在 8K（不向上抬）。
        assert_eq!(
            RenderTier::Tier8192.highest_within(16384),
            Some(RenderTier::Tier8192)
        );
        assert_eq!(
            RenderTier::Tier4096.highest_within(16384),
            Some(RenderTier::Tier4096)
        );
    }
}
