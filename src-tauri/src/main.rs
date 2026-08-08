// 手机任务管理器（Rust + Tauri 重写版）
// 原程序：RunTaskManagerOnYourPhone（VB.NET / .NET Framework 4.7.2）

#![windows_subsystem = "windows"]

mod i18n;
mod logger;
mod netinfo;
mod process;
mod qrcode;
mod server;
mod settings;
mod web;
mod lightweight;

/// 当前是否开启了轻量模式（给 lightweight 模块读取设置用）。
#[inline]
pub(crate) fn lightweight_enabled() -> bool {
    app_state().settings.lock().unwrap().lightweight
}

use i18n::Lang;
use serde::Serialize;
use std::sync::{atomic::AtomicBool, Arc, Mutex, OnceLock};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

/// 全局状态句柄。命令/托盘回调里拿不到 AppHandle 的生命周期时，用它访问共享状态。
static APP_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

fn app_state() -> &'static Arc<AppState> {
    APP_STATE
        .get()
        .expect("AppState 尚未初始化（不应发生）")
}

/// 桌面 UI 与后台服务之间共享的状态。
struct AppState {
    server: Arc<server::ServerHandle>,
    settings: Mutex<settings::Settings>,
    tray: Mutex<Option<tauri::tray::TrayIcon>>,
    /// 是否正在主动退出（仅托盘「退出」时置位，用于放行 ExitRequested）。
    exiting: AtomicBool,
}

#[derive(Serialize)]
struct NetItem {
    ip: String,
    desc: String,
}

#[derive(Serialize)]
struct Snapshot {
    title: String,
    server_working: bool,
    port: u16,
    password: String,
    language: String,
    languages: Vec<LangInfo>,
    autostart: bool,
    lightweight: bool,
    bind: String,
    addresses: Vec<NetItem>,
    url: String,
    qr: String,
    messages: Messages,
}

#[derive(Serialize)]
struct LangInfo {
    code: String,
    display: String,
}

#[derive(Serialize)]
struct Messages {
    save_and_restart: String,
    server: String,
    working: String,
    closed: String,
    restart: String,
    exits: String,
    open_settings: String,
    port_range_hint: String,
    firewall_hint: String,
    open_this: String,
    add_favor: String,
    connect: String,
    network_card: String,
    copy: String,
    copied: String,
    add_auto_start: String,
    auto_start_on: String,
    auto_start_off: String,
    lightweight: String,
    lightweight_hint: String,
    bind: String,
    bind_hint: String,
}

fn lang_info() -> Vec<LangInfo> {
    i18n::ALL
        .iter()
        .map(|l| LangInfo {
            code: l.name.to_string(),
            display: l.display.to_string(),
        })
        .collect()
}

fn current_lang(code: &str) -> &'static Lang {
    i18n::get(code)
}

/// 规范化绑定地址：空或非法 IPv4 一律回退到 0.0.0.0（监听所有网卡）。
/// 这样即使误填公网域名/错误格式也不会把服务绑到不可预期的地方。
fn normalize_bind(bind: &str) -> String {
    let b = bind.trim();
    if b.is_empty() {
        return "0.0.0.0".to_string();
    }
    // 仅接受合法的 IPv4 地址（不含端口、不含通配符以外的字符）
    if b.parse::<std::net::Ipv4Addr>().is_ok() {
        b.to_string()
    } else {
        "0.0.0.0".to_string()
    }
}

#[tauri::command]
fn snapshot() -> Snapshot {
    let state = app_state();
    let s = state.settings.lock().unwrap().clone();
    let lang = current_lang(&s.language);
    let working = state.server.is_running();
    let addresses: Vec<NetItem> = netinfo::lan_addresses()
        .into_iter()
        .map(|(ip, desc)| NetItem { ip, desc })
        .collect();
    let url = if let Some(first) = addresses.first() {
        format!("http://{}:{}/", first.ip, s.port)
    } else {
        format!("http://127.0.0.1:{}/", s.port)
    };
    let qr = qrcode::svg(&url);
    let messages = Messages {
        save_and_restart: lang.save_and_restart.to_string(),
        server: lang.server.to_string(),
        working: lang.working.to_string(),
        closed: lang.closed.to_string(),
        restart: lang.restart.to_string(),
        exits: lang.exits.to_string(),
        open_settings: lang.open_settings.to_string(),
        port_range_hint: lang.port_range_hint.to_string(),
        firewall_hint: lang.firewall_hint.to_string(),
        open_this: lang.open_this.to_string(),
        add_favor: lang.add_favor.to_string(),
        connect: lang.connect.to_string(),
        network_card: lang.network_card.to_string(),
        copy: lang.copy.to_string(),
        copied: lang.copied.to_string(),
        add_auto_start: lang.add_auto_start.to_string(),
        auto_start_on: lang.auto_start_on.to_string(),
        auto_start_off: lang.auto_start_off.to_string(),
        lightweight: lang.lightweight.to_string(),
        lightweight_hint: lang.lightweight_hint.to_string(),
        bind: lang.bind.to_string(),
        bind_hint: lang.bind_hint.to_string(),
    };
    Snapshot {
        title: lang.title.to_string(),
        server_working: working,
        port: s.port,
        password: s.password,
        language: s.language,
        languages: lang_info(),
        autostart: s.autostart,
        lightweight: s.lightweight,
        bind: s.bind,
        addresses,
        url,
        qr,
        messages,
    }
}

