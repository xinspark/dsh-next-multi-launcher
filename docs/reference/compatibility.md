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
| `2.0.22-next` | `0.2.2-rc.1` | ✅ 通过 | 契约逐条核对 + token 重抽取（1191 个）+ 单测 78 项 |
| `2.0.21-next` | `0.2.1-rc.4` | ✅ 完整通过 | [../testing.md](../testing.md) |
| `2.0.20-next` | `0.2.0-rc.9` | ✅ 版本扫描 | 单测 `versions::tests::scans_and_marks_invalid` |

其它版本**未经验证**。升级 DSH-NEXT 后请按下方"如何适配新版本"重新跑一遍。

### 2.0.22 这一版动了什么（与启动器相关的部分）

| 变化 | 对启动器的影响 |
|---|---|
| 内核升到 `0.2.2-rc.1`（Cordis 4.1.0），**第三方插件可能启动失败** | 与启动器无关；装了旧插件的数据目录要先进安全模式或恢复助手 |
| 官方文档明确 `DSH_DESKTOP_NEXT_HOME` 可隔离数据目录（应用内选择优先） | 指针文件仍优先于环境变量，所以本工具继续用指针 + last-home |
| 禁止把**安装目录**选作数据目录/工作区（`program-directory` 校验） | 实例 home 不能放在 `E:\...\DSH NEXT` 里，否则 DSH-NEXT 侧拒绝 |
| 数据目录切换多了一个「数据目录与上次不同」模态 | 见下方契约第 5 条：last-home 必须与指针同步写 |
| 设计 token 1191 个（+34 / −1 / 33 个改值） | 重新抽取 `design/tokens.lock.json` 与 `src/styles/tokens.css` |

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
| 同目录的 `desktop-next-last-home.json`（DSH-NEXT 记录的"上次启动目录"）必须与指针写同一个 home | `pointer.rs`（`LastHome`）、`launcher.rs` | 不写 → 每次切换实例都弹「数据目录与上次不同」模态，**默认按钮「改用上次的目录」会把这次启动改回上一个实例** |
| `home` → `userData = <home>\electron-user-data` | `scan.rs`、`launcher.rs` | 进程归属反推失败，列表全部显示"已停止" |
| 进程命令行含 `--user-data-dir=<home>\electron-user-data`（gpu/renderer）或 `<home>\profiles\desktop`（host） | `scan.rs` | 同上 |
| 安全模式：命令行含 `--next-safe-mode`，或 userData 落在 `electron-user-data\safe-mode` | `scan.rs` | 安全模式徽标与提示条失效 |
| `versions\<tag>` 链接指向的安装树被原地升级后，`app.asar!/package.json.version` 才是真实版本 | `versions.rs`（`reconcile`） | 不跟随 → 版本页显示旧版本号、在线列表认不出「已安装」、实例版本字段停在旧号 |

> 这些契约都是从 `app.asar` 的 `lib/main.js` 与实跑命令行取证得到的，不是猜测。
> 取证过程见 [../experience/02-进程扫描.md](../experience/02-进程扫描.md)、
> [../experience/08-原地升级与版本号.md](../experience/08-原地升级与版本号.md)、
> [../experience/09-数据目录确认框.md](../experience/09-数据目录确认框.md)。

## 参考指纹（`2.0.22-next`）

| 项 | 值 |
|---|---|
| `DSH NEXT.exe` 大小 | 246,521,344 B |
| `resources\app.asar` 大小 | 237,037,363 B |
| 设计 token 数 | 1191 |
| `design/tokens.lock.json` | 可复跑：同参数两次运行逐字节相同 |

## 如何适配新版本

```powershell
# 1) 在对新版本的 app.asar 上重新抽取 token（安装目录不在默认位置时要显式给 --asar）
node tools/extract-tokens.mjs --asar "E:\FXS\APP\DSH NEXT\resources\app.asar" `
  --out design/tokens.lock.json --css src/styles/tokens.css
# 2) 看差异：新增/删除的变量在 design/controls.md（带 pkg/file:符号 来源）；
#    删除的变量若被 src/styles/*.css 直接引用，门禁会当场报出来
# 3) 门禁
node tools/verify-tokens.mjs
node tools/lint-css.mjs
# 4) 回归
cargo test --manifest-path src-tauri/Cargo.toml
node tools/e2e.mjs
```

升级后**必查三件事**（都是"文档没变、行为变了"的那类）：

1. `lib/main.js` 里数据目录的两份记录（`desktop-next-location.json` /
   `desktop-next-last-home.json`）是否还是这个语义——决定要不要继续"同写同回滚"；
2. 进程命令行里 `--user-data-dir=` / `\profiles\desktop` 是否还成立——决定进程归属能否反推；
3. 版本树是不是原地升级（链接目标变了、链接名没变）——启动时的 `reconcile` 会跟着改名，
   但要确认 `package.json.version` 仍可读。

再按 [../testing.md](../testing.md) §3 做一次真实窗口多开的手工验收。

## 已知不兼容

| 情况 | 处理 |
|---|---|
| 同一个 home 起两个实例 | 拒绝（`E1003`）——两个 writer 会损坏会话，这是安全策略而非缺陷 |
| 安装目录缺 `resources\app.asar` | 拒绝登记（`E1004`），无法读取壳/内核版本 |
| 实例 home 落在 DSH-NEXT 安装目录内 | 2.0.22 起 DSH-NEXT 自己会拒绝（禁止把安装目录当数据目录） |
| 纯 webview 套壳、非 Electron 形态 | 不支持：没有 `--user-data-dir` 子进程，进程归属无法反推 |
