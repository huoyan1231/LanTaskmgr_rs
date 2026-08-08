//! HTTP 服务：手机端访问的入口。逻辑与原程序 Listener 一一对应。
//!
//! 路由（原程序是 Mono.Net.HttpListener，这里用 axum 0.8）：
//!   GET  /app.css        -> 样式
//!   GET  /app.js         -> 注入语言后的脚本
//!   POST /dologin        -> 校验密码（密码放在请求体里），错误 3 次封禁该 IP
//!   POST /list           -> 已登录时返回进程列表 JSON
//!   POST /kill           -> 已登录时结束指定进程（进程名放请求体）
//!   POST /logout         -> 退出当前会话
//!   GET  /              -> 已登录时返回 manager.html，否则返回 login.html
//!   GET  /favicon.ico    -> 空响应
//!
//! 安全模型：
//!   - 登录成功后签发随机 token，通过 Set-Cookie 下发；后续请求优先用 cookie 里的 token
//!     校验（替代纯 IP 鉴权，避免局域网共用/伪造 IP、DHCP 换 IP 导致登录态错乱）。
//!   - 兼容老行为：无 cookie 时退回到按 IP 鉴权（仅用于未带 cookie 的少数情况）。
//!   - 10 分钟无访问后需要重新登录；连续输错 3 次密码永久封禁该 IP，直到手动重启服务。
//!   - 密码以 argon2 哈希形式存储，校验走恒定时间比较。
//!   - 空密码：放行但返回 warning，提示用户设置密码（不禁止 /kill，照顾可能记不住密码的用户）。

use crate::i18n;
use crate::process;
use crate::settings;
use crate::web;
use axum::extract::{ConnectInfo, DefaultBodyLimit, OriginalUri, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use rand::RngCore;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const AUTH_TIMEOUT_SECS: u64 = 10 * 60;
const MAX_WRONG: u32 = 3;
const TOKEN_BYTES: usize = 32;
const TOKEN_MAX_AGE: u64 = AUTH_TIMEOUT_SECS;

pub struct Device {
    authorized: bool,
    last_visit: Instant,
    wrong: u32,
    blocked: bool,
}

impl Device {
    fn new() -> Self {
        Device {
            authorized: false,
            last_visit: Instant::now(),
            wrong: 0,
            blocked: false,
        }
    }
}

pub struct ServerState {
    pub devices: Mutex<HashMap<String, Device>>,
    /// 存储的是 argon2 哈希（空串表示未设置密码）
    pub password_hash: Mutex<String>,
    pub language: Mutex<String>,
    /// token -> (ip, last_visit)
    pub sessions: Mutex<HashMap<String, (String, Instant)>>,
}

impl ServerState {
    pub fn new() -> Self {
        ServerState {
            devices: Mutex::new(HashMap::new()),
            password_hash: Mutex::new(String::new()),
            language: Mutex::new("EN".to_string()),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn device(
        &self,
        ip: &str,
    ) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, Device>> {
        let mut map = self.devices.lock().unwrap();
        // 先确保存在
        if !map.contains_key(ip) {
            map.insert(ip.to_string(), Device::new());
        }
        map
    }

    fn is_authorized_ip(&self, ip: &str) -> bool {
        let mut map = self.device(ip);
        let d = map.get_mut(ip).unwrap();
        if d.authorized && d.last_visit.elapsed().as_secs() > AUTH_TIMEOUT_SECS {
            d.authorized = false;
        }
        d.authorized
    }

    fn set_authorized_ip(&self, ip: &str, v: bool) {
        let mut map = self.device(ip);
        let d = map.get_mut(ip).unwrap();
        d.authorized = v;
        if v {
            d.last_visit = Instant::now();
        }
    }

    fn is_blocked(&self, ip: &str) -> bool {
        let map = self.device(ip);
        map.get(ip).unwrap().blocked
    }

    fn record_wrong(&self, ip: &str) {
        let mut map = self.device(ip);
        let d = map.get_mut(ip).unwrap();
        d.wrong += 1;
        if d.wrong >= MAX_WRONG {
            d.blocked = true;
        }
    }

    /// 生成新 token 并与 IP 绑定。
    fn new_session(&self, ip: &str) -> String {
        let token = random_token();
        self.sessions
            .lock()
            .unwrap()
            .insert(token.clone(), (ip.to_string(), Instant::now()));
        token
    }

    fn touch_session(&self, token: &str) {
        if let Some(v) = self.sessions.lock().unwrap().get_mut(token) {
            v.1 = Instant::now();
        }
    }

    fn logout_session(&self, token: &str) {
        self.sessions.lock().unwrap().remove(token);
    }

    /// 根据 token 返回绑定的 IP；token 过期则清掉并返回 None。
    fn ip_of_token(&self, token: &str) -> Option<String> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some((ip, last)) = sessions.get(token) {
            if last.elapsed() > Duration::from_secs(AUTH_TIMEOUT_SECS) {
                sessions.remove(token);
                return None;
            }
            Some(ip.clone())
        } else {
            None
        }
    }
}

fn random_token() -> String {
    let mut buf = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

pub struct ServerHandle {
    pub port: Mutex<u16>,
    pub running: Mutex<bool>,
    shutdown: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// 旧服务彻底停稳（端口已释放）后发出，保证重启时能重新绑定成功
    done: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    pub state: Arc<ServerState>,
}

impl ServerHandle {
    pub fn new() -> Self {
        ServerHandle {
            port: Mutex::new(0),
            running: Mutex::new(false),
            shutdown: Mutex::new(None),
            done: Mutex::new(None),
            state: Arc::new(ServerState::new()),
        }
    }

    pub async fn start(&self, bind: &str, port: u16, password: &str, language: &str) {
        // 先彻底停掉旧服务（会等待端口释放），否则重启时绑定同一端口会失败
        self.stop().await;

        // 存储哈希：空密码存空串；明文密码先哈希再存
        let hash = if password.is_empty() {
            String::new()
        } else {
            settings::hash_password(password)
        };
        *self.state.password_hash.lock().unwrap() = hash;
        *self.state.language.lock().unwrap() = language.to_string();
        *self.port.lock().unwrap() = port;

        let addr = format!("{bind}:{port}");
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                crate::logger::log(format!("无法监听 {addr}：{e}"));
                return;
            }
        };

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        *self.shutdown.lock().unwrap() = Some(tx);
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        *self.done.lock().unwrap() = Some(done_rx);
        *self.running.lock().unwrap() = true;

        let app = Router::new()
            // 显式处理根路径：axum 的 /{*path} 通配符不会匹配裸根 "/"
            .route("/", axum::routing::any(handler))
            .route("/{*path}", axum::routing::any(handler))
            // 请求体上限 8KB（密码/进程名都很短），防止大请求体耗尽内存
            .route_layer(DefaultBodyLimit::max(8192))
            .with_state(self.state.clone())
            .into_make_service_with_connect_info::<std::net::SocketAddr>();

        crate::logger::log(format!("服务已启动，监听 {addr}"));

        tauri::async_runtime::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async {
                let _ = rx.await;
            });
            if let Err(e) = server.await {
                crate::logger::log(format!("服务运行出错：{e}"));
            }
            // 服务已彻底停止，通知等待方（重启时用于确保端口已释放）
            let _ = done_tx.send(());
        });
    }

    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown.lock().unwrap().take() {
            let _ = tx.send(());
        }
        // 取出接收端后立刻释放锁（guard 不能跨 await，否则 future 不是 Send）
        let rx = { self.done.lock().unwrap().take() };
        // 等待旧服务真正停稳（释放端口）再返回
        if let Some(rx) = rx {
            let _ = rx.await;
        }
        *self.running.lock().unwrap() = false;
        crate::logger::log("服务已停止。");
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

