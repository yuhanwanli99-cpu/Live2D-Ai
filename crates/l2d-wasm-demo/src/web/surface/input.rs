//! 舞台输入边界（仅 `target_arch = "wasm32"` 下编译）：父页 bridge 消息与指针/滚轮
//! 写入的 [`BridgeState`]，以及每帧把它应用为可见效果。
//!
//! 应用面：口型（dB 映射 + dt 归一化衰减）→ 缩放/平移（Affine2）→ 舞台底色 / 背景图
//! → 待机生命体征。取值合法性（如舞台底色的 `#RRGGBB`）在 [`normalize_stage_color`] 收口。

use glam::f32::{Affine2, Vec2};
use l2d::renderer::RenderTier;

use super::idle::apply_idle_life;
use super::render::SharedState;
use crate::mouth::{DEFAULT_MOUTH_SENSITIVITY, mouth_open_from_level};

/// 口型音量显示衰减时间常数（ms）。
/// `volume_display *= exp(-dt_ms / TAU_MS)` —— 衰减到 37% 需时 TAU_MS。
///
/// **2026-09-10 由 160 调至 90**：dB 映射后口型幅度已经够大，160ms 的慢衰减会把
/// 快速连续音节糊成一片（嘴一直张着不闭合）。90ms 让每个音节的「开—合」都看得见。
/// 该时间常数与帧率无关：60fps 与 240fps 分别给出 `exp(-16.67/90)≈0.831` 与
/// `exp(-4.17/90)≈0.955`，总衰减速率一致。
const TAU_MS: f64 = 90.0;

/// 把父页给的舞台底色**校验**成可安全写进 CSS 的 `#RRGGBB`。
///
/// # 为什么要校验而不是照抄
///
/// 这个字符串会进 `canvas.style.setProperty("background-color", …)`。
/// 虽然当前唯一的发送方是我们自己的前端，但 `/render` 是一个**独立页面**，
/// 任何能 postMessage 到它的东西都能塞值进来。CSS 值允许
/// `url(...)`、`var(...)` 等构造，照抄等于开了一个注入面。
///
/// 规则刻意收紧到「只认 `#RGB` / `#RRGGBB` 的十六进制」——
/// 舞台底本来就只需要纯色；非法值返回 `None`，于是**回落到 `dark` 的默认色**，
/// 而不是把整条 `sync` 消息拒掉（其余字段照常生效）。
pub(crate) fn normalize_stage_color(raw: &str) -> Option<String> {
    let s = raw.trim();
    let hex = s.strip_prefix('#')?;
    if !(hex.len() == 3 || hex.len() == 6) {
        return None;
    }
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", hex.to_ascii_lowercase()))
}

/// iframe bridge 接收状态（P0-2a）：父页 postMessage → 可见效果。
/// 首帧后由 main.rs 的 message listener 写入；rAF 循环每帧读取产生可见效果。
#[derive(Clone)]
pub(crate) struct BridgeState {
    /// stage-config.scale（滚轮/滑块缩放，0.5..2.0，默认 1.0）。
    /// 应用为 ModelRendererCore 的逻辑变换缩放（以画布中心为锚）。
    pub scale: f32,
    /// 左键拖动累计平移（NDC 单位，Default 0.0）。
    /// 由 pointermove 在拖盘过程中写入，写入前已换算为 NDC。
    pub offset_x: f32,
    pub offset_y: f32,
    /// stage-config.dark（背景暗色，默认 true）
    ///
    /// 只在 [`BridgeState::stage_color`] 为 `None` 时决定背景色——它是
    /// **历史回退路径**，保留是为了让没发 `stageColor` 的旧前端继续能用。
    pub dark: bool,
    /// stage-config.stageColor（**舞台纯色底**，`#RRGGBB` 形式的 CSS 颜色）。
    ///
    /// 2026-09-11 新增（向后兼容：缺省即 `None`，行为与旧版完全一致）。
    /// 为什么不让前端自己画：舞台是 `<iframe>` 平台视图，iframe 内的 canvas
    /// 自带不透明背景色，会**盖住**父页画的任何底色。所以底色必须由渲染面
    /// 自己写。用户裁决「舞台背影全黑/全白即可，中央不要放贴图」——
    /// 四套主题各自的舞台底因此走这条通道下发。
    pub stage_color: Option<String>,
    /// stage-config.lipSync（口型开关，默认 true）
    pub lip_sync: bool,
    /// stage-config.mouthSensitivity（口型灵敏度，默认 1.0）。
    ///
    /// 乘在 [`mouth_open_from_level`] 的 dB 映射结果上；`1.0` 即「实测真实语音
    /// 的 p90 约开到 0.58」的标定值。范围由 `stage-config` 侧 clamp 到 `[0,4]`。
    pub mouth_sensitivity: f32,
    /// stage-config.idleEnabled（空闲随机动作开关，默认 true；false 时跳过
    /// 呼吸/眨眼/微表情等 idle 生命体征层，模型静止）。
    pub idle_enabled: bool,
    /// stage-config.clickEnabled（点击/拖动互动开关，默认 true；false 时
    /// 忽略 pointerdown 拖拽与双击缩放，模型不因交互移动）。
    pub click_enabled: bool,
    /// 渲染档位（ADR §3.3：离屏 target 侧边上限，默认 [`RenderTier::DEFAULT`]）。
    pub tier: RenderTier,
    /// stage-bg.dataUrl（自定义背景图 dataURL；None = 无背景图）。
    /// 应用为 canvas CSS background-image（cover 缩放），与 background-color 共存。
    pub bg_data_url: Option<String>,
    /// 最近 audio-volume（0..1；每 20ms 音频帧更新）
    pub volume: f32,
    /// 衰减后的口型显示值（rAF 每帧 volume_display = volume.max(volume_display*0.85)）
    pub volume_display: f32,
    /// 诊断（P3.1）：收到的消息数 / 应用数 / 最近类型——HUD 显示定位"模型不见"。
    pub msg_recv: u32,
    pub msg_applied: u32,
    pub last_msg: String,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            dark: true,
            stage_color: None,
            lip_sync: true,
            mouth_sensitivity: DEFAULT_MOUTH_SENSITIVITY,
            idle_enabled: true,
            click_enabled: true,
            tier: RenderTier::DEFAULT,
            bg_data_url: None,
            volume: 0.0,
            volume_display: 0.0,
            msg_recv: 0,
            msg_applied: 0,
            last_msg: String::new(),
        }
    }
}

