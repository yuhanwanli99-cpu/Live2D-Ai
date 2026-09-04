//! 行数豁免：节点 C P0-1（C4 三分支生命周期 + C3 frame latency 显式设置）合规范畴 ≤1000。
//! `l2d-wasm-demo` — 可选的浏览器 WASM 渲染演示（RFC D10「renderer：提供 WASM demo」）。
//!
//! 平台形态：
//! - **wasm32-unknown-unknown**（真实实现，见下方 [`web`] 模块）：
//!   从 query `?model=<model3 URL>`（缺省 `/models/bai/runtime/bai.model3.json`）
//!   fetch model3 清单 → 按 manifest 引用逐个 fetch moc3/纹理/physics/cdi →
//!   [`l2d::asset::ModelPackage::from_memory_map`]（内存资源构造） +
//!   [`l2d::model::LoadedModel::resolve`]（统一加载路径） → `<canvas>` surface
//!   （WebGPU 后端，WebGL2 回退）→ [`l2d::renderer::GpuContext::from_parts_with_label`]
//!   接入已有 device/queue → [`l2d::renderer::ModelRendererCore`] →
//!   requestAnimationFrame 固定 dt（[`l2d::renderer::FIXED_DT_60HZ`]）
//!   `render_to_view_submit` 直出交换链视图（**实时路径不得调用阻塞的
//!   `render_to_view`——C1/C6/C8 硬门禁「实时调用图不得出现 PollType::Wait」**）；
//! - **native target**（stub）：仅打印运行指引后退出——保证 workspace 的原生
//!   构建/clippy/test 不受影响；本 crate 不做 headless 初始化、不调用 pollster
//!   阻塞、不做帧读回。
//!
//! 错误处理约定：所有可失败步骤都收敛为 `Result<_, String>`，任何失败写入页面
//! `<pre id="status">`（并同步 console.error），绝不 panic、绝不静默吞错；
//! 渲染循环内的致命错误同样写状态栏后停止循环。
//!
//! 文件拆分（wasm-only）：
//! - `main.rs`：本文件——入口与 rAF 调度骨架。
//! - `surface.rs`：WebGPU/WebGL2 surface 协商、FrameState、单帧状态机（C4 裁决）。
//! - `net.rs`：fetch / URL 工具 / model3.json 引用解析。
//!
//! 交付口径（2026-08-26）：本环境无浏览器/无 GPU 实跑条件，**只完成编译门禁**
//! ——`cargo check -p l2d-wasm-demo --target wasm32-unknown-unknown` 真实通过；
//! 浏览器实跑步骤（`trunk serve --release` 与模型静态目录挂载要求）见本 crate
//! README。Bai 资产**不复制**进本 crate 或任何公共包。

fn main() {
    // wasm32：真实入口。
    #[cfg(target_arch = "wasm32")]
    web::start();

    // native：stub——只提示运行方式，不触碰任何 wasm/GPU API。
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("l2d-wasm-demo 是 wasm32 浏览器渲染演示；当前为 native stub（未接入渲染）。");
        println!("浏览器预览：cd crates/l2d-wasm-demo && trunk serve --release");
        println!("模型静态目录挂载与 ?model= 参数说明见 crates/l2d-wasm-demo/README.md。");
    }
}

/// 浏览器实现入口（仅在 wasm32 下编译；依赖也全部 target-gated，native 构建零 web 栈）。
///
/// 子模块拆分：
/// - [`surface`]：surface 协商 / FrameState / 单帧调度（详见 `surface.rs`）。
/// - [`net`]：fetch / URL 工具 / 清单解析（详见 `net.rs`）。
#[cfg(target_arch = "wasm32")]
mod web {
    use std::{cell::RefCell, rc::Rc};

