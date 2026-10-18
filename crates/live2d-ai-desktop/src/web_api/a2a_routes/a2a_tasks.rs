//! 进程内 A2A 任务表（0.3.8 rc.2）。
//!
//! 执行单 §3 的 tasks/get 要求：**按本进程内存里的 task id 取回，上限 32 条，
//! 不落盘；进程重启后任务消失**。因此这里刻意不接持久化、不接磁盘：
//! - 环形淘汰：第 33 条任务进来时丢最旧的一条（FIFO）；
//! - 生产用进程级单例（global_tasks），测试注入自己的 TaskStore 才能断言淘汰。

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde_json::Value;

/// 任务表上限（执行单 §3 钉死 32）。
pub(super) const TASK_STORE_CAP: usize = 32;

/// 进程内任务表：按 task id 取回 completed Task 的 JSON。
///
/// pub(crate)：handle_a2a_with 是 pub(crate) 的测试注入点，签名里用到它；
/// 其余（insert/get/len）仍只在本模块子树内可见。
pub(crate) struct TaskStore {
    cap: usize,
    /// FIFO 顺序（淘汰用）。
    order: VecDeque<String>,
    tasks: HashMap<String, Value>,
}

impl TaskStore {
    /// 新建任务表（生产 cap = TASK_STORE_CAP；测试可造更小 cap）。
    pub(super) fn new(cap: usize) -> Self {
        Self {
            cap,
            order: VecDeque::new(),
            tasks: HashMap::new(),
        }
    }

    /// 存一条任务；超过上限先淘汰最旧的一条（同 id 覆盖不重复占位）。
    pub(super) fn insert(&mut self, id: String, task: Value) {
        if let std::collections::hash_map::Entry::Occupied(mut existing) =
            self.tasks.entry(id.clone())
        {
            existing.insert(task);
            return;
        }
        while self.order.len() >= self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.tasks.remove(&oldest);
            }
        }
        self.order.push_back(id.clone());
        self.tasks.insert(id, task);
    }

    /// 按 id 取回（克隆；调用方不持有内部引用）。
    pub(super) fn get(&self, id: &str) -> Option<Value> {
        self.tasks.get(id).cloned()
    }

    /// 当前条数（测试断言淘汰 / 上限）。
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.tasks.len()
    }
}

/// 生产任务表单例（进程级；OnceLock 保证只建一次）。
pub(super) fn global_tasks() -> &'static Mutex<TaskStore> {
    static TASKS: OnceLock<Mutex<TaskStore>> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(TaskStore::new(TASK_STORE_CAP)))
}

/// 加锁读任务表；锁中毒不 panic（A2A 状态面不能因此整个失效）。
pub(super) fn lock_tasks(store: &Mutex<TaskStore>) -> MutexGuard<'_, TaskStore> {
    store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