#[tauri::command]
fn save_settings(password: String, port: u16, language: String, autostart: bool, lightweight: bool, bind: String) -> String {
    let state = app_state();
    let lang = current_lang(&language);
    let bind = normalize_bind(&bind);
    {
        let mut s = state.settings.lock().unwrap();
        s.password = password;
        s.port = port;
        s.language = language.clone();
        s.autostart = autostart;
        s.lightweight = lightweight;
        s.bind = bind.clone();
        settings::save(&s);
    }

    set_autostart(autostart);

    let s2 = state.settings.lock().unwrap().clone();
    tauri::async_runtime::spawn({
        let state = state.clone();
        async move {
            state
                .server
                .start(&s2.bind, s2.port, &s2.password, &s2.language)
                .await;
            update_tray(&*state);
        }
    });
    lang.restart_successfully.to_string()
}

#[tauri::command]
fn set_language(language: String) {
    let state = app_state();
    {
        let mut s = state.settings.lock().unwrap();
        s.language = language.clone();
        settings::save(&s);
    }
    update_tray(&state);
}

#[tauri::command]
fn restart_service() {
    let state = app_state();
    let s = state.settings.lock().unwrap().clone();
    tauri::async_runtime::spawn({
        let state = state.clone();
        async move {
            state
                .server
                .start(&s.bind, s.port, &s.password, &s.language)
                .await;
            update_tray(&*state);
        }
    });
}

#[tauri::command]
fn open_external(url: String) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;

        // 用系统默认程序打开（通常是浏览器）。直接走 ShellExecute，避免 `cmd /c start`
        // 对含特殊字符/引号的 URL 产生歧义或注入风险。
        let url_w: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        let open_w: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = ShellExecuteW(
                None,
                PCWSTR::from_raw(open_w.as_ptr()),
                PCWSTR::from_raw(url_w.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
        }
    }
    #[cfg(not(windows))]
    {
        if let Err(e) = std::process::Command::new("xdg-open").arg(&url).spawn() {
            logger::log(format!("打开外部链接失败 {url}: {e}"));
        }
    }
}

fn set_autostart(enable: bool) {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Run", KEY_WRITE);
        if let Ok(key) = key {
            if enable {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = key.set_value("LanTaskmgr_rs", &exe.to_string_lossy().to_string());
                }
            } else {
                let _ = key.delete_value("LanTaskmgr_rs");
            }
        }
    }
    let _ = enable;
}

#[allow(dead_code)]
fn get_autostart() -> bool {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
        use winreg::RegKey;
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Run", KEY_READ)
        {
            if let Ok(v) = key.get_value::<String, _>("LanTaskmgr_rs") {
                if let Ok(exe) = std::env::current_exe() {
                    return v == exe.to_string_lossy().to_string();
                }
                return true;
            }
        }
    }
    false
}

/// 根据服务状态切换托盘图标颜色（绿=工作中，红=已停止）与提示文字。
fn update_tray(state: &AppState) {
    let working = state.server.is_running();
    let lang = {
        let s = state.settings.lock().unwrap();
        current_lang(&s.language).clone()
    };
    let tooltip = format!(
        "{} {} {}",
        lang.title,
        lang.server,
        if working { lang.working } else { lang.closed }
    );

    if let Some(tray) = state.tray.lock().unwrap().as_ref() {
        let icon_bytes = if working {
            include_bytes!("../icons/32x32.png").to_vec()
        } else {
            include_bytes!("../icons/32x32_red.png").to_vec()
        };
        if let Ok(img) = Image::from_bytes(&icon_bytes) {
            let _ = tray.set_icon(Some(img));
        }
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn build_tray(app: &tauri::AppHandle, state: Arc<AppState>) -> tauri::Result<()> {
    let lang = {
        let s = state.settings.lock().unwrap();
        current_lang(&s.language).clone()
    };

    let open_item = MenuItem::with_id(app, "open", lang.open_settings, true, None::<&str>)?;
    let restart_item = MenuItem::with_id(app, "restart", lang.restart, true, None::<&str>)?;
    let exit_item = MenuItem::with_id(app, "exit", lang.exits, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &restart_item, &exit_item])?;

    let icon_bytes = include_bytes!("../icons/32x32.png").to_vec();
    let icon = Image::from_bytes(&icon_bytes).expect("内置图标损坏");

    let tooltip = format!("{} {}", lang.title, lang.server);

    let tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let app_owned = app.clone();
            let id = event.id.as_ref().to_string();
            match id.as_str() {
                "open" => {
                    show_main_window(&app_owned);
                }
                "restart" => {
                    let state = app_state().clone();
                    let s = state.settings.lock().unwrap().clone();
                    tauri::async_runtime::spawn(async move {
                        state.server.start(&s.bind, s.port, &s.password, &s.language).await;
                        update_tray(&*state);
                    });
                }
                "exit" => {
                    logger::log("程序退出。");
                    app_state().exiting.store(true, std::sync::atomic::Ordering::SeqCst);
                    lightweight::shutdown();
                    app_owned.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    *state.tray.lock().unwrap() = Some(tray);
    Ok(())
}

/// 给指定窗口挂上关闭处理：
/// - **轻量模式**：放行关闭（窗口真正销毁），随后立刻回收本进程树下所有 `msedgewebview2`
///   进程，把内存还给系统；程序与手机端服务靠 `RunEvent::ExitRequested` 的 `prevent_exit()` 保留，
///   从托盘可随时重建窗口；
/// - **普通模式**：拦截关闭并隐藏，最小化到托盘（WebView 保留，下次秒开）。
fn attach_close_handler(win: &tauri::WebviewWindow) {
    let win_clone = win.clone();
    let app_handle = win.app_handle().clone();
    win.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // 轻量模式不调用 prevent_close，让窗口走一遍 WebView2 的正常销毁流程
            if !lightweight_enabled() {
                api.prevent_close();
                let _ = win_clone.hide();
            }
        }
        tauri::WindowEvent::Destroyed => {
            // 窗口真的没了：兜底结束残留的 msedgewebview2 进程
            if lightweight_enabled() {
                lightweight::on_window_destroyed(&app_handle);
            }
        }
        _ => {}
    });
}

