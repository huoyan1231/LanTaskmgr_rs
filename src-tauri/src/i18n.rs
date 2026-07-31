//! 多语言文案。
//!
//! 原程序把语言放在 Languages/*.xml 里，运行时反序列化；缺文件就会退化成英文。
//! 这里直接编译进二进制，避免分发时丢文件，键名与原程序保持一致。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Lang {
    /// 语言代码，会被注入到手机端 script.js 的 WEBLANGUAGE 占位符
    pub name: &'static str,
    /// 下拉框里显示的名字
    pub display: &'static str,

    pub title: &'static str,
    pub settings: &'static str,
    pub language: &'static str,
    pub port: &'static str,
    pub password: &'static str,
    pub save_and_restart: &'static str,
    pub server: &'static str,
    pub working: &'static str,
    pub closed: &'static str,
    pub restart: &'static str,
    pub exits: &'static str,
    pub password_cant_be: &'static str,
    pub restart_successfully: &'static str,
    pub fail_to_start: &'static str,
    pub add_auto_start: &'static str,
    pub auto_start_on: &'static str,
    pub auto_start_off: &'static str,
    pub connect: &'static str,
    pub open_this: &'static str,
    pub add_favor: &'static str,
    pub block: &'static str,
    pub help: &'static str,
    pub about: &'static str,
    pub about_url: &'static str,
    pub guide: &'static str,
    pub guide_url: &'static str,
    pub troubleshoot: &'static str,
    pub troubleshoot_url: &'static str,
    pub open_settings: &'static str,
    pub network_card: &'static str,
    pub copy: &'static str,
    pub copied: &'static str,
    pub port_range_hint: &'static str,
    pub firewall_hint: &'static str,
    pub lightweight: &'static str,
    pub lightweight_hint: &'static str,
}

pub const EN: Lang = Lang {
    name: "EN",
    display: "English",
    title: "Run Task Manager On Your Phone",
    settings: "Settings",
    language: "Language",
    port: "Port",
    password: "Password",
    save_and_restart: "Save settings and restart service",
    server: "Service",
    working: "working",
    closed: "closed",
    restart: "Restart Service",
    exits: "Exit",
    password_cant_be: "Password can't be empty!",
    restart_successfully: "Restart service successfully.",
    fail_to_start: "Unable to start http server. Reason:",
    add_auto_start: "Start with Windows",
    auto_start_on: "Enabled",
    auto_start_off: "Disabled",
    connect: "Connect",
    open_this: "Open this in your phone's browser:",
    add_favor: "Add this page to your phone browser's favorites, and let this app start with Windows. Hope it helps when your PC is in trouble.",
    block: "You're now blocked. If you want to unblock, please restart service on your PC.",
    help: "Help",
    about: "About",
    about_url: "https://github.com/gordonwalkedby/RunTaskManagerOnYourPhone",
    guide: "Guide to use",
    guide_url: "https://github.com/gordonwalkedby/RunTaskManagerOnYourPhone/wiki/Guide-to-use",
    troubleshoot: "Troubleshoot connection",
    troubleshoot_url:
        "https://github.com/gordonwalkedby/RunTaskManagerOnYourPhone/wiki/Troubleshoot-connection",
    open_settings: "Open settings",
    network_card: "Network adapter",
    copy: "Copy",
    copied: "Copied",
    port_range_hint: "Valid range 1-65535, ports below 1024 may need admin rights.",
    firewall_hint: "If your phone can't connect, allow this app through Windows Firewall on a private network.",
    lightweight: "Lightweight mode",
    lightweight_hint: "When on, closing the window fully stops the app window to free resources. The service keeps running for your phone; open it again from the tray.",
};

