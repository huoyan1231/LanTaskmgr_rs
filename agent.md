# agent.md — LanTaskmgr_rs

项目说明与对 AI 编程助手（agent）的约定，供后续自动修改 / 构建 / 发布时参考。

## 这是什么
把 .NET（VB.NET / WinForms）程序 **RunTaskManagerOnYourPhone** 用 **Rust + Tauri v2** 重写。
功能：手机浏览器在同一局域网内用密码登录 PC，查看进程列表并一键结束进程。

- 仓库：`D:/git/LanTaskmgr_rs`
- 程序名：`LanTaskmgr_rs`（exe：`lantaskmgr_rs.exe`，identifier：`com.lantaskmgr.rs`）
- 作者：`huoyan1231`
- 默认无密码；默认端口 `5555`；绑定 `0.0.0.0`（手机才能访问）；可改填具体局域网 IP 只在该网卡监听（防公网暴露）。

## 技术栈
- **Rust + Tauri v2**（窗口 label = `"main"`，托盘图标 + 菜单）。
- 内嵌 HTTP 服务：`axum` 0.8，通过 `tauri::async_runtime::spawn` 启动。
- 进程枚举：`sysinfo` 0.33（`refresh_processes` / `refresh_cpu_all`）。
- Windows API：`windows` crate `=0.61.3`（`EnumWindows` / 窗口标题 / 可见性）。
- 全局状态：`OnceLock<Arc<AppState>>`（`APP_STATE` 静态单例）。
- 前端：桌面设置页 `ui/index.html`（Tauri frontend）；手机端页面在 `src-tauri/src/web/`（`include_str!` 嵌入 `login.html` / `manager.html` / `app.css` / `app.js`）。

## 目录结构（关键文件）
- `src-tauri/src/main.rs` — `AppState`、`snapshot` / `save_settings` / `set_language` / `restart_service` / `open_external` 命令、托盘、`attach_close_handler` / `show_main_window` / `recreate_main_window`、运行循环（`ExitRequested` 里 `prevent_exit()` 保留程序）。
- `src-tauri/src/lightweight.rs` — 轻量模式：窗口**关闭后立刻**回收本进程树下全部 `msedgewebview2` 进程（多轮兜底扫描 + 复用取消）。
- `src-tauri/src/server.rs` — axum 路由、登录鉴权（按 IP 记录、错误 3 次封禁、10 分钟超时）、`start()` / `stop()`（用 oneshot `done` channel 等旧 server 真正停稳再 rebind，避免重启只切开关）。
- `src-tauri/src/process.rs` — `list()` 返回 `ListResponse { mem, list:[{n,t,m,p,k,c,i}] }`；`kill_by_name` 返回 `found && !failed`。
- `src-tauri/src/settings.rs` — `Settings` 持久化到 `%APPDATA%/LanTaskmgr/settings.json`（含 `lightweight`、`autostart`、`port`、`password`、`language`、`bind`）。
- `src-tauri/src/i18n.rs` — 编译进二进制的 EN / CN / ZHTW 文案（手机端 `WEBLANGUAGE` 注入，注意用 `ZHTW` 不是 `TW`）。
- `src-tauri/src/web.rs` + `src-tauri/src/web/*` — 手机端 UI（移植自参考项目 `D:/git/LanTaskmgr/web`，视觉一致）。
- `ui/index.html` — 桌面设置页（设置 / 连接 / 帮助三页；含「轻量模式」开关）。
- `.github/workflows/release-portable.yml` — 便携版自动发布（v* tag 触发，`cargo build` + 压缩 zip）。
- `.github/workflows/release.yml` — NSIS 安装包（改为仅手动 `workflow_dispatch`）。
- `verify_webview_kill.ps1` — 在本机 PowerShell 运行，自动验证「轻量模式关窗后 WebView2 进程立刻归零、程序仍存活」。
- `readme.md` — 面向用户的中文项目说明。

## 关键行为
- **普通模式（非轻量）**：关窗走 `api.prevent_close()` + `win.hide()`，窗口只隐藏不销毁，
  WebView 保留在内存里，下次托盘打开秒开；程序也不退出。
- **轻量模式（lightweight）**：窗口关闭后**立刻销毁** WebView 并回收全部 `msedgewebview2` 进程，
  内存立刻还给系统（用户硬性要求，不可用延迟方案）。见 `src-tauri/src/lightweight.rs`：
  - 状态机 `LightweightState { Normal=0, In=1 }`，存在 `AtomicU8` 里。
  - `attach_close_handler` 在轻量模式下**不调用** `prevent_close()`，让 `WebviewWindow` 真正销毁，
    先走一遍 WebView2 的正常释放；普通模式仍 `prevent_close` + `hide`。
  - 窗口真正销毁后（`WindowEvent::Destroyed`，而非 `CloseRequested`）触发 `on_window_destroyed`：
    1. 先等窗口从 WindowManager 摘掉（`WAIT_GONE_ROUNDS=20` × `WAIT_GONE_INTERVAL_MS=50`）；
       若等待期间窗口又被打开，则放弃清理，避免误杀新 WebView；
    2. `GRACE_MS=150` 后开始扫描，调用 `process::kill_own_webview_processes()`；
    3. 多轮兜底复扫（`SWEEP_ROUNDS=4` × `SWEEP_INTERVAL_MS=350`），WebView2 子进程分批退出，一轮扫不干净；
    4. 任一轮发现 `exiting==true` 或窗口被重新打开，立即停手；
    5. 结束记日志「结束 N 个 WebView2 进程，残留 M 个」。
  - 托盘打开走 `show_main_window()`：窗口还在（普通模式隐藏态）就 `show + unminimize + set_focus`；
    轻量模式下窗口已销毁则 `recreate_main_window()` 重建，重建后 `notify_window_opened()` 回到 `Normal`。
