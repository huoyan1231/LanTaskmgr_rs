//! HTTP 服务：手机端访问的入口。逻辑与原程序 Listener 一一对应。
//!
//! 路由（原程序是 Mono.Net.HttpListener，这里用 axum 0.8）：
//!   GET  /app.css        -> 样式
//!   GET  /app.js         -> 注入语言后的脚本
//!   POST /dologin        -> 校验密码（密码放在请求体里），错误 3 次封禁该 IP
//!   POST /list           -> 已登录时返回进程列表 JSON
//!   POST /kill           -> 已登录时结束指定进程（进程名放请求体）
//!   POST /logout         -> 退出当前 IP 的登录态
//!   GET  /              -> 已登录时返回 manager.html，否则返回 login.html
//!   GET  /favicon.ico    -> 空响应
//!
//! 安全模型（完全复刻）：按「请求来源 IP」记录登录状态；
//! 10 分钟无访问后需要重新登录；连续输错 3 次密码永久封禁该 IP，直到手动重启服务。

use crate::i18n;
use crate::process;
use crate::web;
use axum::extract::{ConnectInfo, OriginalUri, State};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const AUTH_TIMEOUT_SECS: u64 = 10 * 60;
const MAX_WRONG: u32 = 3;

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
    pub password: Mutex<String>,
    pub language: Mutex<String>,
}

impl ServerState {
    pub fn new() -> Self {
        ServerState {
            devices: Mutex::new(HashMap::new()),
            password: Mutex::new(String::new()),
            language: Mutex::new("EN".to_string()),
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

    fn is_authorized(&self, ip: &str) -> bool {
        let mut map = self.device(ip);
        let d = map.get_mut(ip).unwrap();
        if d.authorized && d.last_visit.elapsed().as_secs() > AUTH_TIMEOUT_SECS {
            d.authorized = false;
        }
        d.authorized
    }

    fn set_authorized(&self, ip: &str, v: bool) {
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

        *self.state.password.lock().unwrap() = password.to_string();
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

async fn handler(
    State(state): State<Arc<ServerState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    body: String,
) -> Response {
    let ip = addr.ip().to_string();
    let rawurl = uri.path().to_string();

    // 1) favicon
    if rawurl.ends_with(".ico") {
        return ().into_response();
    }

    // 2) 样式
    if rawurl.ends_with(".css") {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/css; charset=utf-8"),
        );
        return (headers, web::APP_CSS.to_string()).into_response();
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
        return (headers, body).into_response();
    }

    // 4) 退出登录
    if rawurl.ends_with("/logout") {
        state.set_authorized(&ip, false);
        return (text_plain(), "ok".to_string()).into_response();
    }

    // 5) 登录（密码放在请求体里，参考实现一致）
    if rawurl.ends_with("/dologin") {
        let pw = state.password.lock().unwrap().clone();
        let supplied = body.trim().to_string();
        if pw.is_empty() {
            // 未设置密码：空登录即放行
            state.set_authorized(&ip, true);
            crate::logger::log(format!("登录成功（无密码）：{ip}"));
            return (text_plain(), "ok".to_string()).into_response();
        }
        if !supplied.is_empty() && supplied == pw {
            state.set_authorized(&ip, true);
            crate::logger::log(format!("登录成功：{ip}"));
            return (text_plain(), "ok".to_string()).into_response();
        }
        state.record_wrong(&ip);
        state.set_authorized(&ip, false);
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
    if !state.is_authorized(&ip) {
        if method == Method::GET {
            // 页面请求：返回登录页
            return (html_utf8(), web::LOGIN_HTML.to_string()).into_response();
        }
        // API 请求（/list、/kill 等）：返回 401，让前端重定向回登录页
        return (
            StatusCode::UNAUTHORIZED,
            text_plain(),
            "unauthorized".to_string(),
        )
            .into_response();
    }

    // 8) 已登录
    if rawurl.ends_with("/kill") {
        let target = body.trim().to_string();
        if !target.is_empty() {
            crate::logger::log(format!("结束进程：{ip} {target}"));
            let ok = process::kill_by_name(&target);
            let resp = if ok { "ok" } else { "fail" };
            return (text_plain(), resp.to_string()).into_response();
        }
        return (text_plain(), "fail".to_string()).into_response();
    }

    if rawurl.ends_with("/list") {
        let list = process::list();
        let body = serde_json::to_string(&list)
            .unwrap_or_else(|_| "{\"mem\":{\"pct\":0,\"used\":0,\"total\":0},\"list\":[]}".into());
        return (text_plain(), body).into_response();
    }

    // 根路径 -> 管理页
    (html_utf8(), web::MANAGER_HTML.to_string()).into_response()
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
