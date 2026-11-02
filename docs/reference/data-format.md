# 数据格式

启动器的数据分四块：全局指针（DSH-NEXT 的）、启动器配置、实例文件、版本树。

## 1. 全局指针（DSH-NEXT 的，不是我们的）

```
%APPDATA%\DSH NEXT\home\desktop-next-location.json
```

```json
{
  "home": "C:\\Users\\<用户名>\\.dsh"
}
```

- 由 **DSH-NEXT** 读取，决定"这次启动进哪个数据目录"；
- 启动器在启动实例前写入目标 home，启动成功后还原为默认 home；
- 写入前会备份为同目录下的 `.launcher-bak[N]`。

> 该文件属于 DSH-NEXT，启动器只是临时改写它。手工修改后需重启 DSH-NEXT 才生效。

## 2. 启动器配置 `app.json`

位置：`<数据目录>\app.json`（默认 `%LOCALAPPDATA%\DSH-NEXT-Multi\`，便携模式为 `<应用根>\DSH-NEXT-Multi-data\`）。

```json
{
  "schemaVersion": 1,
  "defaultHome": "C:\\Users\\<用户名>\\.dsh",
  "homeRoot": "C:\\Users\\<用户名>",
  "launchTimeoutSec": 90,
  "pollIntervalMs": 500,
  "restorePointerAfterLaunch": true,
  "downloadMirror": "",
  "onboarding": { "completedAt": "2026-11-03T01:30:00+08:00", "skippedAt": null, "lastStep": 5 },
  "lastPointer": { "home": "C:\\Users\\<用户名>\\.dsh-lab", "at": "2026-11-02T12:34:56+08:00" }
}
```

| 字段 | 说明 |
|---|---|
| `defaultHome` | 双击桌面图标应进入的环境；启动成功后指针还原到它 |
| `homeRoot` | 新建实例时 home 的**默认值**前缀（`<homeRoot>\.dsh-<id>`），不限制手改；**空串 = 跟随 `%USERPROFILE%`** |
| `launchTimeoutSec` | 启动超时，夹取 10–600 |
| `pollIntervalMs` | 进程轮询间隔，夹取 100–5000 |
| `restorePointerAfterLaunch` | 启动成功后是否把指针还原为 `defaultHome` |
| `downloadMirror` | 下载加速前缀（如 `https://gh-proxy.com`）；空 = 直连 GitHub。作用于安装包下载、版本目录 API、更新检查 |
| `onboarding` | 初始化向导状态：`completedAt`（完成后不再自动展开）、`skippedAt`（跳过时间）、`lastStep`（0–5，中断后回到的位置）。三项全空 = 从未引导过 |
| `lastPointer` | 上一次写指针的记录，仅用于诊断 |

## 3. 实例文件 `instances\<id>.json`

```json
{
  "schemaVersion": 1,
  "id": "lab",
  "name": "实验室",
  "version": "2.0.21-next",
  "home": "C:\\Users\\<用户名>\\.dsh-lab",
  "archived": false,
  "createdAt": "2026-11-01T09:00:00+08:00",
  "lastLaunchAt": "2026-11-02T12:30:00+08:00",
  "lastExitClean": true,
  "notes": "专门用来试新插件",
  "safeMode": false
}
```

| 字段 | 约束 |
|---|---|
| `id` | `[a-z0-9-]{1,32}`，创建后不可改，等于文件名 |
| `version` | 必须是 `versions\` 下某个目录名 |
| `home` | 绝对路径；未归档实例之间不允许重复 |
| `archived` | 归档只改标记，不动磁盘数据 |
| `lastExitClean` | 上次是否正常退出（false = 可能是崩溃） |
| `safeMode` | 上次启动是否带 `--next-safe-mode` |

## 4. 版本树 `versions\<tag>\`

指向已解包的安装目录的**目录链接（junction）**，不复制文件：

```
versions\
  2.0.21-next  →  <DSH-NEXT 安装目录>
  2.0.20-next  →  ...
```

- 有效性判定：`DSH NEXT.exe` 与 `resources\app.asar` 都存在；
- 壳版本/内核版本从 `app.asar` 内的 `package.json` 读取；
- 删除版本时只删链接，不动目标目录。

## 5. 日志 `logs\launcher-YYYYMMDD.log`

按天滚动，追加写。包含：启动参数、版本扫描、指针写入/还原、实例操作、错误详情。
报告问题请附最后 30 行（并脱敏用户名）。
