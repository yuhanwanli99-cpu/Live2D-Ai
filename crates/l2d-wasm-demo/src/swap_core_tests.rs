//! `swap_core` 的原生单测（fake loader / fake host，不依赖 wasm 与 GPU）。

use super::*;

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
    /// 每次 install 依次消费：`None` = 脚本耗尽后默认成功。
    install_script: VecDeque<Result<(), String>>,
    /// 每次 install 的 `(url, 是否成功)`，按调用序。
    install_attempts: Vec<(String, bool)>,
    revoked: u32,
    /// 字段映射重载收到的 model id（按调用序）。
    reloaded: Vec<String>,
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

#[derive(Clone, Default)]
struct FakeFieldMap {
    reloaded: Rc<RefCell<Vec<String>>>,
}

impl FieldMapReloader for FakeFieldMap {
    type Fut = Ready<String>;

    fn reload(&self, model_id: &str) -> Self::Fut {
        self.reloaded.borrow_mut().push(model_id.to_owned());
        std::future::ready(format!("字段映射覆盖表：model={model_id}"))
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

/// 成功：装载 → revoke → 更新 URL → 重载字段表 → 只发一条 `loaded`。
#[test]
fn success_installs_revokes_and_emits_loaded_once() {
    let host = FakeHost::new(OLD);
    let loader = FakeLoader::new([Ok(NEW.to_owned())]);
    let field_map = FakeFieldMap::default();

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.generation, 1, "每个请求都必须自增一次代际");
    assert_eq!(st.current_url, NEW, "成功后 current_url 必须是新 URL");
    assert_eq!(
        st.install_attempts,
        vec![(NEW.to_owned(), true)],
        "成功路径只装一次新模型（不装载旧 URL）"
    );
    assert_eq!(st.revoked, 1, "成功后必须 revoke 一次旧贡献 / pending");
    assert_eq!(
        field_map.reloaded.borrow().as_slice(),
        &["Epsilon_free".to_owned()],
        "成功后必须按新 model id 重载字段表"
    );
    let loaded: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "loaded").collect();
    assert_eq!(loaded.len(), 1, "loaded 只能发一条");
    assert_eq!(loaded[0].1["url"], NEW);
    assert!(
        st.statuses.iter().any(|s| s.contains("模型已切换")),
        "状态栏必须报告成功：{:?}",
        st.statuses
    );
    assert!(st.phases.iter().any(|p| p.contains("装载成功")));
}

/// 失败（坏 URL）：只回 `error` 帧；旧模型、旧 URL、字段贡献都不动。
#[test]
fn fetch_failure_keeps_old_model_and_emits_error_only() {
    let host = FakeHost::new(OLD);
    let loader = FakeLoader::new([Err("HTTP 404".to_owned())]);
    let field_map = FakeFieldMap::default();

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.current_url, OLD, "失败必须保留旧模型 URL");
    assert!(st.install_attempts.is_empty(), "坏 URL 不得碰渲染核心");
    assert_eq!(st.revoked, 0, "失败不得 revoke 旧贡献");
    assert!(field_map.reloaded.borrow().is_empty(), "失败不得重载字段表");
    let errs: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "error").collect();
    assert_eq!(errs.len(), 1);
    assert!(
        errs[0].1["message"]
            .as_str()
            .unwrap()
            .contains("切换失败: HTTP 404")
    );
    assert_eq!(errs[0].1["url"], NEW);
    assert!(
        !st.emitted.iter().any(|(ty, _)| ty == "loaded"),
        "失败不得发 loaded"
    );
}

/// 失败（装载半失败）：install 是原子的 → **旧核心一个字段都没被碰过**，
/// 不回滚装载旧 URL，只回 error（T1 修法 B 的核心断言）。
#[test]
fn install_failure_leaves_old_core_untouched_and_does_not_rollback_load() {
    let host = FakeHost::new(OLD);
    host.script_install([Err("physics3.json 解析失败".to_owned())]);
    let loader = FakeLoader::new([Ok(NEW.to_owned()), Ok(OLD.to_owned())]);
    let field_map = FakeFieldMap::default();

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.current_url, OLD, "装载失败必须保留旧模型 URL");
    assert_eq!(
        st.install_attempts,
        vec![(NEW.to_owned(), false)],
        "只允许尝试装新模型一次；**不得**再 load 旧 URL 回滚"
    );
    assert_eq!(st.revoked, 0);
    assert!(field_map.reloaded.borrow().is_empty());
    assert!(
        !st.emitted.iter().any(|(ty, _)| ty == "loaded"),
        "失败不得发 loaded"
    );
    let errs: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "error").collect();
    assert_eq!(errs.len(), 1);
    assert!(
        errs[0].1["message"]
            .as_str()
            .unwrap()
            .contains("渲染核心换模型失败")
    );
    assert!(
        st.phases.iter().any(|p| p.contains("旧核心未受影响")),
        "失败阶段日志必须写明旧核心未受影响：{:?}",
        st.phases
    );
}

/// 连点 / 并发：前一条慢请求的结果后到 → 代际守卫丢弃，只装最后一次。
#[test]
fn stale_result_is_discarded_and_only_latest_swap_lands() {
    let host = FakeHost::new(OLD);
    let first_loader = GateLoader::default();
    let second_loader = FakeLoader::new([Ok(NEW.to_owned())]);
    let field_map = FakeFieldMap::default();
    // 慢请求的字段表重载器单独持有：它被丢弃后**不得**被调用。
    let stale_field_map = FakeFieldMap::default();

    // 第一次请求：拉取被 Gate 挂起（模拟慢请求）。
    let mut h1 = host.clone();
    let mut first = Box::pin(run_swap(
        &mut h1,
        "/models/slow.model3.json".to_owned(),
        &first_loader,
        &stale_field_map,
    ));
    assert!(matches!(poll_once(&mut first), Poll::Pending));
    assert_eq!(host.snapshot().generation, 1);

    // 第二次请求：立刻完成（新请求取代旧请求）。
    let mut h2 = host.clone();
    block_on(run_swap(
        &mut h2,
        NEW.to_owned(),
        &second_loader,
        &field_map,
    ));
    assert_eq!(host.snapshot().current_url, NEW);

    // 放行第一次请求的结果：必须被丢弃。
    first_loader.release("/models/slow.model3.json");
    assert!(matches!(poll_once(&mut first), Poll::Ready(())));

    let st = host.snapshot();
    assert_eq!(st.generation, 2);
    assert_eq!(st.current_url, NEW, "慢请求后到不得覆盖已生效的新模型");
    assert_eq!(
        st.install_attempts,
        vec![(NEW.to_owned(), true)],
        "只有最后一次请求允许装核心（连点只装最后一次）"
    );
    assert_eq!(st.revoked, 1, "被丢弃的请求不得 revoke");
    assert_eq!(
        field_map.reloaded.borrow().as_slice(),
        &["Epsilon_free".to_owned()],
        "只有最后一次请求允许重载字段表"
    );
    assert!(
        stale_field_map.reloaded.borrow().is_empty(),
        "被丢弃的慢请求不得重载字段表"
    );
    assert_eq!(
        st.emitted.iter().filter(|(ty, _)| ty == "loaded").count(),
        1,
        "被丢弃的请求不得发 loaded"
    );
    assert!(
        st.phases.iter().any(|p| p.contains("丢弃结果")),
        "被丢弃的请求必须留下阶段日志：{:?}",
        st.phases
    );
}
