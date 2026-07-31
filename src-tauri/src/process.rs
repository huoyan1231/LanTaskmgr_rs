//! 进程枚举、窗口标题获取、以及按名字结束进程。
//!
//! 原程序用 .NET 的 Process.GetProcesses() 拿进程名和 MainWindowTitle。
//! 这里用 sysinfo 拿进程（跨平台），用 Win32 的 EnumWindows 拿窗口标题映射到 pid；
//! 结束进程也走 Win32 TerminateProcess，确保连没有主窗口的后台进程也能杀掉。

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

/// 手机端 manager 页面里的一个进程条目。字段名与原参考实现（D:/git/LanTaskmgr/web/app.js）一一对应。
#[derive(Debug, Clone, Serialize)]
pub struct ProcInfo {
    /// 镜像名（含 .exe）
    pub n: String,
    /// 主窗口标题（没有则空串）
    #[serde(default)]
    pub t: String,
    /// 占用物理内存（字节），同名下多个实例累加
    pub m: u64,
    /// CPU 占用百分比，同名下多个实例累加
    pub p: f32,
    /// 受保护（不可结束）。这里统一为 false：系统进程仅警告、不禁止结束
    pub k: bool,
    /// 分类：0=其它 1=有窗口的应用 2=系统进程
    pub c: u8,
    /// 同名实例个数
    pub i: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemInfo {
    pub pct: u8,
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListResponse {
    pub mem: MemInfo,
    pub list: Vec<ProcInfo>,
}

/// 原程序的系统进程名单（保持完全一致，便于行为对照）。
const SYSTEM_PROCESSES: [&str; 34] = [
    "system",
    "svchost",
    "surfaceservice",
    "surfacedtx",
    "surfacedtxservice",
    "surfaceservice",
    "startmenuexperiencehost",
    "spoolsv",
    "smss",
    "services",
    "searchui",
    "searchprotocolhost",
    "searchindexer",
    "pacjsworker",
    "nvdisplay.container",
    "lsass",
    "lsaiso",
    "intelcphecisvc",
    "intelcphdcpsvc",
    "idle",
    "explorer",
    "dwm",
    "ctfmon",
    "dllhost",
    "csrss",
    "shellexperiencehost",
    "vmms",
    "wininit",
    "vmcompute",
    "wmiprvse",
    "applicationframehost",
    "runtimebroker",
    "searchfilterhost",
    "windowsinternal.composableshell.experiences.textinput.inputapp",
];

fn is_system_process(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SYSTEM_PROCESSES.contains(&lower.as_str())
}

/// 收集当前进程列表，按镜像名聚合，并补上窗口标题、内存、CPU 等。
pub fn list() -> ListResponse {
    let titles = window_titles();
    let mut sys = sysinfo::System::new();
    // 先刷新一次全局 CPU，这样后续 per-process 的 cpu_usage() 才有意义
    sys.refresh_cpu_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut groups: HashMap<String, ProcInfo> = HashMap::new();

    for (pid, proc_) in sys.processes() {
        let name = proc_.name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }
        let lower = name.to_ascii_lowercase();

        let entry = groups.entry(lower.clone()).or_insert_with(|| ProcInfo {
            n: name.clone(),
            t: String::new(),
            m: 0,
            p: 0.0,
            k: false,
            c: 0,
            i: 0,
        });
        entry.i += 1;
        entry.m += proc_.memory();
        entry.p += proc_.cpu_usage();
        if entry.t.is_empty() {
            if let Some(title) = titles.get(&pid.as_u32()) {
                if !title.is_empty() {
                    entry.t = title.clone();
                }
            }
        }
    }

    let mut list: Vec<ProcInfo> = groups.into_values().collect();
    for p in list.iter_mut() {
        let sys_p = is_system_process(&p.n);
        p.c = if sys_p {
            2
        } else if !p.t.is_empty() {
            1
        } else {
            0
        };
    }
    list.sort_by(|a, b| a.n.to_lowercase().cmp(&b.n.to_lowercase()));

    let total = sys.total_memory();
    let used = sys.used_memory();
    let pct = if total > 0 {
        ((used as f64 / total as f64) * 100.0).round() as u8
    } else {
        0
    };

    ListResponse {
        mem: MemInfo { pct, used, total },
        list,
    }
}

/// 结束与给定名字相同的所有进程（去掉 .exe 后缀，与原程序 Process.GetProcessesByName 对齐）。
/// 返回是否「至少匹配到一个进程且全部结束成功」；名字不存在返回 false。
pub fn kill_by_name(name: &str) -> bool {
    let target = name.to_ascii_lowercase();
    let target = target.strip_suffix(".exe").unwrap_or(&target);

    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut found = false;
    let mut failed = false;
    for (_pid, proc_) in sys.processes() {
        let pname = proc_.name().to_string_lossy().to_string();
        let pname = pname.to_ascii_lowercase();
        let pname = pname.strip_suffix(".exe").unwrap_or(&pname);
        if pname == target {
            found = true;
            if !proc_.kill() {
                failed = true;
            }
        }
    }
    // 至少要有一个匹配进程，且全部杀掉才算成功；名字不存在应返回失败
    found && !failed
}

// ---------------- WebView2 进程回收（轻量模式用） ----------------

/// 本程序 WebView2 的 user data folder 标识。
///
/// Tauri 在 Windows 上默认把 WebView2 的数据目录放在
/// `%LOCALAPPDATA%\{identifier}\EBWebView`，于是**每一个** `msedgewebview2.exe`
/// 子进程（browser / renderer / gpu / utility / crashpad）的命令行里都会带上
/// `--user-data-dir=...\com.lantaskmgr.rs\EBWebView`。
/// 这是识别「哪些 WebView2 进程属于我们」最可靠的依据。
///
/// ⚠️ 必须与 `tauri.conf.json` 里的 `identifier` 保持一致。
const WEBVIEW_DATA_MARKER: &str = "com.lantaskmgr.rs";

/// 判断一个进程是否是「属于本程序的」WebView2 进程。
///
/// 两条判据取并集：
/// 1. 命令行里带我们独占的 user data folder —— 能抓到**孤儿进程**
///    （上次实例崩溃/被强杀后残留的 browser process，下次启动会被复用，是进程堆积的元凶）；
/// 2. 在当前进程的子孙树里 —— 兜底，防止某些情况下读不到命令行。
///
/// 绝不会误伤 Edge 浏览器，或别的应用（向日葵 GameViewer、VSCode、QQ 等）的 WebView2。
fn is_our_webview(proc_: &sysinfo::Process, descendants: &std::collections::HashSet<sysinfo::Pid>, pid: sysinfo::Pid) -> bool {
    let name = proc_.name().to_string_lossy().to_ascii_lowercase();
    if !name.starts_with("msedgewebview2") {
        return false;
    }
    let by_cmdline = proc_
        .cmd()
        .iter()
        .any(|arg| arg.to_string_lossy().contains(WEBVIEW_DATA_MARKER));
    by_cmdline || descendants.contains(&pid)
}

/// 收集当前进程的所有子孙 pid。
fn own_descendants(sys: &sysinfo::System) -> std::collections::HashSet<sysinfo::Pid> {
    let self_pid = sysinfo::Pid::from_u32(std::process::id());

    let mut children: HashMap<sysinfo::Pid, Vec<sysinfo::Pid>> = HashMap::new();
    for (pid, proc_) in sys.processes() {
        if let Some(parent) = proc_.parent() {
            children.entry(parent).or_default().push(*pid);
        }
    }

    let mut set = std::collections::HashSet::new();
    let mut queue: Vec<sysinfo::Pid> = vec![self_pid];
    let mut guard = 0usize;
    while let Some(pid) = queue.pop() {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        if let Some(kids) = children.get(&pid) {
            for k in kids {
                if set.insert(*k) {
                    queue.push(*k);
                }
            }
        }
    }
    set
}

fn snapshot() -> sysinfo::System {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys
}

/// 结束所有属于本程序的 `msedgewebview2` 进程，返回结束掉的个数。
///
/// 轻量模式关窗后要立刻把 WebView2 占的内存还回去 —— 实测 Tauri / wry 走正常释放路径时
/// 这些子进程不会马上退出，所以由我们主动清理。
pub fn kill_own_webview_processes() -> usize {
    let sys = snapshot();
    let descendants = own_descendants(&sys);

    let mut killed = 0usize;
    for (pid, proc_) in sys.processes() {
        if is_our_webview(proc_, &descendants, *pid) && proc_.kill() {
            killed += 1;
        }
    }
    killed
}

/// 还剩多少个属于本程序的 `msedgewebview2` 进程（用于日志 / 判断是否清干净）。
pub fn count_own_webview_processes() -> usize {
    let sys = snapshot();
    let descendants = own_descendants(&sys);
    sys.processes()
        .iter()
        .filter(|(pid, proc_)| is_our_webview(proc_, &descendants, **pid))
        .count()
}

// ---------------- Windows 窗口标题枚举 ----------------

#[cfg(windows)]
fn window_titles() -> HashMap<u32, String> {
    let titles: Mutex<HashMap<u32, String>> = Mutex::new(HashMap::new());

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let titles = &*(lparam.0 as *const Mutex<HashMap<u32, String>>);

        if IsWindowVisible(hwnd).as_bool() {
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 {
                let text = String::from_utf16_lossy(&buf[..len as usize]);
                if !text.is_empty() {
                    let mut pid: u32 = 0;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                    if pid != 0 {
                        titles.lock().unwrap().insert(pid, text);
                    }
                }
            }
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(Some(callback), LPARAM(&titles as *const _ as isize));
    }

    titles.into_inner().unwrap()
}

#[cfg(not(windows))]
fn window_titles() -> HashMap<u32, String> {
    HashMap::new()
}
