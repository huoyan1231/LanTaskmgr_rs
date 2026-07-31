# LanTaskmgr_rs

用手机浏览器结束电脑上失控的进程。

当 Windows 把 CPU 占满、桌面停止刷新时，任务管理器恰恰是那个你打不开的东西。
LanTaskmgr_rs 在局域网内常驻一个轻量 HTTP 服务，让你在手机上拉起进程列表、
一键结束捣乱的进程，而不用碰那台已经假死的机器。

这是原程序 **Run Task Manager On Your Phone**（VB.NET / WinForms）用 **Rust + Tauri v2**
的现代化重写。

## 为什么用 Rust + Tauri

原版拖着整套 .NET Framework，外加 `HttpListener` 和 `Newtonsoft.Json` —— 对一个平时
只是闲置、只在你需要的那天才出力的小程序来说，这代价太大了。

这个工具要 **24/7 常驻**，而且关键在于：当机器已经资源耗尽时它**必须还能响应**。
每多占一分内存，都是在和它的初衷作对。所以我们换了个写法：

|                    | 原版 (VB.NET)                | LanTaskmgr_rs                     |
| ------------------ | ---------------------------- | --------------------------------- |
| 语言               | VB.NET / WinForms            | Rust + Tauri v2                   |
| 运行时依赖         | .NET Framework               | WebView2 运行时（系统一般已自带）|
| 需分发的文件       | 多（exe + DLL + web + 语言） | 1 个（`lantaskmgr_rs.exe`，前端已嵌入）|
| UI                 | WinForms                     | WebView2（HTML/CSS/JS），支持深色模式 |
| 进程枚举           | .NET `Process` API           | `sysinfo` + Win32 `EnumWindows`   |
| 手机端             | 内嵌网页                     | 内嵌网页（简体 / English / 繁體）  |

老实说：Tauri 依赖系统里已有的 **WebView2 运行时**（Win10/11 通常自带，缺了装一下即可），
做不到 C 重写版那种"零依赖纯 Win32 单文件"。换来的好处是 UI 用真正的网页技术写，
深色模式、移动端适配、多语言都轻松，而 Rust 本身的内存 / CPU 占用又远低于 .NET。
对一台需要随时被"远程救活"的机器来说，常住进程轻一点，总归是好事。

## 构建

需要 MSVC（任意 Visual Studio 版本，或独立 Build Tools 的"使用 C++ 的桌面开发"工作负载）
和 Rust stable（目标 `x86_64-pc-windows-msvc`）。**不需要**包管理器，也**不需要** npm ——
前端是静态 HTML，已在编译时嵌入二进制。

```powershell
cd src-tauri
cargo build --release
# 产物：src-tauri/target/release/lantaskmgr_rs.exe
```

产物是单文件 exe，静态依赖仅 WebView2 运行时，可直接在干净 Windows 上运行。

## 使用

运行 exe。窗口会显示本机在局域网上可达的地址和各项设置。

1. 服务默认**空密码**（不弹登录框）；想保护局域网访问就在桌面设置页设一个密码。
2. 在手机上（同一 Wi-Fi / 局域网），用浏览器打开其中某个 LAN 地址。
3. 登录，点进程，确认结束。

勾选**开机自启**，下次卡死时它已经在监听了。关闭主窗口只是把程序藏到托盘；
要从托盘菜单里选「退出」才是真正退出。

### 手机上看到什么

进程按镜像名分组（同名实例会计数），并标出私有内存、CPU 占用、主窗口标题，按类别区分
（系统 / 有窗口的应用 / 其它）。

- 每行带内存 / CPU 计量条，高占用会标红；
- 可按 **CPU / 内存 / 名称** 排序，也能用筛选框过滤；
- 列表会定期刷新；
- 点一下进程即可结束。对关键系统进程（`csrss`、`wininit`、`smss`、`services`、
  `lsass`、`winlogon` ……一旦结束会立刻蓝屏或重启 Windows 的那批）服务端**直接拒绝**，
  而不是仅仅警告。

## 配置

`settings.json`，写在 `%APPDATA%\LanTaskmgr\settings.json`
（若 exe 位于只读位置，则写到该用户目录）：

```json
{
  "password": "",          // 留空 = 不弹登录框
  "port": 5555,            // HTTP 服务端口
  "language": "CN",        // CN | EN | ZHTW
  "autostart": false,      // 开机自启
  "bind": "0.0.0.0",       // 绑定地址；0.0.0.0 手机才能访问
  "lightweight": false     // 轻量模式：关窗即销毁 WebView2，释放内存
}
```

服务启停、登录尝试、结束操作都会记入日志。

## 安全性，实话实说

这是一个明文 HTTP 的局域网工具，能结束任意进程。请理性对待：

- 流量**没有加密**。同网段任何人都能看到会话，所以**别在咖啡厅 / 酒店 Wi-Fi 上跑它**；
- 密码以明文存放在 `settings.json`，登录后签发随机会话令牌，并按恒定时间校验；
- 同一 IP 连续 **3 次**登录失败会被封禁 **10 分钟**；
- **切勿**把端口通过路由器映射到公网。

## 便携版与安装包

仓库内置 GitHub Actions，打 `v*` tag 即自动发布：

- **便携版**（`.github/workflows/release-portable.yml`）：`cargo build --release` 后把单文件
  exe 打成 `LanTaskmgr_rs_Portable_vX.Y.Z.zip`，解压即跑，无需安装；
- **安装包**（`.github/workflows/release.yml`）：NSIS 安装包，手动触发。

```powershell
git tag v1.0.0
git push origin v1.0.0
# → 自动产出便携版 Release
```

## 与原版的区别

- 新增：每个进程的内存与 CPU 占用、排序 / 筛选、深色模式、最小化到托盘、首次运行空密码
  （默认不弹登录框）、**轻量模式**（关窗即销毁 WebView2 把内存还给系统）、便携版单文件、
  多语言（简体 / English / 繁體）；
- 沿用：局域网手机管控的核心体验；
- 外部语言目录已编译进二进制；无内建更新检查（这个重写版没有更新服务器）。

## 许可证

MIT。
