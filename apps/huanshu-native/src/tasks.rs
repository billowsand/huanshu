//! 线程模型：**UI 线程只做界面**，其余一律丢到后台，结果回到界面后再落地。
//!
//! - 同步重活（Typst 渲染、PDF 导出、读写数据库、字体枚举）：[`run_bg`]，跑在 GPUI 的后台执行器。
//! - core 的 `async` 服务（生成流水线、模型列表、AI 优化……）要求 **tokio** 运行时
//!   （reqwest），GPUI 的执行器不是 tokio：用 [`run_async`]，它在进程内唯一的 tokio 运行时上跑，
//!   结果再交回界面。
//! - 生成事件：core 只认 `EventSink`。[`ChannelSink`] 把 `CoreEvent` 送进一个通道，
//!   页面用 [`listen_events`] 在界面线程上逐条接收（替代 Tauri 的 `listen('gen:*')`）。
//!
//! 回调里拿到的是视图的 `&mut T` 与窗口；视图已被丢弃（换页了）时回调不会执行。
//! **异步结果回来时，项目、页面、主题都可能已经变了**：需要时自己带上「发起时的身份」比对。
//!
//! `*_in` 是同一套东西的**无窗口**版本（[`run_bg_in`] / [`run_async_in`] / [`listen_events_in`]）：
//! 应用级的 `GenerationStore` 比任何页面活得久、手上没有 `Window`，用 `cx.spawn` 在实体上下文
//! （`AsyncApp`）上跑。回调拿到的是 `&mut Context<T>`，页面用不到窗口就不要走带窗口的版本。

// `ChannelSink` / `listen_events` 是给生成流水线与模型服务页准备的框架接口，
// 那些页面接上之前暂时没有调用方。
#![allow(dead_code)]

use std::future::Future;
use std::sync::OnceLock;

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::{AppContext as _, Context, Window};
use huanshu_core::events::{CoreEvent, EventSink};

/// 在后台线程上执行同步的 `work`，完成后在界面线程上调用 `done`。
pub fn run_bg<T, R>(
    window: &mut Window,
    cx: &mut Context<T>,
    work: impl FnOnce() -> R + Send + 'static,
    done: impl FnOnce(&mut T, R, &mut Window, &mut Context<T>) + 'static,
) where
    T: 'static,
    R: Send + 'static,
{
    cx.spawn_in(window, async move |this, window| {
        let result = window.background_spawn(async move { work() }).await;
        let _ = this.update_in(window, move |view, window, cx| {
            done(view, result, window, cx)
        });
    })
    .detach();
}

/// [`run_bg`] 的无窗口版本：实体上下文（应用级 store）用。
/// 回调拿到的是 `&mut Context<T>`，没有窗口。
pub fn run_bg_in<T, R>(
    cx: &mut Context<T>,
    work: impl FnOnce() -> R + Send + 'static,
    done: impl FnOnce(&mut T, R, &mut Context<T>) + 'static,
) where
    T: 'static,
    R: Send + 'static,
{
    cx.spawn(async move |this, cx| {
        let result = cx.background_spawn(async move { work() }).await;
        let _ = this.update(cx, move |view, cx| done(view, result, cx));
    })
    .detach();
}

/// 进程内唯一的 tokio 运行时（core 的 async 服务需要它）。
pub fn tokio() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("huanshu-tokio")
            .build()
            .expect("start the tokio runtime")
    })
}

/// 在 tokio 上执行 `future`（core 的 async 服务），完成后在界面线程上调用 `done`。
pub fn run_async<T, R, F>(
    window: &mut Window,
    cx: &mut Context<T>,
    future: F,
    done: impl FnOnce(&mut T, R, &mut Window, &mut Context<T>) + 'static,
) where
    T: 'static,
    R: Send + 'static,
    F: Future<Output = R> + Send + 'static,
{
    let handle = tokio().spawn(future);
    cx.spawn_in(window, async move |this, window| {
        // JoinHandle 是一个普通 future，GPUI 的执行器可以等它。
        let Ok(result) = handle.await else {
            return;
        };
        let _ = this.update_in(window, move |view, window, cx| {
            done(view, result, window, cx)
        });
    })
    .detach();
}

/// [`run_async`] 的无窗口版本：实体上下文（应用级 store）用。
pub fn run_async_in<T, R, F>(
    cx: &mut Context<T>,
    future: F,
    done: impl FnOnce(&mut T, R, &mut Context<T>) + 'static,
) where
    T: 'static,
    R: Send + 'static,
    F: Future<Output = R> + Send + 'static,
{
    let handle = tokio().spawn(future);
    cx.spawn(async move |this, cx| {
        let Ok(result) = handle.await else {
            return;
        };
        let _ = this.update(cx, move |view, cx| done(view, result, cx));
    })
    .detach();
}

/// 把 core 的事件送进通道。交给 `services::generation::*` 的 `Arc<dyn EventSink>`。
/// 可以克隆：应用级 store 每次发起生成 / 重试都拿一份。
#[derive(Clone)]
pub struct ChannelSink(pub mpsc::UnboundedSender<CoreEvent>);

impl EventSink for ChannelSink {
    fn emit(&self, event: CoreEvent) {
        let _ = self.0.unbounded_send(event);
    }
}

/// 建一个事件通道，并在界面线程上逐条处理事件，直到发送端全部丢弃或视图被丢弃。
pub fn listen_events<T: 'static>(
    window: &mut Window,
    cx: &mut Context<T>,
    on_event: impl Fn(&mut T, CoreEvent, &mut Window, &mut Context<T>) + 'static,
) -> ChannelSink {
    let (sender, mut receiver) = mpsc::unbounded::<CoreEvent>();
    cx.spawn_in(window, async move |this, window| {
        while let Some(event) = receiver.next().await {
            if this
                .update_in(window, |view, window, cx| on_event(view, event, window, cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    ChannelSink(sender)
}

/// [`listen_events`] 的无窗口版本：实体上下文（应用级 store）用。
pub fn listen_events_in<T: 'static>(
    cx: &mut Context<T>,
    on_event: impl Fn(&mut T, CoreEvent, &mut Context<T>) + 'static,
) -> ChannelSink {
    let (sender, mut receiver) = mpsc::unbounded::<CoreEvent>();
    cx.spawn(async move |this, cx| {
        while let Some(event) = receiver.next().await {
            if this
                .update(cx, |view, cx| on_event(view, event, cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
    ChannelSink(sender)
}