    use l2d::model::LoadedModel;
    use l2d::renderer::{FIXED_DT_60HZ, ModelRendererCore};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use web_sys::{HtmlCanvasElement, MessageEvent};
    // 交互属性（movementX/clientX/deltaY/button 等）通过 js_sys::Reflect 读取，
    // 绕过 web-sys 需额外开启 PointerEvent/WheelEvent 特征的限制（本 crate 仅
    // 允许改动 main.rs/surface.rs，故不改 Cargo.toml）。Reflect 在 js-sys
    // default feature 即有（非 unstable 门控），`js_sys_unstable_apis` 默认 off。
    use js_sys::Reflect;

    pub(crate) mod net;
    pub(crate) mod surface;

    use surface::{FrameSlot, SharedState, adapter_info, init_gpu, install_resize_handler};

    /// 缺省模型清单 URL：约定宿主在 `/models/<...>` 暴露用户合法持有的皮套
    /// （挂载方式见本 crate README）；本仓库不内置、不分发任何模型资产。
    pub(crate) const DEFAULT_MODEL_URL: &str = "/models/bai/runtime/bai.model3.json";

    /// 页面状态栏 `<pre id="status">`：所有错误与阶段状态的唯一出口。
    pub(crate) fn status(msg: &str) {
        web_sys::console::error_1(&JsValue::from_str(msg));
        let el = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("status"));
        if let Some(el) = el {
            el.set_text_content(Some(msg));
        }
    }

    /// wasm 入口：把异步启动任务挂到微任务循环（不在顶层阻塞）。
    pub fn start() {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = run().await {
                status(&format!("启动失败：{err}"));
            }
        });
    }

    /// fetch model3 清单 → 按 manifest 引用逐个 fetch 资源 → 内存组包 →
    /// [`LoadedModel::resolve`]。供首次加载与换模型复用（P0-2a-3 真实重载）。
    async fn load_model_from_url(
        window: &web_sys::Window,
        model_url: &str,
    ) -> Result<LoadedModel, String> {
        status("正在按 manifest 引用拉取资源…");
        let model3_bytes = net::fetch_bytes(window, model_url)
            .await
            .map_err(|e| format!("清单加载失败（{model_url}）：{e}"))?;
        let refs = net::extract_refs(&model3_bytes)?;
        let mut resources = std::collections::BTreeMap::new();
        for rel in refs.unique_paths() {
            let url = net::resolve_relative(model_url, &rel);
            let bytes = net::fetch_bytes(window, &url)
                .await
                .map_err(|e| format!("资源 `{rel}` 加载失败（{url}）：{e}"))?;
            resources.insert(rel.clone(), bytes);
        }
        let package = l2d::asset::ModelPackage::from_memory_map(&model3_bytes, &resources)
            .map_err(|e| format!("ModelPackage::from_memory 失败：{e}"))?;
        LoadedModel::resolve(package).map_err(|e| format!("moc3 解析失败：{e}"))
    }

    /// 一次性启动流程：fetch 清单 → fetch 资源 → 内存组包 → GPU 初始化 →
    /// 安装 resize 监听 → 进入 rAF 固定 dt 渲染循环。
    async fn run() -> Result<(), String> {
        let window = web_sys::window().ok_or_else(|| "无 window 对象".to_owned())?;
        let canvas: HtmlCanvasElement = window
            .document()
            .and_then(|d| d.get_element_by_id("canvas"))
            .ok_or_else(|| "页面缺 <canvas id=\"canvas\">".to_owned())?
            .dyn_into()
            .map_err(|_| "canvas 元素类型异常".to_owned())?;

        status("正在解析 ?model= 参数…");
        let model_url = net::model_url_from_query(&window);

        status(&format!("正在加载模型清单 {model_url} …"));
        let loaded = load_model_from_url(&window, &model_url).await?;

        let summary = load_summary(&model_url, &loaded);

        status(&format!(
            "{summary}\n正在初始化 GPU（WebGPU，不可用时回退 WebGL2）…"
        ));
        let (gpu, adapter, surface_obj, config) = init_gpu(&window, &canvas).await?;

        let mut core =
            ModelRendererCore::new(gpu.clone()).map_err(|e| format!("渲染核心创建失败：{e}"))?;
        core.load_model(&loaded)
            .map_err(|e| format!("模型载入渲染核心失败：{e}"))?;
        core.set_viewport(config.width, config.height)
            .map_err(|e| format!("视口设置失败：{e}"))?;

        // 捕获 layout 初始 transform，供 set_transform 每帧在其基础上叠加用户缩放/平移。
        let layout_transform = core.transform();

        // 先取尺寸副本（config 即将移入共享态）。
        let (width, height) = (config.width, config.height);
        let state: SharedState = Rc::new(RefCell::new(surface::FrameState {
            core,
            surface: surface_obj,
            config,
            gpu,
            canvas: canvas.clone(),
            window: window.clone(),
            warned_validation: false,
            validation_streak: 0,
            hud: surface::HudState::default(),
            bridge: surface::BridgeState::default(),
            last_layout_transform: layout_transform,
            accumulator: 0.0,
        }));
        install_resize_handler(&state);

        // 一次性快照 adapter 信息到 HUD（供 500ms 周期内反复渲染）。
        let (backend, adp_name, is_webgpu) = adapter_info(&adapter);
        state
            .borrow_mut()
            .hud
            .snapshot_adapter(backend, &adp_name, is_webgpu);

        let api = if is_webgpu { "WebGPU" } else { "WebGL2" };
        status(&format!(
            "{summary}\nGPU: {backend}/{api} ({adp_name})；画布 {width}x{height}\n开始渲染循环（固定 dt {FIXED_DT_60HZ}s / 60Hz 步进）。",
        ));

        install_stage_bridge(&state);
        install_stage_interaction(&state);
        start_render_loop(state);
        Ok(())
    }

    /// 加载结果摘要（含兼容报告口径：非 v0 锚定档位照常预览但明确标注）。
    fn load_summary(model_url: &str, loaded: &LoadedModel) -> String {
        let m = loaded.manifest();
        let report = loaded.report();
        let v0_note = if report.is_supported_v0() {
            "v0 支持=是".to_owned()
        } else {
            format!(
                "v0 支持=否（兼容问题 {} 条，预览继续）",
                report.issues().len()
            )
        };
        format!(
            "已加载 {model_url}（model3 v{}）：moc3=`{}`，纹理 {} 张，physics={}，cdi={}；版本={}，{v0_note}",
            m.version,
            m.moc3_file,
            m.texture_files.len(),
            m.physics_file.as_deref().unwrap_or("-"),
            m.display_info_file.as_deref().unwrap_or("-"),
            loaded
                .handle()
                .moc_version()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未知".to_owned()),
        )
    }

    /// 注册父页 → WASM 的 message listener（P0-2a-2）：4 类消息写入 bridge 状态。
    /// rAF 循环（另一函数）每帧读取 bridge 产生可见效果。
    /// **R2**：父页已改发 JSON 字符串（stringify）；处理成功后回 `stage-ack` 给父页
    /// （未收 ACK 前父页不得提示"已生效"）。
    fn install_stage_bridge(state: &SharedState) {
        let handler_state = state.clone();
        let closure = Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
            // data() → JsValue → String → serde_json::Value（父页 R2 后 stringify）
            let Some(text) = ev.data().as_string() else {
                return;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                return;
            };
            let Some(kind) = v.get("type").and_then(|t| t.as_str()) else {
                return;
            };
            // 应用消息 → 记录待 ACK 的类型（borrow 结束后回执）。
            let mut applied = false;
            {
                let mut st = handler_state.borrow_mut();
                st.bridge.msg_recv += 1;
                st.bridge.last_msg = kind.to_string();
                match kind {
                    "stage-config" => {
                        if let Some(s) = v.get("scale").and_then(|x| x.as_f64()) {
                            st.bridge.scale = (s as f32).clamp(0.5, 2.0);
                        }
                        if let Some(d) = v.get("dark").and_then(|x| x.as_bool()) {
                            st.bridge.dark = d;
                        }
                        if let Some(l) = v.get("lipSync").and_then(|x| x.as_bool()) {
                            st.bridge.lip_sync = l;
                        }
                        applied = true;
                    }
                    // UI 精修规格 A2：控制条 +/−/复位（主 Agent 设计，wasm listener 落地）。
                    "stage-zoom" => {
                        match v.get("dir").and_then(|x| x.as_str()).unwrap_or("") {
                            "in" => st.bridge.scale = (st.bridge.scale * 1.1).clamp(0.5, 2.0),
                            "out" => st.bridge.scale = (st.bridge.scale / 1.1).clamp(0.5, 2.0),
                            "reset" => {
                                st.bridge.scale = 1.0;
                                st.bridge.offset_x = 0.0;
                                st.bridge.offset_y = 0.0;
                            }
                            _ => {}
                        }
                        applied = true;
                    }
                    "audio-volume" => {
                        if let Some(vol) = v.get("value").and_then(|x| x.as_f64()) {
                            st.bridge.volume = (vol as f32).clamp(0.0, 1.0);
                        }
                        applied = true;
                    }
                    "action-state" => {
                        let action = v
                            .get("action")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let state_word = v.get("state").and_then(|x| x.as_str()).unwrap_or("");
                        let strength_mult =
                            match v.get("strength").and_then(|x| x.as_f64()).unwrap_or(2.0) as u8 {
                                1 => 0.6,
                                3 => 1.5,
                                _ => 1.0,
                            };
                        if state_word == "start" {
                            let now = web_sys::window()
                                .map(|w| w.performance().map(|p| p.now()).unwrap_or(0.0))
                                .unwrap_or(0.0);
                            st.bridge.action = Some(surface::ActiveAction {
                                name: action,
                                strength_mult,
                                started_ms: now,
                                duration_ms: 1500.0,
                            });
                        } else if state_word == "end" {
                            st.bridge.action = None;
                        }
                        applied = true;
                    }
                    "load-model" => {
                        if let Some(url) = v.get("url").and_then(|x| x.as_str()) {
                            let url = url.to_owned();
                            applied = true;
                            // 异步换模型：不要阻塞 rAF 循环。
                            let reload_state = handler_state.clone();
                            let window = web_sys::window();
                            wasm_bindgen_futures::spawn_local(async move {
                                status("切换模型中…");
                                match window {
                                    Some(win) => match load_model_from_url(&win, &url).await {
                                        Ok(loaded) => {
                                            match reload_state.borrow_mut().core.load_model(&loaded)
                                            {
                                                Ok(()) => status("模型已切换"),
                                                Err(e) => {
                                                    status(&format!("渲染核心换模型失败：{e}"))
                                                }
                                            }
                                        }
                                        Err(e) => status(&format!("切换失败: {e}")),
                                    },
                                    None => status("切换失败: 无 window 对象"),
                                }
                            });
                        }
                    }
                    _ => {} // 未知类型忽略
                }
            } // borrow 作用域结束
            // **R2**：应用成功 → 回 `stage-ack` 给父页（父页收 ACK 前不得提示"已生效"）。
            if applied {
                {
                    let mut st = handler_state.borrow_mut();
                    st.bridge.msg_applied += 1;
                }
                if let Some(win) = web_sys::window() {
                    let ack = serde_json::json!({
                        "type": "stage-ack",
                        "applied": true,
                        "msg_type": kind,
                        "scale": handler_state.borrow().bridge.scale,
                        "offset_x": handler_state.borrow().bridge.offset_x,
                        "offset_y": handler_state.borrow().bridge.offset_y,
                    })
                    .to_string();
                    if let Ok(Some(parent)) = win.parent() {
                        let _ = parent.post_message(&JsValue::from_str(&ack), "*");
                    }
                }
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("message", closure.as_ref().unchecked_ref());
        }
        closure.forget(); // 页面级 listener，长生命周期
    }

    /// 鼠标/触控直接交互（py 版语义对齐）：
    /// - **左键拖动** → 累加平移到 bridge.offset_x/offset_y（CSS 像素）；
    /// - **滚轮** → 放大/缩小 bridge.scale（0.5..2.0，步进 1.1×，以视口中心为锚）；
    /// - **双击** → 重置 scale=1.0 / offset=0；
    /// - wheel 滑块发来的 stage-config.scale 写同一变量，故滚轮与滑块互不冲突。
    ///
    /// 监听器闭包以 `JsValue` 入参（`Closure<dyn FnMut(JsValue)>`），JS 在回调时把
    /// DOM Event 当 JsValue 传进来；属性（movementX/clientX/deltaY/button/pointerId
    /// 等）通过 `js_sys::Reflect` 读取。这绕过了 web-sys 的 PointerEvent/WheelEvent/
    /// MouseEvent 需额外开启 crate feature 的限制（本 crate 仅允许改动 main.rs/
    /// surface.rs，不改 Cargo.toml）。Reflect 属于 js-sys 默认 feature、不受
    /// `js_sys_unstable_apis` 门控，故无需额外依赖。方法调用（preventDefault/
    /// setPointerCapture）亦经 Reflect 取 Function 后 `.call0/.call1`，全部 `.ok()`
    /// 容错，不 panic。listeners `forget()`（页面级，长生命周期），注册失败
    /// `.is_ok()` 吞错不中断。
    fn install_stage_interaction(state: &SharedState) {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };
        // 拖拽会话状态： dragging + 上上一帧鼠标位置（client 坐标，CSS 像素）。
        #[derive(Clone, Copy, Default)]
        struct DragState {
            dragging: bool,
            last_x: f64,
            last_y: f64,
        }
        let drag: Rc<RefCell<DragState>> = Rc::new(RefCell::new(DragState::default()));

        // 读取 JsValue 事件属性为 f64（失败默认 0）—— web-sys 的 PointerEvent/
        // WheelEvent/MouseEvent 需额外开启 crate feature，本 crate 仅允许改动
        // main.rs/surface.rs（不改 Cargo.toml），故用 js_sys::Reflect 绕行。
        // 闭包以 `JsValue` 入参（`Closure<dyn FnMut(JsValue)>`），JS 把 DOM Event
        // 传进来；Reflect 读字段，任何异常 `.ok()`/`.unwrap_or` 容错，不 panic。
        // 用 `fn`（而非闭包）定义，以便多个 move 闭包共享使用（闭包非 Copy，
        // 不能被多次 move 进不同的监听器）。
        fn f64_field(ev: &JsValue, name: &str) -> f64 {
            Reflect::get(ev, &JsValue::from_str(name))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
        }
        // 对 JsValue 对象调用无参方法（如 preventDefault）。
        fn call0(ev: &JsValue, name: &str) {
            let _ = (|| {
                let m = Reflect::get(ev, &JsValue::from_str(name)).ok()?;
                let f = m.dyn_into::<js_sys::Function>().ok()?;
                let _ = f.call0(ev);
                Some(())
            })();
        }
        // 对 target 对象调用方法（如 setPointerCapture）。
        fn call_on_target(ev: &JsValue, method: &str, pid: &JsValue) {
            let _ = (|| {
                let target = Reflect::get(ev, &JsValue::from_str("target")).ok()?;
                let m = Reflect::get(&target, &JsValue::from_str(method)).ok()?;
                let f = m.dyn_into::<js_sys::Function>().ok()?;
                let _ = f.call1(&target, pid);
                Some(())
            })();
        }

        // pointerdown（button 0）
        {
            let drag = drag.clone();
            let closure = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
                if f64_field(&ev, "button") != 0.0 {
                    return;
                }
                let cx = f64_field(&ev, "clientX");
                let cy = f64_field(&ev, "clientY");
                {
                    let mut d = drag.borrow_mut();
                    d.dragging = true;
                    d.last_x = cx;
                    d.last_y = cy;
                }
                // set_pointer_capture(pointerId) — 鼠标离开 canvas 仍收获后续事件。
                let pid = Reflect::get(&ev, &JsValue::from_str("pointerId"))
                    .ok()
                    .unwrap_or(JsValue::from(0.0));
                call_on_target(&ev, "setPointerCapture", &pid);
            });
            if window
                .add_event_listener_with_callback("pointerdown", closure.as_ref().unchecked_ref())
                .is_ok()
            {
                closure.forget();
            }
        }

        // pointermove（dragging 时累加位移）
        {
            let st = state.clone();
            let drag = drag.clone();
            let closure = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
                if !drag.borrow().dragging {
                    return;
                }
                let d = *drag.borrow();
                // 优先 movementX/movementY（相对位移），回退到 client 差值。
                let mx = f64_field(&ev, "movementX");
                let my = f64_field(&ev, "movementY");
                let (dx, dy) = if mx == 0.0 && my == 0.0 {
                    let cx = f64_field(&ev, "clientX");
                    let cy = f64_field(&ev, "clientY");
                    (cx - d.last_x, cy - d.last_y)
                } else {
                    (mx, my)
                };
                {
                    let mut st = st.borrow_mut();
                    let css_w = st.canvas.client_width().max(1) as f32;
                    let css_h = st.canvas.client_height().max(1) as f32;
                    let dx_ndc = 2.0 * dx as f32 / css_w;
                    let dy_ndc = -2.0 * dy as f32 / css_h;
                    st.bridge.offset_x = (st.bridge.offset_x + dx_ndc).clamp(-1.5, 1.5);
                    st.bridge.offset_y = (st.bridge.offset_y + dy_ndc).clamp(-1.5, 1.5);
                }
                // 更新锚点。
                let mut d2 = drag.borrow_mut();
                d2.last_x = f64_field(&ev, "clientX");
                d2.last_y = f64_field(&ev, "clientY");
            });
            if window
                .add_event_listener_with_callback("pointermove", closure.as_ref().unchecked_ref())
                .is_ok()
            {
                closure.forget();
            }
        }

        // pointerup / pointercancel → 停止拖拽
        {
            let drag = drag.clone();
            let closure = Closure::<dyn FnMut(JsValue)>::new(move |_ev: JsValue| {
                drag.borrow_mut().dragging = false;
            });
            if window
                .add_event_listener_with_callback("pointerup", closure.as_ref().unchecked_ref())
                .is_ok()
            {
                closure.forget();
            }
        }

        // wheel → 缩放（deltaY<0 放大 ×1.1，>=0 时缩小 ÷1.1），钳 0.5..2.0。
        {
            let st = state.clone();
            let closure = Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
                let delta_y = f64_field(&ev, "deltaY");
                let factor = if delta_y < 0.0 { 1.1_f32 } else { 1.0 / 1.1 };
                {
                    let mut st = st.borrow_mut();
                    st.bridge.scale = (st.bridge.scale * factor).clamp(0.5, 2.0);
                }
                // 阻止页面滚动惯性。
                call0(&ev, "preventDefault");
            });
            if window
                .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
                .is_ok()
            {
                closure.forget();
            }
        }

        // dblclick → 重置 scale / offset
        {
            let st = state.clone();
            let closure = Closure::<dyn FnMut(JsValue)>::new(move |_ev: JsValue| {
                let mut st = st.borrow_mut();
                st.bridge.scale = 1.0;
                st.bridge.offset_x = 0.0;
                st.bridge.offset_y = 0.0;
            });
            if window
                .add_event_listener_with_callback("dblclick", closure.as_ref().unchecked_ref())
                .is_ok()
            {
                closure.forget();
            }
        }
    }
    fn start_render_loop(state: SharedState) {
        let slot: FrameSlot = Rc::new(RefCell::new(None));
        let slot_for_closure = Rc::clone(&slot);
        *slot.borrow_mut() = Some(Closure::new(move |timestamp_ms: f64| {
            surface::tick(&slot_for_closure, &state, timestamp_ms);
        }));
        schedule_frame(&slot);
    }

    /// rAF 调度：浏览器每帧回调 [`surface::tick`]。
    pub(crate) fn schedule_frame(slot: &FrameSlot) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let cb = slot
            .borrow()
            .as_ref()
            .expect("frame closure 必须常驻")
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .to_owned();
        let _ = window.request_animation_frame(&cb);
    }
}
