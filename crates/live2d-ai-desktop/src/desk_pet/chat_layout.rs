//! 简易会话框的跨平台放置：依据角色两侧工作区选左/右，再把整窗 clamp 回工作区。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub rect: Rect,
    pub side: Side,
}

fn clamp_origin(value: i32, low: i32, size: i32, high: i32) -> i32 {
    value.clamp(low, (high - size).max(low))
}

/// `pet = None` 时放到工作区左上安全位；宽高先于位置收进工作区。
#[must_use]
pub fn place(
    work: Rect,
    pet: Option<Rect>,
    width: i32,
    height: i32,
    gap: i32,
    top_offset: i32,
) -> Placement {
    let work_w = (work.right - work.left).max(1);
    let work_h = (work.bottom - work.top).max(1);
    let width = width.clamp(1, work_w);
    let height = height.clamp(1, work_h);
    let Some(pet) = pet else {
        let left = clamp_origin(work.left + gap, work.left, width, work.right);
        let top = clamp_origin(work.top + gap, work.top, height, work.bottom);
        return Placement {
            rect: Rect {
                left,
                top,
                right: left + width,
                bottom: top + height,
            },
            side: Side::Right,
        };
    };
    let right_space = work.right - pet.right - gap;
    let left_space = pet.left - work.left - gap;
    let right_fits = right_space >= width;
    let left_fits = left_space >= width;
    let side = match (left_fits, right_fits) {
        (true, false) => Side::Left,
        (false, true) => Side::Right,
        _ => {
            let pet_center_x = i64::from(pet.left) + i64::from(pet.right);
            let work_center_x = i64::from(work.left) + i64::from(work.right);
            if pet_center_x >= work_center_x {
                Side::Left
            } else {
                Side::Right
            }
        }
    };
    let desired_x = match side {
        Side::Left => pet.left - gap - width,
        Side::Right => pet.right + gap,
    };
    let left = clamp_origin(desired_x, work.left, width, work.right);
    let top = clamp_origin(pet.top + top_offset, work.top, height, work.bottom);
    Placement {
        rect: Rect {
            left,
            top,
            right: left + width,
            bottom: top + height,
        },
        side,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };

    #[test]
    fn bottom_right_pet_uses_left_instead_of_leaving_nineteen_pixels() {
        let pet = Rect {
            left: 1353,
            top: 203,
            right: 1893,
            bottom: 1013,
        };
        let placed = place(WORK, Some(pet), 360, 260, 8, 120);
        assert_eq!(placed.side, Side::Left);
        assert_eq!(placed.rect.left, 985);
        assert_eq!(placed.rect.right, 1345);
        assert!(placed.rect.right <= pet.left);
    }

    #[test]
    fn left_pet_uses_right_and_vertical_position_is_clamped() {
        let pet = Rect {
            left: 24,
            top: 900,
            right: 364,
            bottom: 1030,
        };
        let placed = place(WORK, Some(pet), 360, 260, 8, 120);
        assert_eq!(placed.side, Side::Right);
        assert_eq!(placed.rect.left, 372);
        assert_eq!(placed.rect.bottom, WORK.bottom);
    }

    #[test]
    fn narrow_or_offset_work_area_clamps_the_whole_window() {
        let work = Rect {
            left: -500,
            top: 20,
            right: -220,
            bottom: 200,
        };
        let pet = Rect {
            left: -280,
            top: 100,
            right: -225,
            bottom: 190,
        };
        let placed = place(work, Some(pet), 360, 260, 8, 120);
        assert_eq!(placed.rect.left, work.left);
        assert_eq!(placed.rect.right, work.right);
        assert_eq!(placed.rect.top, work.top);
        assert_eq!(placed.rect.bottom, work.bottom);
    }

    #[test]
    fn missing_pet_still_stays_inside_work_area() {
        let placed = place(WORK, None, 360, 260, 8, 120);
        assert_eq!(placed.rect.left, 8);
        assert_eq!(placed.rect.top, 8);
        assert_eq!(placed.rect.right, 368);
        assert_eq!(placed.rect.bottom, 268);
    }
}
