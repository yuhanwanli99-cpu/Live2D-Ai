//! 有界 f32 样本队列 [`SampleQueue`]：解码线程（生产者）与播放消费之间的
//! 背压边界。
//!
//! # 溢出策略（显式，且对音频线程友好）
//!
//! - **接纳前缀**：空间不足时写入能容纳的前缀，**返回实际接纳的样本数**，
//!   多余部分被调用方感知并丢弃——不阻塞、不 panic、不覆盖已有数据；
//! - FIFO 顺序恒定：先写入的样本先被读出；
//! - 所有操作 O(n)（n 为本批样本数）且无锁；当前实现为纯 std
//!   `VecDeque` 的单消费者 `&mut` API。接入 cpal 回调需要跨线程无锁 SPSC
//!   时，内部替换为 `ringbuf` Producer/Consumer，对外 facade 不变
//!   （见模块根文档）。
//!
//! 队列是**纯传输层**：原样存储样本（包括非有限值），不做任何 DSP 处理。

use std::collections::VecDeque;

use crate::error::{Error, Result};

/// 有界 f32 样本 FIFO。容量以**样本数**计；多声道时一个 interleaved 帧占
/// `channels` 个样本，由调用方自行换算。
#[derive(Debug)]
pub struct SampleQueue {
    buf: VecDeque<f32>,
    capacity: usize,
}

impl SampleQueue {
    /// 创建容量为 `capacity` 个样本的队列；0 容量返回
    /// [`Error::InvalidAudioConfig`]。
    pub fn new(capacity: usize) -> Result<Self> {
        if capacity == 0 {
            return Err(Error::InvalidAudioConfig {
                message: "队列容量必须非零".to_string(),
            });
        }
        Ok(Self {
            buf: VecDeque::with_capacity(capacity),
            capacity,
        })
    }

    /// 最大容量（样本数）。
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// 当前存量（样本数）。
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// 队列是否为空。
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// 剩余可写空间（样本数）。
    pub fn free_space(&self) -> usize {
        self.capacity - self.buf.len()
    }

    /// 写入一批样本。空间不足时接纳前缀并返回接纳数（≤ `samples.len()`）；
    /// 溢出部分被拒绝——由调用方决定记日志/丢弃。永不阻塞、永不 panic。
    pub fn push_slice(&mut self, samples: &[f32]) -> usize {
        let accepted = samples.len().min(self.free_space());
        self.buf.extend(samples[..accepted].iter().copied());
        accepted
    }

    /// 读出至多 `out.len()` 个样本到 `out` 前缀，返回实际读出的数量。
    /// 队列空时返回 0（`out` 内容不动）。
    pub fn pop_slice(&mut self, out: &mut [f32]) -> usize {
        let n = out.len().min(self.buf.len());
        for slot in &mut out[..n] {
            *slot = self.buf.pop_front().expect("len checked above");
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_capacity_is_rejected() {
        assert!(matches!(
            SampleQueue::new(0),
            Err(Error::InvalidAudioConfig { .. })
        ));
    }

    #[test]
    fn fifo_order_is_preserved_across_wraparound() {
        let mut q = SampleQueue::new(4).expect("valid");
        // 反复交错读写，迫使 VecDeque 内部环形缓冲多次回绕。
        for round in 0..16_i32 {
            let base = round * 4;
            assert_eq!(
                q.push_slice(&[
                    base as f32,
                    1. + base as f32,
                    2. + base as f32,
                    3. + base as f32
                ]),
                4
            );
            let mut out = [0.0_f32; 4];
            assert_eq!(q.pop_slice(&mut out), 4);
            assert_eq!(
                out,
                [
                    base as f32,
                    1. + base as f32,
                    2. + base as f32,
                    3. + base as f32
                ]
            );
        }
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn overflow_accepts_prefix_and_reports_count() {
        let mut q = SampleQueue::new(4).expect("valid");
        assert_eq!(q.push_slice(&[1.0, 2.0]), 2);
        // 只剩 2 格：前缀被接纳，多余的被拒绝，已有数据不受影响。
        assert_eq!(q.push_slice(&[3.0, 4.0, 5.0, 6.0]), 2);
        assert_eq!(q.len(), q.capacity());
        assert_eq!(q.free_space(), 0);

        let mut out = [0.0_f32; 6];
        assert_eq!(q.pop_slice(&mut out), 4);
        assert_eq!(&out[..4], &[1.0, 2.0, 3.0, 4.0], "顺序必须保持，不得覆盖");

        // 满后再写：一格都进不去。
        let mut full = SampleQueue::new(2).expect("valid");
        full.push_slice(&[7.0, 8.0]);
        assert_eq!(full.push_slice(&[9.0]), 0);
        assert_eq!(full.push_slice(&[]), 0);
    }

    #[test]
    fn partial_reads_and_underrun_behave() {
        let mut q = SampleQueue::new(8).expect("valid");
        q.push_slice(&[1.0, 2.0, 3.0]);

        let mut out = [0.0_f32; 2];
        assert_eq!(q.pop_slice(&mut out), 2);
        assert_eq!(out, [1.0, 2.0]);

        // 欠载：只返回现存数量，剩余槽位不动。
        let mut rest = [0.0_f32; 5];
        assert_eq!(q.pop_slice(&mut rest), 1);
        assert_eq!(rest[0], 3.0);

        // 空队列读取是干净的无操作。
        let mut again = [f32::NAN; 3];
        assert_eq!(q.pop_slice(&mut again), 0);
    }

    #[test]
    fn queue_is_a_passthrough_transport_for_any_finite_or_not_values() {
        let mut q = SampleQueue::new(4).expect("valid");
        assert_eq!(q.push_slice(&[f32::NAN, f32::INFINITY, -0.0]), 3);
        let mut out = [1.0_f32; 3];
        assert_eq!(q.pop_slice(&mut out), 3);
        assert!(out[0].is_nan());
        assert!(out[1].is_infinite());
        assert_eq!(out[2], -0.0, "传输层原样搬运，连符号位也不动");
    }
}
