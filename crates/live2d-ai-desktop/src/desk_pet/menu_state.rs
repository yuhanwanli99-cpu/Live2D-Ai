//! 右键菜单的状态与命令表（2026-11-17 纠正版，**平台无关**）。
//!
//! 六组菜单的「谁打勾 / 哪一项对应哪个动作」全在这里，Linux 单测能真跑；
//! 真正建 Win32 菜单与做副作用在 `menu.rs`（只 Windows 编译）。
//!
//! 状态只住在**宠物窗进程的内存**里：置顶与当前皮肤由 `menu.rs` 另外写进
//! `mods.desk-pet.config`，其余显示/语音/会话框开关**不写任何配置文件**
//! （尤其不写 `live2d-ai.toml`）。缺省：语音输入 = 关闭。

use std::sync::Mutex;

/// 语音输入三档，同一时间只有一档打勾。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceMode {
    /// 不录音（**缺省**）。
    #[default]
    Off,
    /// 按住 ≥300 ms 且移动 ≤8 px 才开麦。
    Hold,
    /// 宠物窗在、且自己没在播时持续听；一句结束就识别并 `say`。
    Always,
}

impl VoiceMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "关闭",
            Self::Hold => "长按检测",
            Self::Always => "常开",
        }
    }

    pub fn id(self) -> usize {
        match self {
            Self::Off => ID_VOICE_OFF,
            Self::Hold => ID_VOICE_HOLD,
            Self::Always => ID_VOICE_ALWAYS,
        }
    }
}

/// 六组菜单的所有命令。皮肤子项带索引（列表由 `skin.rs` 运行时给出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 对话框总开关；关掉时两个子项都不画。
    DialogToggle,
    /// 角色旁是否画「用户刚说的那句」。
    UserToggle,
    /// 角色旁是否画「已经交给合成的那一句」（现有气泡）。
    AiToggle,
    TopmostToggle,
    BackToWeb,
    Voice(VoiceMode),
    Skin(usize),
    ChatToggle,
    ChatTopmost(bool),
}

// ── 命令 ID（TPM_RETURNCMD 直接返回这个数；皮肤段从 100 起） ──
pub const ID_DIALOG: usize = 1;
pub const ID_USER: usize = 2;
pub const ID_AI: usize = 3;
pub const ID_TOPMOST: usize = 10;
pub const ID_BACK_TO_WEB: usize = 11;
pub const ID_VOICE_OFF: usize = 20;
pub const ID_VOICE_HOLD: usize = 21;
pub const ID_VOICE_ALWAYS: usize = 22;
pub const ID_CHAT_TOGGLE: usize = 30;
pub const ID_CHAT_TOPMOST: usize = 31;
pub const ID_CHAT_NOTOPMOST: usize = 32;
pub const ID_SKIN_BASE: usize = 100;
/// 皮肤段的上界（命令空间有界，越界 ID 一律不认）。
pub const ID_SKIN_MAX: usize = ID_SKIN_BASE + 64;

impl Command {
    #[must_use]
    pub fn from_id(id: usize) -> Option<Self> {
        Some(match id {
            ID_DIALOG => Self::DialogToggle,
            ID_USER => Self::UserToggle,
            ID_AI => Self::AiToggle,
            ID_TOPMOST => Self::TopmostToggle,
            ID_BACK_TO_WEB => Self::BackToWeb,
            ID_VOICE_OFF => Self::Voice(VoiceMode::Off),
            ID_VOICE_HOLD => Self::Voice(VoiceMode::Hold),
            ID_VOICE_ALWAYS => Self::Voice(VoiceMode::Always),
            ID_CHAT_TOGGLE => Self::ChatToggle,
            ID_CHAT_TOPMOST => Self::ChatTopmost(true),
            ID_CHAT_NOTOPMOST => Self::ChatTopmost(false),
            n if (ID_SKIN_BASE..ID_SKIN_MAX).contains(&n) => Self::Skin(n - ID_SKIN_BASE),
            _ => return None,
        })
    }
}

/// 内存态。缺省 = 对话框两路都画、语音关闭、会话框关且不置顶。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuState {
    pub dialog_on: bool,
    pub show_user: bool,
    pub show_ai: bool,
    pub voice: VoiceMode,
    pub chat_open: bool,
    pub chat_topmost: bool,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            dialog_on: true,
            show_user: true,
            show_ai: true,
            voice: VoiceMode::Off,
            chat_open: false,
            chat_topmost: false,
        }
    }
}

static STATE: Mutex<MenuState> = Mutex::new(MenuState {
    dialog_on: true,
    show_user: true,
    show_ai: true,
    voice: VoiceMode::Off,
    chat_open: false,
    chat_topmost: false,
});