/// P0-2a-3：每帧把 bridge 状态应用为可见效果（口型/缩放/平移/背景/待机体征）。
/// 由 `tick` 在 `core.update` 后、渲染前调用。
///
/// `dt_millis`：真实帧间隔（毫秒），用于 dt 归一化的口型衰减，避免高帧率
///（如 240fps）下口型/动作"动得飞快"。
pub(crate) fn apply_bridge_effects(state: &SharedState, dt_millis: f64) {
    let now_ms = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0);
    // 先 immutable borrow：dt 归一化口型衰减，取出本帧要用的 clone 值。
    let (apply_mouth, apply_mouth_sensitivity, scale, offset_x, offset_y, _dark) = {
        let mut st = state.borrow_mut();
        // 口型衰减（dt 归一化）：volume_display *= exp(-dt_ms / TAU_MS)
        // 再取 max(volume, ...) 保持峰值不掉落。
        // 该公式与帧率无关：60fps 与 240fps 衰减速率一致
        //   60fps:  exp(-16.67/90) ≈ 0.831
        //   240fps: exp(-4.17/90) ≈ 0.955
        // 二者单帧衰减不同，但*速率*相同——这是正确的物理行为。
        let decay = (-dt_millis / TAU_MS).exp();
        st.bridge.volume_display = st
            .bridge
            .volume
            .max(st.bridge.volume_display * decay as f32);
        (
            st.bridge.lip_sync.then(|| st.bridge.volume_display),
            st.bridge.mouth_sensitivity,
            st.bridge.scale,
            st.bridge.offset_x,
            st.bridge.offset_y,
            st.bridge.dark,
        )
    };

    let mut st = state.borrow_mut();

    // 口型：线性 RMS → dB 映射 → ParamMouthOpenY。
    //
    // **2026-09-10 修复**：旧实现把原始 RMS 直接写进参数（且只在 >0.01 时写），
    // 实测真实语音 RMS 中位数仅 ~0.03 → 嘴几乎不动。dB 映射见
    // [`mouth_open_from_level`]；静音（低于 -36 dBFS）自然归零，故不再需要
    // 旧的门限过滤（那个门限还会让轻声段落整段闭嘴）。
    let mouth = apply_mouth.map_or(0.0, |raw| {
        mouth_open_from_level(raw, apply_mouth_sensitivity)
    });
    let _ = st.core.set_parameter("ParamMouthOpenY", mouth);

    // 2026-09-12（rc.2）：动作参数 override 层已整体删除（见 `core-chain-baseline.md`
    // §3.3）。`final_override` 层因此没有任何写入方——待机生命体征走 input 层
    // （见下方 `apply_idle_life`），口型也走 input 层，两者都不依赖它。

    // 缩放 + 平移（模型级逻辑变换，背景不受影响）。
    //
    // 方案 A（l2d `set_transform(Affine2)`）：
    // `ModelRendererCore::set_transform` 直接设 `self.transform`（模型逻辑变换），
    // 渲染时 `aspect_fit_transform(viewport) * self.transform` 叠加 —— 这已是
    // layout_transform 初始值的基础上进行额外变换，背景 clear color 与 canvas
    // 尺寸完全不受影响，实现"仅模型缩放"。
    //
    // 关键：`set_transform` 是赋值（替换整个 transform 字段），不是累乘。
    // 因此 tick 每帧都必须在 `last_layout_transform`（load_model 后一次性捕获
    // 的 layout 初始值）基础上右乘用户缩放/平移，才能保持 layout 构图
    // 不变，仅叠加用户交互。
    //
    // offset_x/offset_y 存储的 CSS 像素单位，直接作为 NDC 平移分量
    // （bridge comment 声明"已换算为 NDC"），scale 作用于模型自身以
    //   画布中心为锚。
    let base = st.last_layout_transform;
    let user_transform = Affine2::from_translation(Vec2::new(offset_x, offset_y))
        * Affine2::from_scale(Vec2::splat(scale));
    st.core.set_transform(user_transform * base); // 左乘: NDC 屏幕空间等比缩放+平移(修上下压缩)

    // 舞台底色与背景图**不在这里**画。
    //
    // 2026-09-14（rc.5）：这里曾把 stageColor / 背景图写成 canvas 的 CSS
    // `background-color` / `background-image`，靠「canvas 像素透明」把 CSS 露出来。
    // 实测证伪：WebGPU 的 canvas 只能不透明合成（见 `background.rs` 头注），
    // 透明 clear 的像素被合成为不透明黑，把 CSS 整块盖住。
    //
    // 现在两者都由 [`super::background`] 在**背景预通道**里画进 framebuffer：
    // `stage_color` / `dark` 决定 clear 值，`bg_data_url` 决定要不要贴纹理。
    // 本函数只负责把 bridge 状态应用到模型（口型/缩放/待机）。

    // RM6 待机生命体征层（呼吸/眨眼/微表情）——写入 input 层。
    //
    // 2026-09-12（rc.2）：动作 override 层已删除，idle 层之上不再有 `final_override`
    // 写入方；**这一层本身不受影响，必须保留**（`core-chain-baseline.md` §3.4：
    // 待机生命体征与动作是两套机制）。`idle_enabled` 开关来自前端
    //「外观与互动 → 待机小动作」。
    //
    // - breath/blink 参数（ParamBreath / EyeL/R）只在这里写，不与口型争参数。
    //
    // dt 来源：`apply_bridge_effects(state, dt_millis)` 已有 dt（R4 加的），
    // idle 采样用它，帧率无关。
    //
    // 呼吸永不打断（底层持续）；眨眼/微表情状态机用 dt_ms 推进。
    // B1：idleEnabled=false 时跳过 idle 生命体征层（模型静止，仅保留 lip_sync）。
    if st.bridge.idle_enabled {
        apply_idle_life(&mut st, now_ms, dt_millis);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 舞台底色校验（2026-09-11） ──────────────────────────────────

    #[test]
    fn stage_color_accepts_hex_and_normalizes_case() {
        assert_eq!(normalize_stage_color("#000000").as_deref(), Some("#000000"));
        assert_eq!(normalize_stage_color("#FFFFFF").as_deref(), Some("#ffffff"));
        assert_eq!(normalize_stage_color("#1c1C1f").as_deref(), Some("#1c1c1f"));
        // 三位的简写也合法（CSS 允许）。
        assert_eq!(normalize_stage_color("#abc").as_deref(), Some("#abc"));
        // 前后空白容忍（父页拼串时可能带上）。
        assert_eq!(
            normalize_stage_color("  #061223 ").as_deref(),
            Some("#061223")
        );
    }

    /// **注入面**：这个值会进 `style.setProperty("background-color", …)`，
    /// 所以只认纯十六进制。任何其他构造一律拒绝（返回 None = 回落默认色）。
    #[test]
    fn stage_color_rejects_anything_that_is_not_plain_hex() {
        for bad in [
            "",
            "#",
            "#12",
            "#12345",
            "#1234567",
            "#gggggg",
            "red",
            "rgb(1,2,3)",
            "url(https://example.com/x.png)",
            "var(--x)",
            "#fff; background-image: url(x)",
            "#fff}body{background:red",
            "expression(alert(1))",
        ] {
            assert_eq!(
                normalize_stage_color(bad),
                None,
                "{bad:?} 不该被接受——它会被原样写进 CSS"
            );
        }
    }
}
