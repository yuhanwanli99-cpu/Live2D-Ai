//! Windows 子进程树的父进程生命周期（0.4.0 rc.1）。
//!
//! Linux 侧靠 `PR_SET_PDEATHSIG` + 独立进程组（见 `folder_spawn`）；Windows
//! 没有这两样，改用**一个子进程一个 Job Object**：
//!
//! - 建 Job 时设 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`；
//! - 子进程 spawn 后 `AssignProcessToJobObject` 放进去；
//! - 父进程退出（正常关停 / 崩溃 / 任务管理器结束）→ 内核关闭最后一个 Job
//!   句柄 → 该 Job 里的**整个进程树**被结束，不会留下孤儿；
//! - 用户取消勾选时 `TerminateJobObject` 立刻结束整棵树（等价于 Linux 的
//!   整组 SIGTERM/SIGKILL）。
//!
//! 只声明 kernel32 的五个函数：本 crate 顶层依赖已顶格（18/18），**不加
//! windows crate**。工作区 `unsafe_code = deny`，本文件级显式豁免（与
//! `folder_spawn` 的 sys 模块同一条纪律）。
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

/// Win32 `HANDLE`。
type Handle = *mut c_void;

/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
/// `JobObjectExtendedLimitInformation`（`JOBOBJECTINFOCLASS` 枚举值）。
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;

#[repr(C)]
#[derive(Clone, Copy)]
struct BasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ExtendedLimitInformation {
    basic_limit_information: BasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(
        job: Handle,
        information_class: i32,
        information: *const c_void,
        information_length: u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
    fn CloseHandle(object: Handle) -> i32;
}

/// 一个子进程自己的 Job Object；`Drop` 关句柄，`KILL_ON_JOB_CLOSE` 随之结束子树。
pub struct Job(Handle);

// 只持一个内核句柄：句柄本身可跨线程使用（`FolderProcesses` 在 Mutex 里共享）。
unsafe impl Send for Job {}

impl Job {
    /// 建 Job 并把已 spawn 的子进程放进去。
    ///
    /// 任一步失败 → `None`：只记 warn，**不影响启动**（Linux 的等价能力同样
    /// 不是启动前置条件）；此时父进程正常退出仍会走 `folder_spawn` 的显式
    /// 结束路径，退化掉的只是「父进程被硬杀」时的兜底。
    pub fn attach(child: &Child) -> Option<Self> {
        unsafe {
            let raw = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if raw.is_null() {
                tracing::warn!(
                    pid = child.id(),
                    "Windows: CreateJobObject 失败（子进程退出兜底退化）"
                );
                return None;
            }
            let job = Job(raw);
            let mut limits: ExtendedLimitInformation = std::mem::zeroed();
            limits.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                job.0,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                std::ptr::addr_of!(limits).cast(),
                std::mem::size_of::<ExtendedLimitInformation>() as u32,
            );
            if set == 0 {
                tracing::warn!(
                    pid = child.id(),
                    "Windows: SetInformationJobObject(KILL_ON_JOB_CLOSE) 失败（子进程退出兜底退化）"
                );
                return None;
            }
            let assigned = AssignProcessToJobObject(job.0, child.as_raw_handle() as Handle);
            if assigned == 0 {
                tracing::warn!(
                    pid = child.id(),
                    "Windows: AssignProcessToJobObject 失败（父进程退出时靠显式结束兜底）"
                );
                return None;
            }
            Some(job)
        }
    }

    /// 结束 Job 里的整个进程树（取消勾选 / 关停时用）。
    pub fn terminate(&self) {
        unsafe {
            let _ = TerminateJobObject(self.0, 1);
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
