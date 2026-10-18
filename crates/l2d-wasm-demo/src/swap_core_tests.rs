//! `swap_core` 的原生单测（fake loader / fake host，不依赖 wasm 与 GPU）。

use super::*;

use crate::preset::OverrideFetch;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Ready;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// 假模型：只带 URL（状态机不关心模型内容）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct FakeModel {
    url: String,
}

#[derive(Default)]
struct FakeState {
    current_url: String,
    generation: u64,
    /// 渲染面模型代号（0.3.6 T2）：首次装载 = 1，成功换模才 +1。
    model_epoch: u64,
    /// 当前已装进运行时的字段表 model（证明换模后**不沿用旧表**）。
    field_map_model: String,
    /// 每次 install 依次消费：`None` = 脚本耗尽后默认成功。
    install_script: VecDeque<Result<(), String>>,
    /// 每次 install 的 `(url, 是否成功)`，按调用序。
    install_attempts: Vec<(String, bool)>,
    revoked: u32,
    phases: Vec<String>,
    statuses: Vec<String>,
    emitted: Vec<(String, serde_json::Value)>,
}

#[derive(Clone)]
struct FakeHost {
    st: Rc<RefCell<FakeState>>,
}

impl FakeHost {
    fn new(current_url: &str) -> Self {
        Self {
            st: Rc::new(RefCell::new(FakeState {
                current_url: current_url.to_owned(),
                // 首次装载的初始代号 = 1（0.3.6 T2 契约）。
                model_epoch: 1,
                field_map_model: crate::preset::model_id_from_url(current_url),
                ..Default::default()
            })),
        }
    }

    fn script_install(&self, results: impl IntoIterator<Item = Result<(), String>>) {
        self.st.borrow_mut().install_script.extend(results);
    }

    fn snapshot(&self) -> std::cell::Ref<'_, FakeState> {
        self.st.borrow()
    }
}

impl SwapHost for FakeHost {
    type Model = FakeModel;

    fn begin_generation(&mut self) -> u64 {
        let mut st = self.st.borrow_mut();
        st.generation += 1;
        st.generation
    }

    fn generation(&self) -> u64 {
        self.st.borrow().generation
    }

    fn model_epoch(&self) -> u64 {
        self.st.borrow().model_epoch
    }

    fn advance_model_epoch(&mut self) -> u64 {
        let mut st = self.st.borrow_mut();
        st.model_epoch += 1;
        st.model_epoch
    }

    fn current_model_id(&self) -> String {
        crate::preset::model_id_from_url(&self.st.borrow().current_url)
    }

    fn install(&mut self, model: &FakeModel) -> Result<(), String> {
        let mut st = self.st.borrow_mut();
        let result = st.install_script.pop_front().unwrap_or(Ok(()));
        st.install_attempts
            .push((model.url.clone(), result.is_ok()));
        result
    }

    fn revoke_fields(&mut self) {
        self.st.borrow_mut().revoked += 1;
    }

    fn set_current_url(&mut self, url: String) {
        self.st.borrow_mut().current_url = url;
    }

    fn apply_field_map(&mut self, map: crate::preset::FieldMap) {
        self.st.borrow_mut().field_map_model = map.model().to_owned();
    }

    fn status(&mut self, msg: &str) {
        self.st.borrow_mut().statuses.push(msg.to_owned());
    }

    fn phase(&mut self, msg: &str) {
        self.st.borrow_mut().phases.push(msg.to_owned());
    }

    fn emit(&mut self, ty: &str, payload: serde_json::Value) {
        self.st.borrow_mut().emitted.push((ty.to_owned(), payload));
    }
}

/// 按 URL 查表返回脚本化结果的 loader（默认每个 URL 成功并返回该 URL）。
#[derive(Clone, Default)]
struct FakeLoader {
    script: Rc<RefCell<VecDeque<Result<String, String>>>>,
}

impl FakeLoader {
    fn new(results: impl IntoIterator<Item = Result<String, String>>) -> Self {
        Self {
            script: Rc::new(RefCell::new(results.into_iter().collect())),
        }
    }
}

impl ModelLoader for FakeLoader {
    type Model = FakeModel;
    type Fut = Ready<Result<FakeModel, String>>;

    fn load(&self, _url: &str) -> Self::Fut {
        let r = self
            .script
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err("FakeLoader 未编排该次 load".to_owned()));
        std::future::ready(r.map(|url| FakeModel { url }))
    }
}

