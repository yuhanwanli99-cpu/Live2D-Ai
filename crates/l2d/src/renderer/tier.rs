//! 离屏渲染 target 的离散档位（纹理分辨率上限）。
//!
//! 离屏渲染 target 按 [`RenderTier::side`] 设定；native 路径把
//! `max_texture_dimension_2d` 按档位构造以支持高档；wasm 路径受浏览器上限约束，
//! 用 `tier.side()` 作为目标侧边上钳，实际取 `min(tier.side(), canvas_pixel_size)`。
//!
//! 关联 ADR：`docs/architecture/renderer-texture-tier-adr.md`（§3 接口冻结）。
//!
//! # 2026-10-09：档位还要改变**实际绘制分辨率**，不只是上限
//!
//! 原语义里 `side()` 只是**上钳**，而普通窗口的物理画布远小于 4096
//!（约 1000–2000 px）⇒ 三档钳完一模一样，界面上看起来「失效」。
//! 现在多一条 [`RenderTier::render_scale`]：**渲染比例**乘在
//! 「CSS 尺寸 × DPR」之后，画布位图因此比显示盒更大，浏览器下采样回 CSS 盒
//! （超采样）——三档在这台屏幕上肉眼可分，且**不会**为字面 16384 分配一张
//! 1 GiB 纹理（分配量 = 显示盒 × 比例，`side()` 仍作最后一道上钳）。

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

    /// **渲染比例**（2026-10-09）：画布位图 = CSS 尺寸 × DPR 钳制 × 本比例。
    ///
    /// 三条取值刻意是「1.0 / 1.5 / 2.0」而不是「4096 / 8192 / 16384」：
    /// 后者要求为档位本身分配一张对应边长的纹理（16384² RGBA8 ≈ 1 GiB），
    /// 而这是**本地优先的桌宠**，普通窗口根本用不到。超采样比例足够让三档
    /// 在本机屏幕上肉眼可分（画布位图 1.0× / 1.5× / 2.0×，浏览器下采样回
    /// CSS 显示盒），分配量随窗口大小走，不随档位标称值走。
    ///
    /// 与 [`Self::side`] 的关系：先乘比例，再按 `side()` 对最长边做保比上钳
    ///（大窗口 + 高档位时的显存兜底）。
    pub fn render_scale(self) -> f64 {
        match self {
            Self::Tier4096 => 1.0,
            Self::Tier8192 => 1.5,
            Self::Tier16384 => 2.0,
        }
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

    /// 三档的渲染比例必须**两两不同**（否则界面上三档看起来一样 = 失效）。
    #[test]
    fn render_scale_is_strictly_increasing() {
        assert_eq!(RenderTier::Tier4096.render_scale(), 1.0);
        assert_eq!(RenderTier::Tier8192.render_scale(), 1.5);
        assert_eq!(RenderTier::Tier16384.render_scale(), 2.0);
        assert!(
            RenderTier::Tier4096.render_scale() < RenderTier::Tier8192.render_scale()
                && RenderTier::Tier8192.render_scale() < RenderTier::Tier16384.render_scale(),
            "比例必须严格递增，否则普通窗口上三档画出来一样"
        );
        // 比例是「小倍数」，不是档位标称边长——16384 档不是 16384 倍。
        assert!(RenderTier::Tier16384.render_scale() <= 2.0);
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
