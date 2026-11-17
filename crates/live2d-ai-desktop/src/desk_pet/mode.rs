//! 「先进网页，再切桌宠」的模式切换唯一落点。
//!
//! `POST /api/v1/desk-pet/mode` 的 handler 与桌宠窗右键菜单**都调这里**，
//! 保证两条入口做同一件事：`pet` 开窗、`web` 关窗并用系统浏览器打开 `/app/`。
//!
//! 刻意不做：不写 `mods.json`、不 enable 工厂、不调用
//! `POST /api/v1/mods/discovered`（那是启动器「进入」的职责）、不重探测
//! LLM/TTS、不杀任何 Mod 进程、不重启服务、不打开 `GET /`。

#![cfg_attr(not(windows), allow(dead_code))]

/// 两种模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Pet,
    Web,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pet => "pet",
            Self::Web => "web",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pet" => Some(Self::Pet),
            "web" => Some(Self::Web),
            _ => None,
        }
    }
}

/// 一次切换的结果；`ok:false` 时 `reason` 给一句人能看懂的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub ok: bool,
    pub mode: Mode,
    pub reason: Option<String>,
}

/// 纯决策（不碰任何副作用）：非 Windows 的 `pet` 恒为「仅 Windows」。
#[must_use]
pub fn decision(mode: Mode, windows: bool) -> Option<&'static str> {
    if mode == Mode::Pet && !windows {
        Some("仅 Windows")
    } else {
        None
    }
}

/// 执行切换。单元测试（`cfg(test)`）只走纯决策与响应，不真开 GUI / 弹浏览器。
pub fn apply(mode: Mode) -> Outcome {
    if let Some(reason) = decision(mode, cfg!(windows)) {
        return Outcome {
            ok: false,
            mode,
            reason: Some(reason.to_string()),
        };
    }

    #[cfg(all(windows, not(test)))]
    {
        match mode {
            Mode::Pet => match crate::desk_pet::open_pet() {
                Ok(()) => Outcome {
                    ok: true,
                    mode,
                    reason: None,
                },
                Err(e) => Outcome {
                    ok: false,
                    mode,
                    reason: Some(e),
                },
            },
            Mode::Web => {
                let closed = crate::desk_pet::close_pet();
                tracing::info!(
                    target: "desk_pet",
                    closed,
                    port = crate::desk_pet::http_port(),
                    "回到网页：关闭宠物窗并打开 /app/"
                );
                // 窗本来就不在也照样打开页面（不重跑「进入」、不打开 GET /）。
                crate::browser::open_app(crate::desk_pet::http_port());
                Outcome {
                    ok: true,
                    mode,
                    reason: None,
                }
            }
        }
    }
    #[cfg(any(not(windows), test))]
    {
        Outcome {
            ok: true,
            mode,
            reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parse_and_round_trip() {
        assert_eq!(Mode::parse("pet"), Some(Mode::Pet));
        assert_eq!(Mode::parse("web"), Some(Mode::Web));
        assert_eq!(Mode::parse("desktop"), None);
        assert_eq!(Mode::Pet.as_str(), "pet");
        assert_eq!(Mode::Web.as_str(), "web");
    }

    #[test]
    fn pet_on_non_windows_is_refused_with_a_reason() {
        assert_eq!(decision(Mode::Pet, false), Some("仅 Windows"));
        assert_eq!(decision(Mode::Pet, true), None);
        assert_eq!(decision(Mode::Web, false), None);
    }

    /// 两条入口共用同一个函数：`web` 与右键菜单拿到的结果必须一致。
    #[test]
    fn both_modes_produce_a_well_formed_outcome() {
        for mode in [Mode::Pet, Mode::Web] {
            let outcome = apply(mode);
            assert_eq!(outcome.mode, mode);
            assert!(outcome.ok, "单元测试不真开窗，应返回 ok:true");
        }
    }
}