/// 从 Cookie 头里取出 ltm_session 的值。
fn cookie_session(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("ltm_session=") {
            return Some(v.to_string());
        }
    }
    None
}

async fn handler(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: String,
) -> Response {
    let ip = addr.ip().to_string();
    let rawurl = uri.path().to_string();

    // 鉴权解析：优先 token（cookie），其次退回 IP。返回 (是否登录, 当前 token)。
    let (authorized, token): (bool, Option<String>) = if let Some(t) = cookie_session(&headers) {
        match state.ip_of_token(&t) {
            Some(bound_ip) => {
                // token 有效；若绑定 IP 与来源不一致仅告警（局域网内 IP 可能变动）
                if bound_ip != ip {
                    crate::logger::log(format!(
                        "会话 IP 与来源不符（会话={bound_ip} 来源={ip}），仍放行"
                    ));
                }
                (true, Some(t))
            }
            None => (false, None),
        }
    } else {
        (state.is_authorized_ip(&ip), None)
    };

    // 1) favicon
    if rawurl.ends_with(".ico") {
        return with_security_headers(()).into_response();
    }

    // 2) 样式
    if rawurl.ends_with(".css") {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/css; charset=utf-8"),
        );
        return with_security_headers((headers, web::APP_CSS.to_string())).into_response();
    }

    // 3) 脚本（注入语言）
    if rawurl.ends_with(".js") {
        let lang = state.language.lock().unwrap().clone();
        let body = replace_lang(web::APP_JS, &lang);
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/x-javascript; charset=utf-8"),
        );
        return with_security_headers((headers, body)).into_response();
    }

    // 4) 退出登录
    if rawurl.ends_with("/logout") {
        if let Some(t) = &token {
            state.logout_session(t);
        }
        // 同时清掉 IP 登录态（退回行为）
        state.set_authorized_ip(&ip, false);
        return with_security_headers((text_plain(), "ok".to_string())).into_response();
    }

    // 5) 登录（密码放在请求体里，参考实现一致）
    if rawurl.ends_with("/dologin") {
        let pw_hash = state.password_hash.lock().unwrap().clone();
        let supplied = body.trim().to_string();

        if state.is_blocked(&ip) {
            return (
                StatusCode::FORBIDDEN,
                text_plain(),
                block_text(&state.language.lock().unwrap().clone()),
            )
                .into_response();
        }

        // 未设置密码：空字符串哈希即空串
        let ok = if pw_hash.is_empty() {
            supplied.is_empty()
        } else {
            !supplied.is_empty() && settings::verify_password(&pw_hash, &supplied)
        };

        if ok {
            let token = state.new_session(&ip);
            state.set_authorized_ip(&ip, true);
            let warn = pw_hash.is_empty(); // 空密码 -> 需要提示设置密码
            crate::logger::log(format!(
                "登录成功：{ip}{}",
                if warn { "（无密码）" } else { "" }
            ));
            let mut headers = HeaderMap::new();
            headers.insert(
                header::SET_COOKIE,
                axum::http::HeaderValue::from_str(&format!(
                    "ltm_session={}; Path=/; Max-Age={}; SameSite=Strict; HttpOnly",
                    token, TOKEN_MAX_AGE
                ))
                .unwrap(),
            );
            let resp = if warn { "warning" } else { "ok" };
            return with_security_headers((headers, text_plain(), resp.to_string()))
                .into_response();
        }

        state.record_wrong(&ip);
        state.set_authorized_ip(&ip, false);
        crate::logger::log(format!("登录失败，密码错误：{ip}"));
        return (text_plain(), "bad".to_string()).into_response();
    }

    // 6) 封禁
    if state.is_blocked(&ip) {
        let lang = state.language.lock().unwrap().clone();
        return (
            StatusCode::FORBIDDEN,
            text_plain(),
            block_text(&lang),
        )
            .into_response();
    }

    // 7) 未登录
    if !authorized {
        if method == Method::GET {
            // 页面请求：返回登录页
            return with_security_headers((html_utf8(), web::LOGIN_HTML.to_string()))
                .into_response();
        }
        // API 请求（/list、/kill 等）：返回 401，让前端重定向回登录页
        return (
            StatusCode::UNAUTHORIZED,
            text_plain(),
            "unauthorized".to_string(),
        )
            .into_response();
    }

    // 已登录：刷新会话时间
    if let Some(t) = &token {
        state.touch_session(t);
    } else {
        state.set_authorized_ip(&ip, true);
    }

    // 8) 已登录
    if rawurl.ends_with("/kill") {
        let target = body.trim().to_string();
        // 只允许按 PID 精确结束，避免按名字误杀同名进程。
        if target.is_empty() || target.len() > 16 {
            return (text_plain(), "fail".to_string()).into_response();
        }
        match target.parse::<u32>() {
            Err(_) => return (text_plain(), "fail".to_string()).into_response(),
            Ok(pid) => {
                crate::logger::log(format!("结束进程：{ip} pid={pid}"));
                match process::kill_by_pid(pid) {
                    process::KillResult::Ok => {
                        return with_security_headers((text_plain(), "ok".to_string()))
                            .into_response();
                    }
                    process::KillResult::Partial => {
                        return with_security_headers((text_plain(), "fail".to_string()))
                            .into_response();
                    }
                    process::KillResult::NotFound => {
                        return (StatusCode::NOT_FOUND, text_plain(), "gone".to_string())
                            .into_response();
                    }
                    process::KillResult::Protected => {
                        return (
                            StatusCode::FORBIDDEN,
                            text_plain(),
                            "protected".to_string(),
                        )
                            .into_response();
                    }
                }
            }
        }
    }

    if rawurl.ends_with("/list") {
        let list = process::list();
        let body = serde_json::to_string(&list)
            .unwrap_or_else(|_| "{\"mem\":{\"pct\":0,\"used\":0,\"total\":0},\"list\":[]}".into());
        return with_security_headers((text_plain(), body)).into_response();
    }

    // 根路径 -> 管理页
    with_security_headers((html_utf8(), web::MANAGER_HTML.to_string())).into_response()
}

/// 给所有 HTML/JSON/文本响应统一加安全头（nosniff / X-Frame-Options / CSP）。
fn with_security_headers<R: IntoResponse>(resp: R) -> Response {
    let mut resp = resp.into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        axum::http::HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'",
        ),
    );
    resp
}

fn replace_lang(js: &str, lang: &str) -> String {
    let code = i18n::get(lang).name;
    js.replace("WEBLANGUAGE", code)
}

fn block_text(lang: &str) -> String {
    match i18n::get(lang).name {
        "CN" => "你已经被屏蔽。如果要取消屏蔽，请在你的电脑上重启服务。".to_string(),
        "ZHTW" => "你已經被封鎖。如果要解除封鎖，請在你的電腦上重新啟動服務。".to_string(),
        _ => "You're now blocked. If you want to unblock, please restart service on your PC."
            .to_string(),
    }
}

fn text_plain() -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    h.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    h
}

fn html_utf8() -> axum::http::HeaderMap {
    let mut h = axum::http::HeaderMap::new();
    h.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    h
}
