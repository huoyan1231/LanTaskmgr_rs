//! 轻量模式：窗口关闭后**立刻**销毁 WebView，并回收全部 `msedgewebview2` 进程。
//!
//! ## 为什么要主动杀进程
//! Tauri / wry 在窗口销毁时会走 WebView2 的正常释放路径（`ICoreWebView2Controller::Close`），
//! 但实测下来 browser / renderer / GPU / utility 这些 `msedgewebview2.exe` 子进程
//! **不会立即退出**，反复开关窗口就会不断堆积。
//! 因此轻量模式下由我们自己兜底：窗口一关就把本进程树下的 WebView2 进程全部结束。
//!
//! ## 流程
//! 1. 关窗时**不拦截**（让窗口真正销毁，先走一遍 WebView2 的正常释放，尽量干净）；
//! 2. `GRACE_MS` 后开始扫描，结束本进程树下残留的 `msedgewebview2`；
//! 3. 再复扫几轮兜底（WebView2 的子进程是分批退出的）；
//! 4. 任一轮发现窗口已被重新打开，立即停止清理，避免误杀新 WebView。
//!
//! 程序本体与手机端 HTTP 服务全程不受影响（`RunEvent::ExitRequested` 里 `prevent_exit()`）。

use std::sync::atomic::{AtomicU8, Ordering};
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, Duration};

/// 等待窗口真正从 WindowManager 摘除的最大轮数 / 间隔。
const WAIT_GONE_ROUNDS: usize = 20;
const WAIT_GONE_INTERVAL_MS: u64 = 50;
/// 窗口消失后等多久开始清理。留一点时间让 WebView2 自己正常退出，避免留下崩溃记录。
const GRACE_MS: u64 = 150;
/// 兜底复扫轮数（WebView2 子进程分批退出，一轮往往扫不干净）。
const SWEEP_ROUNDS: usize = 4;
/// 每轮之间的间隔。
const SWEEP_INTERVAL_MS: u64 = 350;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LightweightState {
    /// 窗口存活。
    Normal = 0,
    /// 已进入轻量态：窗口已销毁，只剩托盘 + HTTP 服务。
    In = 1,
}

impl From<u8> for LightweightState {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::In,
            _ => Self::Normal,
        }
    }
}

impl LightweightState {
    const fn as_u8(self) -> u8 {
        self as u8
    }
}

static LIGHTWEIGHT_STATE: AtomicU8 = AtomicU8::new(LightweightState::Normal as u8);

#[inline]
fn set_state(s: LightweightState) {
    LIGHTWEIGHT_STATE.store(s.as_u8(), Ordering::Release);
}

/// 当前是否处于「窗口已销毁」的轻量态。
#[allow(dead_code)]
#[inline]
pub fn is_in_lightweight_mode() -> bool {
    LightweightState::from(LIGHTWEIGHT_STATE.load(Ordering::Acquire)) == LightweightState::In
}

#[inline]
fn is_exiting() -> bool {
    crate::app_state()
        .exiting
        .load(std::sync::atomic::Ordering::SeqCst)
}

/// 轻量模式下窗口已销毁：进入轻量态，并立刻回收 WebView2 进程。
pub fn on_window_destroyed(app: &AppHandle) {
    if is_exiting() {
        return;
    }
    set_state(LightweightState::In);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // 等窗口真正从 WindowManager 中摘掉（Destroyed 回调可能早于摘除）
        for _ in 0..WAIT_GONE_ROUNDS {
            if app.get_webview_window("main").is_none() {
                break;
            }
            sleep(Duration::from_millis(WAIT_GONE_INTERVAL_MS)).await;
        }
        // 还在，说明关闭被取消或窗口又被打开了 —— 放弃清理
        if app.get_webview_window("main").is_some() {
            return;
        }

        sleep(Duration::from_millis(GRACE_MS)).await;

        // 子孙树在窗口已销毁后基本不变，整轮清理只算一次，复扫直接复用
        let mut reaper = crate::process::prepare_webview_reaper();
        let mut total = 0usize;
        for round in 0..SWEEP_ROUNDS {
            // 程序要退出了，或窗口已被重新打开 —— 立刻停手，别误杀新 WebView
            if is_exiting() || app.get_webview_window("main").is_some() {
                break;
            }
            total += reaper.kill_round();
            if round + 1 < SWEEP_ROUNDS {
                sleep(Duration::from_millis(SWEEP_INTERVAL_MS)).await;
            }
        }

        let left = reaper.count_remaining();
        crate::logger::log(format!(
            "轻量模式：窗口已关闭，结束 {total} 个 WebView2 进程，残留 {left} 个。"
        ));
    });
}

/// 窗口重新打开（显示或重建）时调用：回到 Normal 状态。
pub fn notify_window_opened() {
    set_state(LightweightState::Normal);
}

/// 程序退出前调用，避免清理任务在退出过程中继续动作。
pub fn shutdown() {
    set_state(LightweightState::Normal);
}
