//! 气泡的跨平台几何：DPI 缩放、测量结果收边、双泡堆叠与角色 bbox 定位。
//! Windows 的文字宽高由 GDI 量出后喂给这里，Linux 单测可直接喂测量值。

/// 气泡矩形（帧内坐标；绘制仍逐像素裁剪）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BubbleRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairLayout {
    pub ai: Option<BubbleRect>,
    pub user: Option<BubbleRect>,
}

/// 96 DPI 的设计值统一按当前窗 DPI 缩放；0 是 API 失败时的安全回退。
#[must_use]
pub fn dpi_scale(base: i32, dpi: u32) -> i32 {
    let dpi = if dpi == 0 { 96 } else { dpi };
    ((i64::from(base) * i64::from(dpi) + 48) / 96).clamp(1, i64::from(i32::MAX)) as i32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DpiMetrics {
    pub font_px: i32,
    pub line_px: i32,
    pub pad: i32,
    pub radius: i32,
    pub margin: i32,
    pub gap: i32,
    pub max_width: i32,
    pub max_height: i32,
}

impl DpiMetrics {
    #[must_use]
    pub fn new(dpi: u32) -> Self {
        Self {
            font_px: dpi_scale(16, dpi),
            line_px: dpi_scale(19, dpi),
            pad: dpi_scale(12, dpi),
            radius: dpi_scale(14, dpi),
            margin: dpi_scale(10, dpi),
            gap: dpi_scale(8, dpi),
            max_width: dpi_scale(420, dpi),
            max_height: dpi_scale(320, dpi),
        }
    }

    #[must_use]
    pub fn min_readable_height(self) -> i32 {
        self.line_px + 2 * self.pad
    }

    #[must_use]
    pub fn max_bubble_width(self, frame_width: i32) -> i32 {
        let margin = effective_margin(frame_width, self.margin);
        (frame_width - 2 * margin).max(1).min(self.max_width)
    }

    #[must_use]
    pub fn max_bubble_height(self, frame_height: i32) -> i32 {
        let margin = effective_margin(frame_height, self.margin);
        (frame_height - 2 * margin).max(1).min(self.max_height)
    }
}

/// GDI 暂时失败时的可见降级；正常路径不使用估算，而是使用同一字体的实测值。
#[must_use]
pub fn fallback_size(text: &str, max_w: i32, max_h: i32, metrics: DpiMetrics) -> (i32, i32) {
    let content_w = (max_w - 2 * metrics.pad).max(1);
    let chars = text.chars().count().max(1) as i64;
    let natural_w = chars.saturating_mul(i64::from(metrics.font_px));
    let text_w = natural_w.clamp(1, i64::from(content_w)) as i32;
    let lines = ((natural_w + i64::from(content_w) - 1) / i64::from(content_w)).max(1);
    let text_h = lines.saturating_mul(i64::from(metrics.line_px));
    (
        (text_w + 2 * metrics.pad).min(max_w).max(1),
        (text_h + i64::from(2 * metrics.pad))
            .min(i64::from(max_h))
            .max(1) as i32,
    )
}

/// 极小帧不能继续套固定 margin；最多取边长的四分之一，给内容留下空间。
fn effective_margin(frame_size: i32, margin: i32) -> i32 {
    if frame_size <= 0 {
        0
    } else {
        margin.max(0).min(frame_size / 4)
    }
}

/// 较高者先降到另一泡的高度；仍有 overflow 时双方均担，且不破各自可读下限。
fn shrink_to_fit(ai: &mut i32, user: &mut i32, available: i32, gap: i32, min_readable: i32) {
    let ai_floor = (*ai).min(min_readable).max(1);
    let user_floor = (*user).min(min_readable).max(1);
    let mut overflow = (*ai + *user + gap - available).max(0);
    if overflow == 0 {
        return;
    }
    if *ai > *user {
        let amount = (*ai - *user).min(overflow).min((*ai - ai_floor).max(0));
        *ai -= amount;
        overflow -= amount;
    } else if *user > *ai {
        let amount = (*user - *ai).min(overflow).min((*user - user_floor).max(0));
        *user -= amount;
        overflow -= amount;
    }
    if overflow == 0 {
        return;
    }

    // 奇数 overflow 允许 AI 多承担 1px；任一泡到下限后，剩余全部转给另一泡。
    let ai_room = (*ai - ai_floor).max(0);
    let user_room = (*user - user_floor).max(0);
    let mut ai_take = ((overflow + 1) / 2).min(ai_room);
    let mut user_take = (overflow - ai_take).min(user_room);
    let mut remaining = overflow - ai_take - user_take;
    let extra_ai = remaining.min(ai_room - ai_take);
    ai_take += extra_ai;
    remaining -= extra_ai;
    let extra_user = remaining.min(user_room - user_take);
    user_take += extra_user;
    remaining -= extra_user;
    *ai -= ai_take;
    *user -= user_take;
    debug_assert_eq!(remaining, 0, "调用前必须验证两个可读下限能共存");
    debug_assert!(*ai >= ai_floor && *user >= user_floor);
    debug_assert_eq!(*ai + *user + gap, available);
}

/// 双泡能可读共存才公平压缩；否则明确保留 AI、隐藏用户泡。
/// 若连 AI 自己的可读下限也放不下，则不为凑可见而返回 1px 假矩形。
fn allocate_heights(
    ai: Option<i32>,
    user: Option<i32>,
    available: i32,
    gap: i32,
    min_readable: i32,
) -> (Option<i32>, Option<i32>) {
    fn single(height: i32, available: i32, min_readable: i32) -> Option<i32> {
        let minimum = height.min(min_readable).max(1);
        (available >= minimum).then(|| height.min(available))
    }

    match (ai.filter(|h| *h > 0), user.filter(|h| *h > 0)) {
        (Some(ai), Some(user)) => {
            let ai_min = ai.min(min_readable).max(1);
            let user_min = user.min(min_readable).max(1);
            if available >= ai_min + user_min + gap {
                let mut ai_h = ai.min(available);
                let mut user_h = user.min(available);
                shrink_to_fit(&mut ai_h, &mut user_h, available, gap, min_readable);
                (Some(ai_h), Some(user_h))
            } else {
                (single(ai, available, min_readable), None)
            }
        }
        (Some(ai), None) => (single(ai, available, min_readable), None),
        (None, Some(user)) => (None, single(user, available, min_readable)),
        (None, None) => (None, None),
    }
}

fn centered_x(anchor: i32, width: i32, frame_width: i32, margin: i32) -> i32 {
    let preferred = (anchor - width / 2).clamp(margin, (frame_width - margin - width).max(margin));
    preferred.clamp(0, (frame_width - width).max(0))
}

/// 用户泡在上、AI 泡在下；每块保留自己的实测宽高，空间不足才压缩高。
/// bbox 每帧传入，角色移动或文本换句都会重新定位，不缓存首次结果。
#[must_use]
pub fn place_pair(
    frame_width: i32,
    frame_height: i32,
    bbox: Option<(u32, u32, u32, u32)>,
    ai_size: Option<(i32, i32)>,
    user_size: Option<(i32, i32)>,
    metrics: DpiMetrics,
) -> PairLayout {
    let empty = PairLayout {
        ai: None,
        user: None,
    };
    if frame_width <= 0 || frame_height <= 0 || (ai_size.is_none() && user_size.is_none()) {
        return empty;
    }
    let margin_x = effective_margin(frame_width, metrics.margin);
    let margin_y = effective_margin(frame_height, metrics.margin);
    let max_w = frame_width - 2 * margin_x;
    let available_h = frame_height - 2 * margin_y;
    if max_w <= 0 || available_h <= 0 {
        return empty;
    }
    let ai_w = ai_size.map(|size| size.0.clamp(1, max_w)).unwrap_or(0);
    let user_w = user_size.map(|size| size.0.clamp(1, max_w)).unwrap_or(0);
    let (ai_h, user_h) = allocate_heights(
        ai_size.map(|size| size.1),
        user_size.map(|size| size.1),
        available_h,
        metrics.gap,
        metrics.min_readable_height(),
    );
    if ai_h.is_none() && user_h.is_none() {
        return empty;
    }
    let gap = if ai_h.is_some() && user_h.is_some() {
        metrics.gap
    } else {
        0
    };
    let total_h = ai_h.unwrap_or(0) + user_h.unwrap_or(0) + gap;
    let bbox = bbox.map(|(x0, y0, x1, y1)| {
        (
            x0.min(i32::MAX as u32) as i32,
            y0.min(i32::MAX as u32) as i32,
            x1.min(i32::MAX as u32) as i32,
            y1.min(i32::MAX as u32) as i32,
        )
    });
    let anchor = bbox.map_or(frame_width / 2, |(x0, _, x1, _)| {
        ((i64::from(x0) + i64::from(x1)) / 2) as i32
    });
    let desired_y = if let Some((_, y0, _, y1)) = bbox {
        let above = y0 - margin_y - total_h;
        let below = y1.saturating_add(margin_y);
        if above >= margin_y {
            above
        } else if below + total_h <= frame_height - margin_y {
            below
        } else {
            margin_y
        }
    } else {
        margin_y
    };
    let group_y = desired_y
        .clamp(margin_y, (frame_height - margin_y - total_h).max(margin_y))
        .clamp(0, (frame_height - total_h).max(0));
    let ai = ai_h.map(|h| {
        let y = group_y + user_h.unwrap_or(0) + gap;
        BubbleRect {
            x: centered_x(anchor, ai_w, frame_width, margin_x),
            y,
            w: ai_w,
            h,
        }
    });
    let user = user_h.map(|h| BubbleRect {
        x: centered_x(anchor, user_w, frame_width, margin_x),
        y: group_y,
        w: user_w,
        h,
    });
    PairLayout { ai, user }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_150_scales_font_padding_radius_and_bounds() {
        let normal = DpiMetrics::new(96);
        let scaled = DpiMetrics::new(144);
        assert_eq!((normal.font_px, scaled.font_px), (16, 24));
        assert_eq!((normal.pad, scaled.pad), (12, 18));
        assert_eq!((normal.radius, scaled.radius), (14, 21));
        assert_eq!((normal.max_width, scaled.max_width), (420, 630));
        assert_eq!(DpiMetrics::new(0), normal);
    }

    #[test]
    fn short_and_long_measured_text_get_different_rects_without_overlap() {
        let metrics = DpiMetrics::new(96);
        let placed = place_pair(
            800,
            900,
            Some((300, 500, 500, 850)),
            Some((120, 40)),
            Some((300, 130)),
            metrics,
        );
        let ai = placed.ai.unwrap();
        let user = placed.user.unwrap();
        assert_eq!((ai.w, ai.h), (120, 40));
        assert_eq!((user.w, user.h), (300, 130));
        assert_eq!(user.y + user.h + metrics.gap, ai.y);
    }

    #[test]
    fn placement_reacts_to_bbox_and_new_measurement() {
        let metrics = DpiMetrics::new(96);
        let first = place_pair(
            800,
            900,
            Some((100, 500, 300, 850)),
            Some((120, 40)),
            None,
            metrics,
        )
        .ai
        .unwrap();
        let moved = place_pair(
            800,
            900,
            Some((500, 400, 700, 850)),
            Some((280, 90)),
            None,
            metrics,
        )
        .ai
        .unwrap();
        assert_ne!(first.x, moved.x);
        assert_ne!(first.y, moved.y);
        assert_eq!((moved.w, moved.h), (280, 90));
    }

    #[test]
    fn two_long_bubbles_are_height_limited_and_never_overlap() {
        let metrics = DpiMetrics::new(96);
        let placed = place_pair(400, 220, None, Some((380, 180)), Some((380, 160)), metrics);
        let ai = placed.ai.unwrap();
        let user = placed.user.unwrap();
        assert_eq!(user.y + user.h + metrics.gap, ai.y);
        assert!(user.y >= metrics.margin);
        assert!(ai.y + ai.h <= 220 - metrics.margin);
        assert!(ai.h < 180 || user.h < 160);
    }

    #[test]
    fn equal_320px_bubbles_share_overflow_fairly_without_one_pixel_fallback() {
        let metrics = DpiMetrics::new(96);
        let placed = place_pair(400, 240, None, Some((380, 320)), Some((380, 320)), metrics);
        let ai = placed.ai.unwrap();
        let user = placed.user.unwrap();
        // 有效高度 220，gap=8：双方严格各得 (220-8)/2=106，而非 AI=1/user=211。
        assert_eq!((ai.h, user.h), (106, 106));
        assert_eq!(metrics.min_readable_height(), 43);
        assert!(ai.h >= metrics.min_readable_height());
        assert!(user.h >= metrics.min_readable_height());
        assert_eq!(user.y + user.h + metrics.gap, ai.y);
        assert_eq!(ai.y + ai.h, 240 - metrics.margin);
    }

    #[test]
    fn higher_bubble_absorbs_difference_before_any_shared_reduction() {
        let metrics = DpiMetrics::new(96);
        let placed = place_pair(400, 240, None, Some((380, 220)), Some((380, 100)), metrics);
        let ai = placed.ai.unwrap();
        let user = placed.user.unwrap();
        // overflow=108 小于高度差 120：较高 AI 独自降 108，用户泡不应先被压缩。
        assert_eq!((ai.h, user.h), (112, 100));
        assert_eq!(user.y + user.h + metrics.gap, ai.y);
    }

    #[test]
    fn insufficient_pair_height_hides_user_and_keeps_readable_ai() {
        let metrics = DpiMetrics::new(96);
        let placed = place_pair(400, 100, None, Some((380, 320)), Some((380, 320)), metrics);
        let ai = placed.ai.expect("空间不足一块时必须优先保留 AI");
        assert_eq!(placed.user, None, "不能返回被压成 1px 的用户泡");
        assert_eq!(ai.h, 80);
        assert!(ai.h >= metrics.min_readable_height());
        assert!(ai.y >= 0 && ai.y + ai.h <= 100);
    }

    #[test]
    fn unreadably_small_ai_is_hidden_instead_of_returning_fake_rect() {
        let metrics = DpiMetrics::new(96);
        let placed = place_pair(400, 50, None, Some((380, 320)), Some((380, 320)), metrics);
        assert_eq!(placed.ai, None, "有效高 30 已容不下 43px 可读 AI 泡");
        assert_eq!(placed.user, None);
    }

    #[test]
    fn tiny_frames_clamp_every_origin_and_edge_inside_the_frame() {
        let metrics = DpiMetrics::new(96);
        for (frame_w, frame_h) in [(1, 1), (2, 3), (5, 4), (12, 8), (20, 9)] {
            let placed = place_pair(
                frame_w,
                frame_h,
                Some((100, 100, 200, 200)),
                Some((50, 1)),
                None,
                metrics,
            );
            let rect = placed.ai.expect("1px 原始短泡可用于验证极小帧几何");
            assert!(rect.w > 0 && rect.h > 0);
            assert!(rect.x >= 0 && rect.y >= 0, "origin 越界: {rect:?}");
            assert!(
                rect.x <= (frame_w - rect.w).max(0),
                "x 未 clamp 到合法区间: {rect:?}"
            );
            assert!(
                rect.y <= (frame_h - rect.h).max(0),
                "y 未 clamp 到合法区间: {rect:?}"
            );
            assert!(rect.x + rect.w <= frame_w, "右缘越界: {rect:?}");
            assert!(rect.y + rect.h <= frame_h, "下缘越界: {rect:?}");
        }
        let empty = place_pair(0, 10, None, Some((10, 1)), None, metrics);
        assert_eq!((empty.ai, empty.user), (None, None));
    }

    #[test]
    fn fallback_also_distinguishes_short_and_long_text() {
        let metrics = DpiMetrics::new(96);
        let short = fallback_size("短句", 420, 320, metrics);
        let long = fallback_size(&"长".repeat(100), 420, 320, metrics);
        assert!(short.0 < long.0);
        assert!(short.1 < long.1);
        assert!(long.1 <= 320);
    }
}
