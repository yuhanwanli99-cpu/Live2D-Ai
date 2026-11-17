//! 宠物窗运行中可改的配置（右键菜单写、帧循环读）。
//!
//! 只放透明度、置顶和原来的命中/窗高。改完记进 `mods.json` 的
//! `mods.desk-pet.config`，下次开窗还在。整窗穿透不放进菜单：打开之后
//! 右键点不到角色，只能靠网页上的按钮回来。

use std::sync::Mutex;

use super::config::{PetConfig, OPACITY_MAX, OPACITY_MIN};

static LIVE: Mutex<PetConfig> = Mutex::new(PetConfig {
    hit: super::config::HIT_DEFAULT,
    opacity: super::config::OPACITY_DEFAULT,
    always_on_top: super::config::ALWAYS_ON_TOP_DEFAULT,
    height_ratio: super::config::HEIGHT_RATIO_DEFAULT,
});

fn with_live<R>(f: impl FnOnce(&mut PetConfig) -> R) -> R {
    match LIVE.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

pub fn publish(config: PetConfig) {
    with_live(|slot| *slot = config);
}

pub fn get() -> PetConfig {
    with_live(|slot| *slot)
}

pub fn opacity() -> f32 {
    get().opacity
}

/// 透明度加减一档（0.1）。已经到头就停在边界。
pub fn nudge_opacity(delta: f32) -> PetConfig {
    with_live(|slot| {
        let next = (slot.opacity + delta).clamp(OPACITY_MIN, OPACITY_MAX);
        // 浮点台阶对齐到 0.1，避免 0.30000004 写进配置。
        slot.opacity = (next * 10.0).round() / 10.0;
        slot.opacity = slot.opacity.clamp(OPACITY_MIN, OPACITY_MAX);
        *slot
    })
}

pub fn set_always_on_top(on: bool) -> PetConfig {
    with_live(|slot| {
        slot.always_on_top = on;
        *slot
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_steps_stay_inside_the_contract() {
        publish(PetConfig {
            opacity: 1.0,
            ..PetConfig::default()
        });
        assert!((nudge_opacity(-0.1).opacity - 0.9).abs() < 0.001);
        publish(PetConfig {
            opacity: OPACITY_MIN,
            ..PetConfig::default()
        });
        assert_eq!(nudge_opacity(-0.1).opacity, OPACITY_MIN);
        publish(PetConfig {
            opacity: 0.95,
            ..PetConfig::default()
        });
        let topped = nudge_opacity(0.1).opacity;
        assert!(topped <= OPACITY_MAX);
    }
}
