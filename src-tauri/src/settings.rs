//! 设置持久化（port / password / language / autostart）。
//!
//! 原程序用 settings.xml（VB 的 XmlSerializer），存到 exe 同目录。
//! 这里改存到「应用数据目录」，对安装到 Program Files 的程序更合适，也避免写 exe 目录没权限。

use crate::i18n;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_password")]
    pub password: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub autostart: bool,
    /// 绑定地址，默认 0.0.0.0（监听所有网卡，手机才能访问）。
    #[serde(default = "default_bind")]
    pub bind: String,
    /// 轻量模式：关闭前台窗口时彻底销毁 WebView 窗口以释放资源，
    /// HTTP 服务继续运行（手机仍可访问），从托盘可重新打开窗口。
    #[serde(default)]
    pub lightweight: bool,
}

fn default_password() -> String {
    // 默认无密码：手机端直接登录即可（用户可在桌面设置里自行设定密码）
    String::new()
}

fn default_port() -> u16 {
    5555
}

fn default_language() -> String {
    i18n::detect_system_language().to_string()
}

fn default_bind() -> String {
    "0.0.0.0".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            password: default_password(),
            port: default_port(),
            language: default_language(),
            autostart: false,
            bind: default_bind(),
            lightweight: false,
        }
    }
}

/// 应用数据目录：Windows 用 %APPDATA%/LanTaskmgr，其它回退到 exe 所在目录。
pub fn data_dir() -> PathBuf {
    if let Some(dir) = dirs_app_data() {
        let p = dir.join("LanTaskmgr");
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    // 回退：exe 同目录
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(windows)]
fn dirs_app_data() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(not(windows))]
fn dirs_app_data() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// 是否已经存在设置文件（用于判断「首次启动」）。
#[allow(dead_code)]
pub fn exists() -> bool {
    settings_path().exists()
}

pub fn load() -> Settings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<Settings>(&s) {
            Ok(mut s) => {
                if !i18n::is_known_language(&s.language) {
                    s.language = default_language();
                }
                s
            }
            Err(e) => {
                crate::logger::log(format!("读取设置失败，使用默认：{e}"));
                Settings::default()
            }
        },
        Err(_) => Settings::default(),
    }
}

pub fn save(s: &Settings) {
    let path = settings_path();
    match serde_json::to_string_pretty(s) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                crate::logger::log(format!("写入设置失败：{e}"));
            }
        }
        Err(e) => crate::logger::log(format!("序列化设置失败：{e}")),
    }
}

/// 生成一个随机测试密码（仅在「首次启动」时展示给用户）。
#[allow(dead_code)]
pub fn random_password() -> String {
    default_password()
}

/// 把明文密码哈希成可存储的字符串（salt 内嵌在结果里）。
/// 空串表示「未设置密码」，调用方应直接存空串、不要哈希。
pub fn hash_password(plain: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    match argon2.hash_password(plain.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        // 理论上不会失败；兜底返回不可登录的占位串（verify 必失败）
        Err(_) => "$argon2id$v=19$m=19456,t=2,p=1$$error".to_string(),
    }
}

/// 恒定时间校验明文密码与存储哈希是否匹配（防止计时侧信道）。
pub fn verify_password(stored_hash: &str, plain: &str) -> bool {
    use argon2::password_hash::PasswordVerifier;
    let Ok(parsed) = argon2::password_hash::PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}
