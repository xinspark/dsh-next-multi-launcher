# 阶段 0 — 环境检查（env-check）

- 执行时间：2026-11-01 07:2x（Asia/Shanghai）
- 执行目录：`E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\`
- 结论：**五项全绿，走 Tauri 分支**（不改道 WPF）。

## 0.1 根目录可写

命令：

```powershell
Set-Content -Path ".probe" -Value "probe" -Encoding utf8; Test-Path ".probe"; Remove-Item ".probe" -Force; Test-Path ".probe"
```

实际输出：

```
True
False
```

结论：目录可写、可删 → 通过。

## 0.2 五项工具链检查

| 依赖 | 命令 | 实际输出 | 结论 |
|---|---|---|---|
| Node | `"C:\Users\FXS\.dsh\dsh-runtimes\dsh-next-primary-runtime\dependencies\node\bin\node.exe" -v` | `v24.21.0` | ✅ ≥ 20 |
| pnpm | `pnpm -v` | `11.7.0` | ✅ ≥ 9 |
| Rust | `cargo -V && rustc -V` | `cargo 1.98.0 (797e8a9bc 2026-08-05)` / `rustc 1.98.0 (88d9e12ae 2026-08-18)` | ✅ stable |
| WebView2 | `(Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}").pv` | `140.0.3485.66` | ✅ 已安装 |
| DSH-NEXT | `Test-Path "E:\FXS\APP\DSH NEXT\DSH NEXT.exe"` | `True` | ✅ 存在 |

补充（系统 Node 与官方 registry 连通性）：

```
node -v            -> v25.2.1            （系统 Node，仅备用；构建统一用 DSH 自带 v24.21.0）
npm config get registry -> https://registry.npmmirror.com
registry.npmjs.org /ping -> 200
index.crates.io  /config.json -> 200
```

## 0.3 编译链可用性

命令（临时目录，结束即删）：

```powershell
cargo new --quiet "$env:TEMP\cargo-probe-dshmma"
cd "$env:TEMP\cargo-probe-dshmma"; cargo build --quiet
Test-Path "target\debug\cargo-probe-dshmma.exe"
```

实际输出：`True`（生成可执行文件成功，随后删除临时目录）。

结论：Rust 编译链可用 → **不进入 WPF 改道分支**。

## 0.4 DSH-NEXT 本体指纹（供阶段 1 取证与版本登记）

| 项 | 值 | 来源 |
|---|---|---|
| 安装目录 | `E:\FXS\APP\DSH NEXT` | `Test-Path` |
| `DSH NEXT.exe` 大小 | 246,521,344 B（2026/10/30 06:01） | `Get-Item` |
| `resources\app.asar` 大小 | 190,247,659 B | `Get-Item` |
| 壳版本（app.asar!/package.json`.version`） | `2.0.21-next` | asar 索引解析 |
| 内核版本（`dependencies["@deepseek-ai/dsh"]`） | `0.2.1-rc.4` | 同上 |
| `resources\app-update.yml` | 存在（115 B），启动器只读、不改 | `Get-Item` |

## 0.5 指针与实例现状（实测快照，供阶段 2/3 参照）

```
%APPDATA%\DSH NEXT\home\desktop-next-location.json
  { "home": "C:\\Users\\FXS\\.dsh-b" }        (38 B, 2026/11/1 06:42:00)
%APPDATA%\DSH NEXT\home\desktop-next-last-home.json
  { "home": "C:\\Users\\FXS\\.dsh-b" }        (39 B, 2026/11/1 06:42:12)
```

运行中的 DSH-NEXT（启动早于指针改写，因此仍用 `.dsh`）：

| pid | 角色 | 命令特征 |
|---|---|---|
| 16128 | 主进程（窗口） | `"DSH NEXT.exe"`（无参数） |
| 40752 | host | `--expose-internals ...\app.asar\lib\host.js ... C:\Users\FXS\.dsh\profiles\desktop ...` |
| 13900 / 21428 / 35544 / 42844 | gpu / network / crashpad / renderer | `--user-data-dir="C:\Users\FXS\.dsh\electron-user-data"` |

结论印证设计 §2.3 与 §2.4：**主进程命令行里没有 `--user-data-dir`**，home 只能从 gpu/renderer/host 子进程反推，主进程 pid 要靠 host 的父进程得到。

## 0.6 阶段 0 判定

| 检查 | 结果 |
|---|---|
| 0.1 目录可写 | ✅ |
| 0.2 五项工具链 | ✅ 全绿 |
| 0.3 编译链 | ✅ `cargo build` 成功 |
| 0.4 本体指纹 | ✅ 已记录（2.0.21-next / 0.2.1-rc.4） |
| 0.5 指针快照 | ✅ 已记录（测试前必须原样还原） |

→ **进入阶段 1。**