- **WebView2 进程识别（`src-tauri/src/process.rs`）**：`is_our_webview()` 两条判据取并集 ——
  1. 命令行带我们独占的 `--user-data-dir=...\com.lantaskmgr.rs\EBWebView` 标记（= `tauri.conf.json` 的 `identifier`），
     能抓到上次实例崩溃/被强杀后残留、下次会被复用的**孤儿进程**；
  2. 当前进程子孙树兜底。
  **绝不误伤** Edge 浏览器、向日葵 GameViewer、VSCode、QQ 等其它应用的 WebView2。
- **启动清理**：`main()` 一开始（早于 `settings::load` 与建窗）就 `process::kill_own_webview_processes()`，
  清掉上次残留的孤儿进程，否则会被本次 WebView2 复用而越攒越多。
- **程序不随窗口关闭而退出**：靠 `app.run` 里的 `RunEvent::ExitRequested` + `api.prevent_exit()`，
  不是靠 `prevent_close()`（后者只挡窗口、挡不住进程退出）。轻量模式下窗口销毁触发的是 ExitRequested，
  同样被 `prevent_exit()` 拦下，托盘与手机端 HTTP 服务继续存活。
- **退出**：仅托盘「退出」会置 `AppState.exiting = true`、调用 `lightweight::shutdown()` 后 `app.exit(0)`，放行 `ExitRequested`。

## 构建（Windows MSVC）
- `cargo` 不在 PATH：`C:\Users\huoyan1231\.cargo\bin\cargo.exe`
- 需要 VS2026 的 MSVC 环境（PowerShell：`Enter-VsDevShell` 或手动设 `PATH/LIB/INCLUDE`）。VS 路径：`D:\Microsoft Visual Studio\18\Community`
- **重建前必须先** `Stop-Process -Name lantaskmgr_rs`，否则运行中的 exe 被锁，`link.exe` 报 LNK1104。
- 用 PowerShell 跑构建 / 停止进程（`cmd /c` 会被安全策略拦截）。
- `cargo build --release` 即生成 release；`tauri.conf.json` 的 `bundle.targets = ["nsis"]`。

## 发布（GitHub Actions）
两条工作流（`tauri.conf.json` 的 `build.beforeBuildCommand` 设为 `""`，前端是静态 HTML 无需 npm 构建；
前端 `ui/` 在编译时经 `build.rs → tauri-build` 嵌入二进制，故 `cargo build --release` 产出的 exe 可独立运行）：

- **便携版（自动）**：`.github/workflows/release-portable.yml`
  - 触发：推送 `v*` tag **或** 手动 `workflow_dispatch`。
  - 流程：`dtolnay/rust-toolchain`（x86_64-pc-windows-msvc）→ `swatinem/rust-cache` →
    `cargo build --release`（在 `src-tauri`）→ PowerShell 把 `lantaskmgr_rs.exe` 改名为
    `LanTaskmgr_rs.exe` 并附 `便携版说明.txt` → `Compress-Archive` 打成
    `LanTaskmgr_rs_Portable_vX.Y.Z.zip` → `softprops/action-gh-release@v2` 上传到同名 Release。
  - 这是 `v*` tag 的**默认自动发布**，用户下载解压即跑。
- **NSIS 安装包（手动）**：`.github/workflows/release.yml`
  - 触发：**仅** `workflow_dispatch`（已改为手动，避免与便携版在同一 tag 重复创建 Release）。
  - `tauri-apps/tauri-action@v0` 编译并打包 NSIS 安装包，上传到同名 Release。
  - 如需 NSIS 自动随 `v*` 发布，把两条工作流合并为一个（单一 Release 同时挂便携 zip + NSIS）即可。

## 约定
- 改动后端逻辑后务必重新构建验证；桌面窗口渲染 / 托盘交互无法在沙箱（headless）目视测试，需在 GUI 环境确认（见仓库 `verify_webview_kill.ps1` 可自动验证轻量模式关窗行为）。
- 数据契约（手机端 `ListResponse` 字段名 `n/t/m/p/k/c/i`、`mem{pct,used,total}`）要和 `src-tauri/src/web/app.js` 对齐，勿随意改名。
- 新增二进制依赖优先用 `cargo` 在隔离环境安装，勿污染用户全局环境。
- 项目说明见 `readme.md`；本文件是对 AI 编程助手的实现约定。

## ⚠️ 沙箱构建坑（仅本 agent 运行环境，用户真机不受影响）
- 本沙箱**只允许新建文件、拒绝改写/删除工作区里已存在的文件**（`target/` 等）。表现为 `cargo build/clean` 反复
  `os error 5`（access denied）于既有的 `.cargo-build-lock` / `fingerprint/.../bin-lantaskmgr_rs` /
  `deps/*.d` / `.rustc_info.json`；`cargo clean`、`shutil.rmtree`、PowerShell `OpenWrite` 一律失败。
- **绕过**：设 `CARGO_TARGET_DIR` 指向全新目录（只新建不改写）即可正常编译，如
  `CARGO_TARGET_DIR=D:\git\LanTaskmgr_rs\relbuild cargo build --release`（已加入 `.gitignore`）。
- **用户真机无此限制**：在自己的 Windows 上 `cargo build --release`（默认 `target/`）一切正常。
