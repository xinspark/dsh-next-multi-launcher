# 配置项与环境变量

## 设置面板（顶栏「设置」）

| 字段 | 默认 | 说明 |
|---|---|---|
| 数据目录树根 `homeRoot` | `%USERPROFILE%` | 决定**新建实例时 home 的默认值**：`<homeRoot>\.dsh-<id>`；**留空 = 跟随 `%USERPROFILE%`**，仍可手改 |
| 默认数据目录 `defaultHome` | `%USERPROFILE%\.dsh` | 双击桌面图标应进入的环境；启动成功后指针还原到它 |
| 下载加速前缀 `downloadMirror` | 空（直连） | GitHub 在国内常不可达时填写，如 `https://gh-proxy.com`；作用于**安装包下载、版本目录 API、更新检查**。第三方服务会失效，失效就换或清空 |
| 启动超时 | 90 秒 | 夹取 10–600 |
| 轮询间隔 | 500 毫秒 | 夹取 100–5000 |
| 启动成功后还原指针 | 开 | 关掉后指针会停留在最后启动的实例 |
| 运行权限 | 只读显示 | 管理员 / 标准用户 |

改「数据目录树根」时：只有当 `defaultHome` 仍等于「旧根的 `.dsh`」才会跟随，用户自定义过就不动。
留空保存等价于「跟随 `%USERPROFILE%`」。

## 初始化向导

首次启动（`app.json` 的 `onboarding.completedAt` 与 `skippedAt` 都为空）会自动展开，
五步：语言（默认跟随系统）→ 环境体检 → 默认数据目录树根 → 选择 DSH-NEXT → 实例。
「跳过」或「新建实例」都会写 `skippedAt`，之后不再自动展开；
设置 → 维护 →「重新运行初始化向导」可随时重开。字段含义见
[数据格式](../reference/data-format.md)。

## 环境变量（覆盖用，正常使用不需要）

| 变量 | 作用 | 示例 |
|---|---|---|
| `DSH_MULTI_DATA_DIR` | 启动器数据目录 | `D:\test\data` |
| `DSH_MULTI_VERSIONS` | 版本树根目录 | `D:\test\versions` |
| `DSH_MULTI_POINTER` | 指针文件路径 | `D:\test\pointer.json` |
| `DSH_MULTI_WINDOW_SIZE` | 初始窗口尺寸（CSS 像素） | `1280x800` |
| `DSH_MULTI_WEBVIEW_DATA` | WebView2 数据目录 | `D:\test\webview2` |
| `DSH_MULTI_CLI_OUT` | 命令行报告落盘路径 | `D:\test\last-cli.json` |
| `DSH_MULTI_SELFTEST_DIR` | 自检临时目录 | `D:\test\selftest` |
| `DSH_MULTI_NO_ELEVATE` | 不自动请求管理员权限 | `1` |
| `DSH_MULTI_NO_DIALOG` | 启动失败时不弹框 | `1` |

> 上图这些主要用于自动化测试与受限环境排障。日常使用只改设置面板即可。

## 数据目录的兜底顺序

```
DSH_MULTI_DATA_DIR
  → %LOCALAPPDATA%\DSH-NEXT-Multi        （标准安装）
  → <应用根>\DSH-NEXT-Multi-data         （便携模式）
  → %TEMP%\DSH-NEXT-Multi                （最后兜底）
```

`app.json`、`instances\`、`logs\`、`cache\`、`webview2\` 都在这一个目录下。

## 命令行

| 命令 | 用途 |
|---|---|
| `--selftest` | 七项自检，输出 JSON（GUI 子系统无控制台时写 `cache\last-cli.json`） |
| `--e2e-status` | 指针 + 版本 + 实例 + 运行中列表 |
| `--e2e-launch <id>` / `--e2e-stop <id> [--force]` | 脚本化启动/停止（验收用） |
| `--scan-debug` | 逐条打印 DSH 进程与解析出的 home |
