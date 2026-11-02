# 兼容性

启动器不修改 DSH-NEXT 本体，但**依赖它的若干外部事实**。这些事实一旦变化，多开就会失效。
本文件记录已验证版本、依赖契约，以及升级后如何重新适配。

## 平台支持

| 平台 | 状态 | 说明 |
|---|---|---|
| Windows 10/11 (x64) | ✅ 支持 | 唯一经过构建与验收的平台 |
| macOS | ❌ 未支持 | 无构建与验收环境；欢迎社区适配 |
| Linux | ❌ 未支持 | 同上 |

代码里的平台假设集中在四个文件，适配时优先看它们：

| 文件 | 假设 | 适配方向 |
|---|---|---|
| `util.rs` | 路径拼接、`%APPDATA%` / `%LOCALAPPDATA%` / `%USERPROFILE%` | `PathBuf::join` + `dirs` crate |
| `scan.rs` | 命令行含 `--user-data-dir=<home>\electron-user-data`、`\profiles\desktop` | 按目标平台的 DSH-NEXT 命令行重新取证 |
| `perm.rs` | `IsUserAnAdmin`、`ShellExecuteW "runas"` | mac `geteuid` / linux `pkexec` |
| `versions.rs` | `mklink /J`（junction） | Unix `symlink` |

`tauri.conf.json` 的 `bundle.targets` 目前只有 `nsis`；跨平台需要补 `dmg` / `appimage` / `deb` 与对应图标。

## 已验证版本

| 壳版本 | 内核（`@deepseek-ai/dsh`） | 结果 | 验证方式 |
|---|---|---|---|
| `2.0.21-next` | `0.2.1-rc.4` | ✅ 完整通过 | [../testing.md](../testing.md) |
| `2.0.20-next` | `0.2.0-rc.9` | ✅ 版本扫描 | 单测 `versions::tests::scans_and_marks_invalid` |

其它版本**未经验证**。升级 DSH-NEXT 后请按下方"如何适配新版本"重新跑一遍。

## 为什么用独立 home，而不是官方 profile

DSH-NEXT 自带的实例 / profile 切换**只隔离插件系统**（`plugins/`、`profiles/`）；
会话（`sessions/`）、插件存储（`storages/`）、凭据（`.credentials.yaml`）仍在同一个 home 内，
切换 profile 后这些数据互相可见。

本工具用**独立 home** 实现整套隔离：`sessions/`（按 workspace 分组）、`storages/`、
`.credentials.yaml` 都在各自 home 内，文件级互不可见；`electron-user-data` 也独立，
因此 Electron 单实例锁互不冲突。

边界：Agents Anywhere 账号（设备绑定）与内核版本仍是全局的，不随 home 隔离（见[能力边界](../../README.md#能力边界)）。

## 依赖契约（DSH-NEXT 侧）

| 契约 | 用在哪 | 失效表现 |
|---|---|---|
| 指针文件 `%APPDATA%\DSH NEXT\home\desktop-next-location.json` 存在，内容为 `{"home": "..."}` | `pointer.rs` | 写不进去 → `E1001`；回滚失败 → `E1002` |
| `home` → `userData = <home>\electron-user-data` | `scan.rs`、`launcher.rs` | 进程归属反推失败，列表全部显示"已停止" |
| 进程命令行含 `--user-data-dir=<home>\electron-user-data`（gpu/renderer）或 `<home>\profiles\desktop`（host） | `scan.rs` | 同上 |
| 安全模式：命令行含 `--next-safe-mode`，或 userData 落在 `electron-user-data\safe-mode` | `scan.rs` | 安全模式徽标与提示条失效 |

> 这四条都是从 `app.asar` 的 `lib/main.js` 与实跑命令行取证得到的，不是猜测。
> 取证过程见 [../experience/02-进程扫描.md](../experience/02-进程扫描.md)。

## 参考指纹（`2.0.21-next`）

| 项 | 值 |
|---|---|
| `DSH NEXT.exe` 大小 | 246,521,344 B |
| `resources\app.asar` 大小 | 190,247,659 B |
| 设计 token 数 | 1158 |
| `design/tokens.lock.json` | 可复跑：同参数两次运行逐字节相同 |

## 如何适配新版本

```powershell
# 1) 重新抽取 token（产物写入 design/）
node tools/extract-tokens.mjs --out design/tokens.lock.json --css src/styles/tokens.css
# 2) 看差异：新增/删除的变量在 design/controls.md（带 pkg/file:符号 来源）
# 3) 补登记后跑门禁
node tools/verify-tokens.mjs
# 4) 回归
cargo test --manifest-path src-tauri/Cargo.toml
node tools/e2e.mjs
```

再按 [../testing.md](../testing.md) §3 做一次真实窗口多开的手工验收。

## 已知不兼容

| 情况 | 处理 |
|---|---|
| 同一个 home 起两个实例 | 拒绝（`E1003`）——两个 writer 会损坏会话，这是安全策略而非缺陷 |
| 安装目录缺 `resources\app.asar` | 拒绝登记（`E1004`），无法读取壳/内核版本 |
| 纯 webview 套壳、非 Electron 形态 | 不支持：没有 `--user-data-dir` 子进程，进程归属无法反推 |