pub const CN: Lang = Lang {
    name: "CN",
    display: "简体中文",
    title: "手机任务管理器",
    settings: "设置",
    language: "语言",
    port: "端口",
    password: "密码",
    save_and_restart: "保存以上并重启服务",
    server: "服务",
    working: "工作中",
    closed: "已关闭",
    restart: "重启服务",
    exits: "退出",
    password_cant_be: "密码不能为空！",
    restart_successfully: "成功重启服务！",
    fail_to_start: "无法启动 HTTP 服务器。原因：",
    add_auto_start: "开机自动启动",
    auto_start_on: "已开启",
    auto_start_off: "已关闭",
    connect: "连接",
    open_this: "在你的手机浏览器里打开这个页面：",
    add_favor: "最好把这个页面添加到你手机浏览器的收藏夹里，并设置软件开机自启。希望它可以在关键时刻帮你一把。",
    block: "你已经被屏蔽。如果要取消屏蔽，请在你的电脑上重启服务。",
    help: "帮助",
    about: "关于",
    about_url: "https://walkedby.com/runtaskmanageronyourphone/",
    guide: "使用指南",
    guide_url: "https://github.com/gordonwalkedby/RunTaskManagerOnYourPhone/wiki/%E4%BD%BF%E7%94%A8%E6%8C%87%E5%8D%97",
    troubleshoot: "检查连接问题",
    troubleshoot_url: "https://github.com/gordonwalkedby/RunTaskManagerOnYourPhone/wiki/%E6%A3%80%E6%9F%A5%E8%BF%9E%E6%8E%A5%E9%97%AE%E9%A2%98",
    open_settings: "打开设置",
    network_card: "网卡",
    copy: "复制",
    copied: "已复制",
    port_range_hint: "有效范围 1-65535，低于 1024 的端口可能需要管理员权限。",
    firewall_hint: "如果手机连不上，请在 Windows 防火墙里允许本程序在专用网络上通信。",
    lightweight: "轻量模式",
    lightweight_hint: "开启后，关闭窗口会彻底停止应用窗口以释放资源。服务仍为手机运行，可从托盘重新打开。",
};

pub const ZHTW: Lang = Lang {
    name: "ZHTW",
    display: "繁體中文",
    title: "手機工作管理員",
    settings: "設定",
    language: "語言",
    port: "連接埠",
    password: "密碼",
    save_and_restart: "儲存以上並重新啟動服務",
    server: "服務",
    working: "工作中",
    closed: "已關閉",
    restart: "重新啟動服務",
    exits: "結束",
    password_cant_be: "密碼不能為空！",
    restart_successfully: "成功重新啟動服務！",
    fail_to_start: "無法啟動 HTTP 伺服器。原因：",
    add_auto_start: "開機自動啟動",
    auto_start_on: "已開啟",
    auto_start_off: "已關閉",
    connect: "連線",
    open_this: "在你的手機瀏覽器裡開啟這個頁面：",
    add_favor: "最好把這個頁面加入你手機瀏覽器的書籤，並設定軟體開機自動啟動。希望它可以在關鍵時刻幫你一把。",
    block: "你已經被封鎖。如果要解除封鎖，請在你的電腦上重新啟動服務。",
    help: "說明",
    about: "關於",
    about_url: "https://walkedby.com/runtaskmanageronyourphone/",
    guide: "使用指南",
    guide_url: "https://github.com/gordonwalkedby/RunTaskManagerOnYourPhone/wiki/%E4%BD%BF%E7%94%A8%E6%8C%87%E5%8D%97",
    troubleshoot: "檢查連線問題",
    troubleshoot_url: "https://github.com/gordonwalkedby/RunTaskManagerOnYourPhone/wiki/%E6%A3%80%E6%9F%A5%E8%BF%9E%E6%8E%A5%E9%97%AE%E9%A2%98",
    open_settings: "開啟設定",
    network_card: "網路卡",
    copy: "複製",
    copied: "已複製",
    port_range_hint: "有效範圍 1-65535，低於 1024 的連接埠可能需要系統管理員權限。",
    firewall_hint: "如果手機連不上，請在 Windows 防火牆裡允許本程式在專用網路上通訊。",
    lightweight: "輕量模式",
    lightweight_hint: "開啟後，關閉視窗會徹底停止應用視窗以釋放資源。服務仍為手機運行，可從托盤重新開啟。",
};

pub const ALL: [&Lang; 3] = [&EN, &CN, &ZHTW];

pub fn get(code: &str) -> &'static Lang {
    match code {
        "CN" => &CN,
        "ZHTW" => &ZHTW,
        _ => &EN,
    }
}

pub fn is_known_language(code: &str) -> bool {
    matches!(code, "EN" | "CN" | "ZHTW")
}

/// 根据系统区域猜一个默认语言。
pub fn detect_system_language() -> &'static str {
    let raw = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default()
        .to_ascii_lowercase();

    #[cfg(windows)]
    let raw = if raw.is_empty() {
        windows_ui_language().to_ascii_lowercase()
    } else {
        raw
    };

    if raw.starts_with("zh") {
        if raw.contains("tw") || raw.contains("hk") || raw.contains("mo") || raw.contains("hant") {
            return "ZHTW";
        }
        return "CN";
    }
    "EN"
}

#[cfg(windows)]
fn windows_ui_language() -> String {
    // 不额外引依赖，直接问 PowerShell 太重，这里用 registry 里的 locale name
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Control Panel\\International")
        .ok()
        .and_then(|k| k.get_value::<String, _>("LocaleName").ok())
        .unwrap_or_default()
}
