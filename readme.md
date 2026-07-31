# 手机任务管理器 · LanTaskmgr_rs

在**手机浏览器**里查看并一键结束**电脑上的进程**。原程序是 .NET（VB.NET / WinForms）写的
[RunTaskManagerOnYourPhone](https://github.com/...)，这里用 **Rust + Tauri v2** 完全重写，
更轻量、启动更快、内存占用更低。

> 手机和电脑必须在**同一个局域网**内。

---

## ✨ 功能特性

- **局域网手机管控**：手机浏览器访问 `http://电脑IP:5555`，即可看到进程列表。
- **密码保护**：默认**无密码**；可在桌面设置页设定密码（同一 IP 连续错误 3 次封禁 10 分钟）。
- **进程列表**：进程名、主窗口标题、物理内存、CPU 占用、分类（系统 / 应用 / 其它），同名进程已聚合累加。
- **一键结束**：在手机端点一下即可结束进程；系统关键进程仅警告、不禁止。
- **托盘常驻**：关闭主窗口不会退出程序，手机端服务继续运行；从托盘可随时重新打开。
- **轻量模式**：开启后关闭窗口会**立刻销毁所有 `msedgewebview2` 进程**并把内存还给系统；程序与手机服务继续存活，托盘重开窗口即重建。
- **多语言**：简体中文 / English / 繁體中文。
- **开机自启**：设置页可勾选。
- **便携版**：单个 `exe`，前端已编译进二进制，解压即跑，无需安装。

---

## 🚀 快速开始（便携版）

1. 到 **Releases** 页下载 `LanTaskmgr_rs_Portable_vX.Y.Z.zip`；
2. 解压，双击 `LanTaskmgr_rs.exe`（首次会请求防火墙放行，允许即可）；
3. 桌面弹出设置窗口，记下左下角的 **手机访问地址**（如 `http://192.168.1.10:5555/`）；
4. 手机浏览器打开该地址 → 进入管理页 → 结束进程。

> ⚠️ **系统要求**：Windows 10 / 11，且已安装 **WebView2 运行时**（Win10/11 通常自带；
> 若提示缺少 WebView2，请从
> [Microsoft 官方页面](https://developer.microsoft.com/zh-cn/microsoft-edge/webview2/) 下载「Evergreen 引导程序」安装）。

---

## 🛠 本地构建

### 前提

- Windows 10 / 11
- [Rust](https://www.rust-lang.org/) stable 工具链（目标 `x86_64-pc-windows-msvc`）
- Visual Studio 2022+ 的 **MSVC 生成工具**（C++ 桌面开发 workload）
- WebView2 运行时

### 编译

```powershell
cd src-tauri
cargo build --release
# 产物：src-tauri/target/release/lantaskmgr_rs.exe
```

前端（`ui/`）会在编译时通过 `build.rs` → `tauri-build` 嵌入二进制，
因此**单文件 exe 即可独立运行**，不需要附带任何资源目录。

---

## 📦 便携版与安装包（GitHub Actions 自动发布）

仓库内置两条 GitHub Actions 工作流：

| 工作流 | 文件 | 触发方式 | 产物 |
| --- | --- | --- | --- |
| **便携版** | `.github/workflows/release-portable.yml` | 推送 `v*` tag **或** 手动 `workflow_dispatch` | `LanTaskmgr_rs_Portable_vX.Y.Z.zip`（单 exe + 说明） |
| 安装包 | `.github/workflows/release.yml` | 手动 `workflow_dispatch` | Windows NSIS 安装包 |

发布便携版只需打一个 tag：

```powershell
git tag v1.0.0
git push origin v1.0.0
```

GitHub Actions 会自动编译并把便携 zip 上传到同名 Release。
（`release.yml` 的 NSIS 安装包改为手动触发，避免与便携版在同一 tag 上重复创建 Release。）

---

## 📁 目录结构（关键文件）

```
LanTaskmgr_rs/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs          # 入口：AppState、托盘、窗口关闭处理、运行循环
│  │  ├─ lightweight.rs   # 轻量模式：关窗后立刻回收所有 msedgewebview2 进程
│  │  ├─ process.rs       # 进程枚举 / 结束 / WebView2 进程识别与回收
│  │  ├─ server.rs        # axum HTTP 服务、登录鉴权、启停
│  │  ├─ settings.rs      # 设置持久化（%APPDATA%/LanTaskmgr/settings.json）
│  │  ├─ i18n.rs          # 内嵌多语言文案
│  │  ├─ web.rs + web/    # 手机端 UI（登录页 / 管理页 / css / js）
│  │  └─ ...
│  ├─ tauri.conf.json     # Tauri 配置（identifier=com.lantaskmgr.rs）
│  └─ Cargo.toml
├─ ui/index.html          # 桌面设置页（前端已嵌入二进制）
├─ .github/workflows/
│  ├─ release-portable.yml  # 便携版自动发布
│  └─ release.yml           # NSIS 安装包（手动）
├─ readme.md
└─ agent.md               # 面向 AI 编程助手的约定
```

---

## 📝 设置项（settings.json）

位于 `%APPDATA%/LanTaskmgr/settings.json`：

| 字段 | 默认值 | 说明 |
| --- | --- | --- |
| `password` | `""` | 空 = 无密码 |
| `port` | `5555` | HTTP 服务端口 |
| `language` | `"CN"` | `CN` / `EN` / `ZHTW` |
| `autostart` | `false` | 开机自启 |
| `bind` | `"0.0.0.0"` | 绑定地址（0.0.0.0 才能被手机访问） |
| `lightweight` | `false` | 轻量模式：关窗即销毁 WebView2 |

---

## 📄 License

MIT（如另需，可在此补充）。
