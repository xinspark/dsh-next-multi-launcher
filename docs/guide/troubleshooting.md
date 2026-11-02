# 故障排查

先看日志：`%LOCALAPPDATA%\DSH-NEXT-Multi\logs\launcher-YYYYMMDD.log`（便携模式在 `DSH-NEXT-Multi-data\logs\`）。

## 症状索引

| 症状 | 优先检查 | 详见 |
|---|---|---|
| 双击没反应 / 窗口一闪就退 | 日志是否有「窗口已创建」；`%TEMP%\DSH-NEXT-Multi-startup-error.txt` | [§1](#1-双击没反应) |
| 弹「无法创建窗口」 | 弹框里列出的 WebView2 数据目录候选，或 `DSH_MULTI_WEBVIEW_DATA` | [§1](#1-双击没反应) |
| 顶栏出现「指针目录不可写」 | `%APPDATA%\DSH NEXT\home` 的 ACL；是否继承了受限环境 | [§2](#2-指针不可写) |
| 启动失败 `E1003` | 该 home 是否已有别的实例在跑 | [§3](#3-e1003-数据目录已被占用) |
| 启动失败 `E1005` | 版本树完整性、杀软、指针权限 | [§3](#3-e1003-数据目录已被占用) |
| 启动失败 `E1006` | 版本与参数是否匹配（退出码在 detail 里） | [§3](#3-e1003-数据目录已被占用) |
| 重启后进了默认实例 | 是否在 DSH-NEXT 内部点了重启 | [§4](#4-重启相关) |
| 实例显示「安全模式」但没想进 | 上次启动是否带了 `--next-safe-mode` | [§4](#4-重启相关) |
| 双击图标进错环境 | 顶栏「全局指针」值与 `defaultHome` | [§5](#5-双击图标进错环境) |
| exe 图标还是旧的 | Windows 图标缓存 | [§6](#6-图标没更新) |

## 1. 双击没反应

判定顺序：

1. 日志里有没有「启动器 v… 启动」→ 没有说明进程根本没跑起来（被拦截 / 路径不可执行）；
2. 有没有「窗口已创建」→ 没有说明 WebView2 初始化失败（数据目录不可写，或运行时缺失）；
3. 有没有「窗口内尺寸」→ 没有说明窗口创建成功但后续初始化异常。

处理：

```powershell
# 换一个可写的 WebView2 数据目录再试
$env:DSH_MULTI_WEBVIEW_DATA = "$env:TEMP\dsh-multi-webview2"
.\DSH-NEXT-Multi.exe
```

程序会依次尝试：`DSH_MULTI_WEBVIEW_DATA` → `<数据目录>\webview2` → `<应用根>\DSH-NEXT-Multi-data\webview2` → `%TEMP%\DSH-NEXT-Multi\webview2`，全失败才弹框。

## 2. 指针不可写

`%APPDATA%\DSH NEXT\home` 如果曾被管理员进程创建，ACL 可能只允许管理员写。两种处理：

```powershell
# 方式一：以管理员身份运行启动器（应用也会自动请求提权）
# 方式二：把目录所有权还给当前用户
takeown /F "$env:APPDATA\DSH NEXT\home" /R /D Y
icacls "$env:APPDATA\DSH NEXT\home" /grant "$env:USERNAME":(OI)(CI)F /T
```

## 3. E1003 / E1005 / E1006

**E1003 数据目录已被占用**：同一 home 已经有一个实例在跑。停止它，或给新实例换 home。

**E1005 启动超时**（默认 90 秒内没看到该 home 的进程）：

- 版本树是否完整（`DSH NEXT.exe` + `resources\app.asar` 都在）；
- 杀软是否拦截了子进程；
- 指针目录是否可写（写不进去时 DSH 会落到默认环境，于是永远等不到目标 home）。

**E1006 启动后进程立即退出**：`detail` 里有退出码。常见于版本树损坏或参数不兼容。

**E1013 下载源请求失败**（版本页「在线下载」或更新检查）：

- GitHub 直连在国内经常不可达：先在设置里填**下载加速前缀**（如 `https://gh-proxy.com`）；
- 也可能是**未登录限流**：每小时 60 次/IP，界面会写明「HH:MM 后恢复」；
- 只是想装一次？点「打开 Releases」用浏览器下载，浏览器会走系统代理。

## 4. 重启相关

- **非默认实例的重启只能在启动器里做**：DSH-NEXT 内部重启会重新读全局指针，而指针在启动成功后已还原为默认 home，所以会落到默认实例；
- 用详情页/实例行的「重启 ▾」：重启 / 重启并进入安全模式 / 重启并退出安全模式；
- 想确认当前是否安全模式：实例状态行会显示「安全模式」徽标（判据：命令行含 `--next-safe-mode`，或 userData 落在 `electron-user-data\safe-mode`）。

## 5. 双击图标进错环境

顶栏显示的是**当前指针**。若与期望不符：

1. 点「恢复默认」把指针写回 `defaultHome`；
2. 检查设置里的「默认数据目录」是否是你想要的环境；
3. 如果 `defaultHome` 指向的目录不存在，界面会出现黄条并提供「按此目录新建实例」。

## 6. 图标没更新

重建 exe 后 Windows 可能仍显示旧图标（Explorer 缓存）：

```powershell
Stop-Process -Name explorer -Force; Start-Process explorer
# 或者清缓存后刷新
Remove-Item "$env:LOCALAPPDATA\IconCache.db" -Force -ErrorAction SilentlyContinue
ie4uinit.exe -show
```

## 诊断命令

```powershell
.\DSH-NEXT-Multi.exe --selftest       # 七项自检（路径/配置/版本/进程/实例/原子写/指针）
.\DSH-NEXT-Multi.exe --e2e-status     # 指针 + 版本 + 实例 + 运行中列表（JSON）
.\DSH-NEXT-Multi.exe --scan-debug     # 逐条打印扫到的 DSH 进程与解析出的 home
```

> release 版是 GUI 子系统、没有控制台：报告同时写到 `<数据目录>\cache\last-cli.json`，
> 也可用 `DSH_MULTI_CLI_OUT` 指定路径。
