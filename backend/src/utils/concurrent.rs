//! 并发执行工具
//!
//! 这个模块提供了一个轻量级的 `join_all` 实现，用于替代 `futures::future::join_all`。
//! 使用 Rust 原生的 Future 轮询机制，不依赖 JavaScript Promise，因此不需要 `'static` 约束。

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// 并发执行多个异步任务
///
/// 与 `futures::future::join_all` 类似，但更轻量。
/// 不需要 `'static` 生命周期约束。
///
/// # 参数
/// - `futures`: 一个 Future 迭代器
///
/// # 返回
/// - 所有 Future 结果的 Vec（保持顺序）
pub fn join_all<F>(futures: impl IntoIterator<Item = F>) -> JoinAll<F>
where
    F: Future,
{
    let futures: Vec<_> = futures.into_iter().map(|f| MaybeDone::Pending(f)).collect();

    JoinAll { futures }
}

/// 表示一个可能已完成的 Future
enum MaybeDone<F: Future> {
    /// Future 仍在等待
    Pending(F),
    /// Future 已完成，结果已存储
    Done(F::Output),
    /// 结果已被取走
    Taken,
}

impl<F: Future> MaybeDone<F> {
    /// 尝试轮询 future，如果尚未完成
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> bool {
        // SAFETY: 我们不会移动 inner future
        let this = unsafe { self.get_unchecked_mut() };

        match this {
            MaybeDone::Pending(fut) => {
                // SAFETY: 我们保证不会移动 future
                let fut = unsafe { Pin::new_unchecked(fut) };
                match fut.poll(cx) {
                    Poll::Ready(output) => {
                        *this = MaybeDone::Done(output);
                        true // 完成
                    }
                    Poll::Pending => false, // 未完成
                }
            }
            MaybeDone::Done(_) => true,
            MaybeDone::Taken => true,
        }
    }

    /// 取出结果
    fn take_output(&mut self) -> Option<F::Output> {
        match std::mem::replace(self, MaybeDone::Taken) {
            MaybeDone::Done(output) => Some(output),
            _ => None,
        }
    }
}

/// `join_all` 返回的 Future 类型
pub struct JoinAll<F: Future> {
    futures: Vec<MaybeDone<F>>,
}

impl<F: Future> Future for JoinAll<F> {
    type Output = Vec<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: 我们不会移动 futures Vec，只会修改其内容
        let this = unsafe { self.get_unchecked_mut() };

        let mut all_done = true;

        for fut in &mut this.futures {
            // SAFETY: futures 不会被移动
            let fut = unsafe { Pin::new_unchecked(fut) };
            if !fut.poll(cx) {
                all_done = false;
            }
        }

        if all_done {
            let results: Vec<_> = this
                .futures
                .iter_mut()
                .map(|f| {
                    f.take_output()
                        .expect("Future completed but output missing")
                })
                .collect();
            Poll::Ready(results)
        } else {
            Poll::Pending
        }
    }
}

// 实现 Unpin，因为我们使用 Vec 并手动处理 Pin
impl<F: Future> Unpin for JoinAll<F> {}

// =============================================================================
// 简化版本：顺序执行（作为备选方案）
// =============================================================================

/// 顺序执行多个异步任务（简单、无额外开销）
///
/// 在不需要真正并发的场景下使用此函数。
#[allow(dead_code)]
pub async fn join_all_sequential<T, F>(futures: impl IntoIterator<Item = F>) -> Vec<T>
where
    F: Future<Output = T>,
{
    let mut results = Vec::new();
    for fut in futures {
        results.push(fut.await);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    // 简单测试，验证基本功能
    #[test]
    fn test_join_all_empty() {
        // 空输入应返回空 Vec
        let futures: Vec<std::future::Ready<i32>> = vec![];
        let join = join_all(futures);

        // 使用简单的阻塞执行器测试
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        let mut pinned = Box::pin(join);
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(results) => assert!(results.is_empty()),
            Poll::Pending => panic!("Empty join_all should complete immediately"),
        }
    }
}
