//! `mod_registry` 测试共享脚手架（自 `mod_registry.rs` 内联 `mod tests` 拆出）。
//!
//! 供同目录的 `tests_lifecycle` / `tests_host` 使用：测试用 Mod 工厂 / runtime、
//! 交付观察通道、静态工厂表。`pub(super)` 只为兄弟测试模块可见性，不进产品 API。

use super::*;

#[rustfmt::skip]
pub(super) struct TestMod;
impl ModFactory for TestMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "test",
            name: "Test",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(TestRuntime))
    }
}

/// worker 通过 shared slot 调用 on_event，TestRuntime 把收到的 topic 写到此处。
pub(super) static RECEIVED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// **事件送达信号**（D1 flaky 修复）：`TestRuntime::on_event` 每处理一条事件就把
/// topic 广播给所有已注册的测试端 `Sender`。等待方用 `recv_timeout` **阻塞**等信号，
/// 判据是「on_event 真的被 worker 调用过」——不再是「50ms×10 的墙钟窗口内有没有轮询到」。
/// 那个窗口在并发负载下会假红：worker 线程被抢占就可能超过任何固定时长。
pub(super) static DELIVERY_TX: Mutex<Vec<std::sync::mpsc::Sender<String>>> = Mutex::new(Vec::new());

/// 注册一个**本用例专属**的送达信号接收端。同 crate 里只有 enable 了 TestMod 且
/// 投递事件的用例会收到广播；当前只有 `event_delivers_to_running_mod`。
pub(super) fn watch_delivery() -> std::sync::mpsc::Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    DELIVERY_TX.lock().unwrap().push(tx);
    rx
}

pub(super) struct TestRuntime;
impl ModRuntime for TestRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        Ok(())
    }
    fn on_event(&mut self, topic: ModEventTopic, _: &str) -> Result<(), ModError> {
        let topic_str = topic.as_str().to_string();
        RECEIVED.lock().unwrap().push(topic_str.clone());
        // 先记 RECEIVED 再广播：等待方一收到信号，RECEIVED 必定已可见。
        // `retain`：接收端已被 drop 的 sender 顺手清掉，不随用例累积。
        DELIVERY_TX
            .lock()
            .unwrap()
            .retain(|tx| tx.send(topic_str.clone()).is_ok());
        Ok(())
    }
    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"test_state": "ready"}))
    }
}
pub(super) static FACTORIES: &[&dyn ModFactory] = &[&TestMod];
