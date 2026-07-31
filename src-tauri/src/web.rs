//! 手机端网页资源（登录页 / 管理页 / 样式 / 脚本），编译进二进制。
//!
//! 视觉与交互对齐参考实现 `D:/git/LanTaskmgr/web`，脚本里的 `WEBLANGUAGE`
//! 占位符在 server.rs 里按当前语言替换成 EN / CN / ZHTW。

pub const LOGIN_HTML: &str = include_str!("web/login.html");
pub const MANAGER_HTML: &str = include_str!("web/manager.html");
pub const APP_CSS: &str = include_str!("web/app.css");
pub const APP_JS: &str = include_str!("web/app.js");