fn with_state<R>(f: impl FnOnce(&mut MenuState) -> R) -> R {
    match STATE.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

pub fn get() -> MenuState {
    with_state(|s| s.clone())
}

/// 单测之间把全局态拨回缺省（进程内菜单状态本来不需要外部重置）。
#[cfg(test)]
pub fn reset() {
    with_state(|s| *s = MenuState::default());
}

/// 简易会话框被用户点 X 关掉时同步菜单状态（置顶选择保留在内存里）。
pub fn close_chat() {
    with_state(|s| s.chat_open = false);
}

/// 只改内存态；副作用（置顶 / 皮肤 / 关窗 / 会话框）由 `menu.rs` 做。
pub fn apply(command: &Command) {
    with_state(|s| match command {
        Command::DialogToggle => s.dialog_on = !s.dialog_on,
        Command::UserToggle => s.show_user = !s.show_user,
        Command::AiToggle => s.show_ai = !s.show_ai,
        Command::Voice(mode) => s.voice = *mode,
        Command::ChatToggle => s.chat_open = !s.chat_open,
        Command::ChatTopmost(on) => s.chat_topmost = *on,
        // 置顶的实际状态在 live.rs / config 里，皮肤在 skin.rs 里。
        Command::TopmostToggle | Command::BackToWeb | Command::Skin(_) => {}
    });
}

/// 用户句与 AI 句是否该画：总开关关掉时两个子项都不画。
#[must_use]
pub fn bubble_visibility(state: &MenuState) -> (bool, bool) {
    (
        state.dialog_on && state.show_user,
        state.dialog_on && state.show_ai,
    )
}

/// 菜单的一条子项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub label: String,
    pub id: usize,
    pub checked: bool,
    /// 简易会话框的置顶子项只在框开着时可选。
    pub enabled: bool,
}

/// 菜单的一组。有 `children` 时 Win32 侧建成 `MF_POPUP` 子菜单；
/// 没有时 `command` 就是这一项自己的命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub label: String,
    pub command: Option<usize>,
    pub checked: bool,
    pub children: Vec<Entry>,
}

fn entry(label: &str, id: usize, checked: bool, enabled: bool) -> Entry {
    Entry {
        label: label.to_string(),
        id,
        checked,
        enabled,
    }
}

/// 六组菜单的纯数据构造。`skins` 是运行时读出来的已安装模型名，
/// `active_skin` 是其中打勾的那一项索引；`topmost` 来自 `live.rs`。
#[must_use]
pub fn build(state: &MenuState, topmost: bool, skins: &[String], active_skin: usize) -> Vec<Group> {
    vec![
        // 1) 对话框显示 / 隐藏。父项是 MF_POPUP（Win32 的弹出项本身不能
        //    再带命令），所以「显示 / 隐藏」放在子菜单第一行，两个子开关随后。
        Group {
            label: "对话框显示 / 隐藏".to_string(),
            command: None,
            checked: state.dialog_on,
            children: vec![
                entry(
                    if state.dialog_on {
                        "隐藏全部"
                    } else {
                        "显示"
                    },
                    ID_DIALOG,
                    state.dialog_on,
                    true,
                ),
                entry("输入消息", ID_USER, state.show_user, state.dialog_on),
                entry("AI 回复", ID_AI, state.show_ai, state.dialog_on),
            ],
        },
        // 2) 置顶 / 取消置顶（单项，勾在当前态上）。
        Group {
            label: if topmost { "取消置顶" } else { "置顶" }.to_string(),
            command: Some(ID_TOPMOST),
            checked: topmost,
            children: Vec::new(),
        },
        // 3) 回到 web 版本。
        Group {
            label: "回到 web 版本".to_string(),
            command: Some(ID_BACK_TO_WEB),
            checked: false,
            children: Vec::new(),
        },
        // 4) 开启语音输入：三档互斥，缺省关闭。
        Group {
            label: "开启语音输入".to_string(),
            command: None,
            checked: false,
            children: [VoiceMode::Always, VoiceMode::Hold, VoiceMode::Off]
                .iter()
                .map(|mode| entry(mode.label(), mode.id(), state.voice == *mode, true))
                .collect(),
        },
        // 5) 切换皮肤：一项一个已安装模型。
        Group {
            label: "切换皮肤".to_string(),
            command: None,
            checked: false,
            children: skins
                .iter()
                .take(ID_SKIN_MAX - ID_SKIN_BASE)
                .enumerate()
                .map(|(i, name)| entry(name, ID_SKIN_BASE + i, i == active_skin, true))
                .collect(),
        },
        // 6) 简易会话框：开 / 关 + 独立置顶。
        Group {
            label: "简易会话框".to_string(),
            command: None,
            checked: state.chat_open,
            children: vec![
                entry(
                    if state.chat_open { "关闭" } else { "打开" },
                    ID_CHAT_TOGGLE,
                    state.chat_open,
                    true,
                ),
                entry(
                    "置顶",
                    ID_CHAT_TOPMOST,
                    state.chat_open && state.chat_topmost,
                    state.chat_open,
                ),
                entry(
                    "非置顶",
                    ID_CHAT_NOTOPMOST,
                    state.chat_open && !state.chat_topmost,
                    state.chat_open,
                ),
            ],
        },
    ]
}

#[cfg(test)]
#[path = "menu_state_tests.rs"]
mod tests;