/// 轻量模式下窗口被销毁后，从托盘「打开」时重建窗口。
fn recreate_main_window(app: &tauri::AppHandle, state: &Arc<AppState>) -> tauri::Result<()> {
    let lang = {
        let s = state.settings.lock().unwrap();
        current_lang(&s.language).clone()
    };
    let win = tauri::WebviewWindowBuilder::new(
        app,
        "main",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title(lang.title)
    .inner_size(480.0, 620.0)
    .min_inner_size(420.0, 560.0)
    .resizable(true)
    .center()
    .visible(true)
    .build()?;
    attach_close_handler(&win);
    lightweight::notify_window_opened();
    Ok(())
}

/// 从托盘打开主窗口：窗口还在（普通模式隐藏态）就直接显示；轻量模式下已销毁则重建。
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        lightweight::notify_window_opened();
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    } else {
        // 轻量模式空闲超时后窗口已销毁，此时才重建
        if let Err(e) = recreate_main_window(app, app_state()) {
            logger::log(format!("重建主窗口失败: {e}"));
        }
    }
}

fn main() {
    logger::log("程序启动。");

    // 上一次实例若被强杀 / 崩溃，可能留下孤儿 msedgewebview2 进程；它们会被本次启动的
    // WebView2 复用（同一个 user data folder），是进程越攒越多的元凶。
    // 必须赶在创建窗口之前清掉，否则会误杀自己刚建好的 WebView。
    {
        let stale = process::kill_own_webview_processes();
        if stale > 0 {
            logger::log(format!("启动清理：结束了 {stale} 个上次残留的 WebView2 进程。"));
        }
    }

    let settings0 = settings::load();
    // 启动时按设置里的 autostart 标记同步注册表
    if settings0.autostart {
        set_autostart(true);
    }

    let initial_lang = current_lang(&settings0.language).clone();

    let builder = tauri::Builder::default()
        .setup(move |app| {
            let state = Arc::new(AppState {
                server: Arc::new(server::ServerHandle::new()),
                settings: Mutex::new(settings0.clone()),
                tray: Mutex::new(None),
                exiting: AtomicBool::new(false),
            });
            app.manage(state.clone());
            let _ = APP_STATE.set(state.clone());

            // 先构建托盘（菜单文字用初始语言）
            let _ = build_tray(app.handle(), state.clone());

            // 启动 HTTP 服务
            let s = state.settings.lock().unwrap().clone();
            tauri::async_runtime::spawn({
                let state = state.clone();
                async move {
                    state
                        .server
                        .start(&s.bind, s.port, &s.password, &s.language)
                        .await;
                    update_tray(&*state);
                }
            });

            // 关窗只隐藏、不退出程序；轻量模式下空闲超时才销毁窗口释放资源
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_title(&initial_lang.title);
                // 始终在启动时展示设置窗口
                let _ = win.show();
                attach_close_handler(&win);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            save_settings,
            set_language,
            restart_service,
            open_external
        ]);

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|_app, event| {
        // 关闭最后一个窗口（轻量模式销毁窗口后）会触发退出请求，
        // 除非用户主动从托盘「退出」，否则阻止程序退出，保留托盘与手机服务。
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !app_state().exiting.load(std::sync::atomic::Ordering::SeqCst) {
                api.prevent_exit();
            }
        }
    });
}