/// 手动放行的 loader：用于「连点 / 前一条慢请求后到」的时序测试。
#[derive(Clone, Default)]
struct GateLoader {
    slot: Rc<RefCell<Option<Result<FakeModel, String>>>>,
}

impl GateLoader {
    fn release(&self, model_url: &str) {
        *self.slot.borrow_mut() = Some(Ok(FakeModel {
            url: model_url.to_owned(),
        }));
    }
}

struct Gate {
    slot: Rc<RefCell<Option<Result<FakeModel, String>>>>,
}

impl Future for Gate {
    type Output = Result<FakeModel, String>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.slot.borrow_mut().take() {
            Some(v) => Poll::Ready(v),
            None => Poll::Pending,
        }
    }
}

impl ModelLoader for GateLoader {
    type Model = FakeModel;
    type Fut = Gate;

    fn load(&self, _url: &str) -> Gate {
        Gate {
            slot: Rc::clone(&self.slot),
        }
    }
}

/// 手动放行的字段表重载器：用于「字段表重载被新请求取代」的时序测试。
#[derive(Clone, Default)]
struct GateFieldMap {
    slot: Rc<RefCell<Option<FieldMapLoad>>>,
    reloaded: Rc<RefCell<Vec<String>>>,
}

impl GateFieldMap {
    fn release(&self, model_id: &str) {
        *self.slot.borrow_mut() = Some(crate::preset::rebuild_for_model(
            model_id,
            OverrideFetch::Missing("HTTP 404".to_owned()),
        ));
    }
}

struct FieldMapGate {
    slot: Rc<RefCell<Option<FieldMapLoad>>>,
}

impl Future for FieldMapGate {
    type Output = FieldMapLoad;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.slot.borrow_mut().take() {
            Some(v) => Poll::Ready(v),
            None => Poll::Pending,
        }
    }
}

impl FieldMapReloader for GateFieldMap {
    type Fut = FieldMapGate;

    fn reload(&self, model_id: &str) -> FieldMapGate {
        self.reloaded.borrow_mut().push(model_id.to_owned());
        FieldMapGate {
            slot: Rc::clone(&self.slot),
        }
    }
}

/// 假字段表重载器：记录收到的 model id，返回一次**真实**的 `rebuild_for_model` 结果。
///
/// 脚本耗尽后默认「外置表取不到（HTTP 404）」→ builtin（非降级），与桌面端
/// 没有 `/actions/field_map.json` 的现实一致。
#[derive(Clone, Default)]
struct FakeFieldMap {
    reloaded: Rc<RefCell<Vec<String>>>,
    script: Rc<RefCell<VecDeque<OverrideFetch>>>,
}

impl FakeFieldMap {
    fn with_fetch(results: impl IntoIterator<Item = OverrideFetch>) -> Self {
        Self {
            reloaded: Rc::default(),
            script: Rc::new(RefCell::new(results.into_iter().collect())),
        }
    }

    /// 外置覆盖表存在（用于验证 `source=external`）。
    fn external_override(model_id: &str) -> OverrideFetch {
        let json = format!(
            r#"{{"version":1,"models":{{"{model_id}":{{"head":{{"x":[{{"param":"ParamAngleX","amount":20.0}}]}}}}}}}}"#
        );
        OverrideFetch::Fetched(json.into_bytes())
    }
}

impl FieldMapReloader for FakeFieldMap {
    type Fut = Ready<FieldMapLoad>;

    fn reload(&self, model_id: &str) -> Self::Fut {
        self.reloaded.borrow_mut().push(model_id.to_owned());
        let fetch = self
            .script
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| OverrideFetch::Missing("HTTP 404".to_owned()));
        std::future::ready(crate::preset::rebuild_for_model(model_id, fetch))
    }
}

fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => {
            panic!("测试 future 不应在 block_on 里 Pending（Gate 请用 poll_once 推进）")
        }
    }
}

fn poll_once<F: Future>(fut: &mut Pin<Box<F>>) -> Poll<F::Output> {
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    fut.as_mut().poll(&mut cx)
}

const OLD: &str = "/models/bai/runtime/bai.model3.json";
const NEW: &str = "/models/epsilon_free/runtime/Epsilon_free.model3.json";

/// 纯用例文件（与 fake 基础设施分开，守「src 内 >500 行文件不新增」棘轮）。
#[path = "swap_core_cases.rs"]
mod cases;
