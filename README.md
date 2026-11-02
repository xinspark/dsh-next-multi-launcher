# DSH-NEXT 应用多开启动器

> 中文名：**DSH-NEXT 应用多开启动器**　英文名：**DSH-NEXT Multi-Launcher**　可执行文件：`DSH-NEXT-Multi.exe`

管理 N 个**原生** DSH-NEXT 实例：真窗口、真托盘、真 Community Market，不是 webview 套壳。
每个实例有独立的**名称 / 数据目录（home）/ 壳版本**，实例之间会话、Bot 团队、凭据、日志、插件 profile、窗口状态各一份。

- 设计与机制依据：[docs/设计说明.md](docs/设计说明.md)
- 实施计划与验收矩阵：[docs/实施计划.md](docs/实施计划.md)
- 环境实测（含本机写盘边界）：[docs/env-check.md](docs/env-check.md)
- 验收记录（M0–M5 证据）：[docs/acceptance.md](docs/acceptance.md)

---

## 1. 它到底怎么做到“多开”

DSH-NEXT 只认一张全局指针文件：

```
%APPDATA%\DSH NEXT\home\desktop-next-location.json
{ "home": "C:\\Users\\FXS\\.dsh" }
```

进程启动时读一次 `home`，然后：

```
home → userData = <home>\electron-user-data → Electron 单实例锁以 userData 为身份
```

所以**不同 home = 不同 userData = 两把不同的锁 → 两个实例都能是 primary**。
启动器做的就是把“写指针 + spawn”串成一条串行管道，并在失败时回滚指针。

三条硬事实（实测见 `docs/env-check.md`）：

1. `DSH_DESKTOP_NEXT_HOME` 环境变量不可用 —— DSH-NEXT 启动后会自我重启一次，环境变量在重启后丢失；
2. 主进程命令行里**没有** `--user-data-dir`，home 要从 gpu/renderer/host 子进程反推，主进程 pid 由 host 的父进程得到；
3. 指针全局一份，因此启动必须**串行**：写指针与 spawn 之间不能被另一次启动插入。

## 2. 能力与边界

**能保证**

- 数据隔离：会话 / Bot / 凭据 / 日志 / 插件 profile / 窗口状态按 home 各一份；
- 多实例同时运行，互不抢锁；
- 启动失败一律回滚指针（超时、版本无效、启动即退出、用户取消）；
- 启动成功后按配置把指针还原成**默认 home**，保证双击桌面图标仍进日常环境；
- 删除数据目录前有安全网（盘根 / `%USERPROFILE%` / `%APPDATA%` / 默认 home / 启动器数据目录一律拒绝）。

**不保证（能力边界，不做承诺）**

| 项 | 说明 |
|---|---|
| Agents Anywhere 账号隔离 | 切 home 不隔离 AA 的登录与设备绑定，跨实例共享 |
| 内核独立 | 壳与 `@deepseek-ai/dsh` 恒同版本，所以只有“壳版本”一个旋钮 |
| 同 home 多实例 | 两个 writer 会损坏会话（seq-gap）；启动器会直接拒绝（E1003） |
| 自动更新管理 | 只提示风险，不改 `app-update.yml`；不写 DSH 安装目录、不改快捷方式、不写注册表 |

## 3. 目录与文件

```
<应用根>\
  DSH-NEXT-Multi.exe          ← 本启动器
  versions\<tag>\             ← 版本树（默认用目录 junction 指向真实安装，不复制 300MB）
  README.md  docs\  design\  tools\  src\  src-tauri\
```

```
%LOCALAPPDATA%\DSH-NEXT-Multi\
  app.json                    ← 全局配置（defaultHome / 超时 / 轮询 / 是否还原指针）
  instances\<id>.json         ← 每个实例一份
  logs\launcher-YYYYMMDD.log  ← 运行日志
  cache\                      ← 扫描缓存
```

**数据目录的判定（按可写逐级兜底）**：`DSH_MULTI_DATA_DIR` → `%LOCALAPPDATA%\DSH-NEXT-Multi` →
`<应用根>\DSH-NEXT-Multi-data`（便携模式）→ `%TEMP%\DSH-NEXT-Multi`。

> 在受限进程里（例如继承了沙箱、或系统用户目录被策略限制），前两档新建目录会被拒绝，
> 启动器会自动退到 **exe 旁边的 `DSH-NEXT-Multi-data\`**，所以**不需要管理员权限**也能运行。
> **不要用管理员身份运行**：提权不会改变用户目录的 ACL，反而会让指针/数据落到别的账户上下文。

**应用根目录的判定**：exe 所在目录；若 exe 在 `src-tauri\target\{debug,release}` 下（开发态），回退到工程根。
可用环境变量覆盖（排查 / 多屏 / 自动化测试用）：

| 变量 | 作用 | 例子 |
|---|---|---|
| `DSH_MULTI_DATA_DIR` | 启动器自己的数据目录 | `E:\test\data` |
| `DSH_MULTI_VERSIONS` | 版本树根目录 | `E:\test\versions` |
| `DSH_MULTI_POINTER` | 指针文件路径 | `E:\test\pointer.json` |
| `DSH_MULTI_WINDOW_SIZE` | 初始窗口尺寸（CSS 像素，`WxH`） | `1280x800` |
| `DSH_MULTI_CLI_OUT` | 命令行模式的报告落盘路径 | `E:\test\last-cli.json` |
| `DSH_MULTI_WEBVIEW_DATA` | WebView2 数据目录（默认 `<数据目录>\webview2`，不可写时自动退到临时目录） | `E:\test\webview2` |
| `DSH_MULTI_NO_DIALOG` | 启动失败时不弹框（自动化用） | `1` |
| `DSH_MULTI_NO_ELEVATE` | 不自动请求管理员权限（自动化 / 调试用） | `1` |

## 4. 使用

### 4.0 顶部使用提示（单行轮播）

指针栏下方有一条常驻提示，**单行轮播、每条 30 秒、不可关闭**：

| 轮播项 | 内容 |
|---|---|
| 重启限制 | 由于代码限制，目前非默认 DSH-NEXT 实例的「重启 / 重启到安全模式」只能在启动器里正常使用。 |
| 数据目录变更 | 由于多开机制，DSH-NEXT 实例启动时可能弹出「数据目录变更」提示，点击「继续」即可。 |

实现：`NOTICES` 数组 + `setInterval(NOTICE_MS)`，切换时用 `{#key}` + `in:fly` 做淡入。
调试时可用 `?noticeMs=800` 加速轮播。
### 4.1 数据目录与权限

- **数据目录树根**（设置 → 数据目录树根，`app.json` 的 `homeRoot`，默认 `%USERPROFILE%`）：
  只决定**新建实例时 home 的默认值** —— `<homeRoot>\.dsh-<ID>`，仍然可以自由改成任意位置。
- **默认数据目录**（`defaultHome`）：双击桌面图标应进入的环境，默认 `%USERPROFILE%\.dsh`。
  改 `homeRoot` 时，只有当 `defaultHome` 仍是旧根的 `.dsh` 才会跟着变，用户自定义过则不动。

### 4.2 管理员权限

启动器默认以普通权限运行；但若 **DSH 的指针目录**（`%APPDATA%\DSH NEXT\home`）
或 **WebView2 数据目录**曾被管理员身份的进程创建过，ACL 可能只允许管理员写入，
此时普通权限会写不了指针（启动实例失败 `E1001`）。

启动器在启动时会检测：

- 指针目录不可写，或数据目录已退到 `DSH-NEXT-Multi-data\`（说明标准位置不可写）
  → **自动以管理员身份重启**（弹一次 UAC）；
- 拒绝 UAC 或设置 `DSH_MULTI_NO_ELEVATE=1` → 继续以普通权限运行，界面顶部显示红条
  「当前是标准用户，且指针目录不可写…」并给出「以管理员身份重启」按钮；
- 也可以直接右键 exe →「以管理员身份运行」。

> 根治办法（把目录所有权还给当前用户，之后就不需要管理员）：
> ```powershell
> takeown /F "%APPDATA%\DSH NEXT\home" /R /D Y
> icacls "%APPDATA%\DSH NEXT\home" /grant "%USERNAME%":(OI)(CI)F /T
> ```

1. **首次启动**：若 `versions\` 为空，启动器会在本机常见位置（正在运行的 DSH-NEXT 进程、`%LOCALAPPDATA%\Programs\DSH NEXT`、`E:\FXS\APP\DSH NEXT`、`%ProgramFiles%\DSH NEXT`）找到一份安装树，并建一条目录链接登记；若还没有任何实例，会为默认 home（`%USERPROFILE%\.dsh`）建一个 `default` 实例。
2. **新建实例**：名称必填，ID 自动转写（可改；ID 由名称统一哈希生成，如 `灵枢-汐 → inst-2wmon31b`，短且稳定，未手改时随名称变化），home 默认 `<homeRoot>\.dsh-<id>`（可改），版本下拉选择。
3. **启动**：串行队列 → 写指针 → spawn → 轮询 `<home>\electron-user-data` 是否出现 → 成功 / 失败（失败回滚指针）。
4. **停止**：先请求关闭窗口（15 秒），超时弹确认后强制结束进程树。
5. **归档**：从主列表移入「已归档」，数据目录不受影响。
5.1 **重启 ▾**：实例行与详情面板底部各一处（都在【启动/停止】**之前**），**两项**：「重启」+ 按当前状态二选一（进入/退出安全模式）；
   重启期间顶部会出现提示条「正在重启「X」…（停止 → 重新启动，请稍候）」，实例起来后自动收起；
   若实例运行在安全模式，顶部常驻提示条「「X」正在安全模式运行：插件与 Profile 未加载」+「退出安全模式」按钮。
   实现为“先停止、再按模式重新启动”，安全模式参数取自 DSH-NEXT 自身（`--next-safe-mode`）。

6. **删除**：输入实例 ID 确认；可勾选“同时删除数据目录”（默认 home 锁定不可勾选）；删除前过安全网。
7. **导入版本**：选一个已解包的安装目录，校验 `DSH NEXT.exe` + `resources\app.asar` 后建链接登记；不复制文件。

   > 登记后 `versions\<tag>` 是一个 **目录链接（junction）**，不是复制：资源管理器点进去看到的是真实文件列表，
   > 因为 NTFS 会透传到目标目录。想确认：
   > ```powershell
   > cmd /C dir /AL versions                      # 显示 <JUNCTION> 2.0.21-next [目标]
   > Get-Item versions\2.0.21-next | Format-List LinkType,Target
   > cmd /C fsutil file queryfileid "versions\2.0.21-next\DSH NEXT.exe"   # 与目标里同一文件应返回相同 File ID
   > ```
   > 界面里「导入版本」对话框会直接给每个版本标 `链接`（悬停看目标）或 `真实目录`。
   > 删除链接请用 `cmd /C rmdir versions\<tag>`（只删链接，不动目标）。

界面语言支持中文 / English（右上角切换，记在 localStorage）。

## 5. 错误码

| 码 | 场景 | 文案 |
|---|---|---|
| E1001 | 指针写入失败 | 无法写入数据目录指针（权限或占用） |
| E1002 | 指针回滚失败 | 指针回滚失败，请手工检查指针文件 |
| E1003 | home 已被占用 | 数据目录 X 已被实例「Y」使用 |
| E1004 | 版本无效 | 版本目录无效：缺少 DSH NEXT.exe 或 app.asar |
| E1005 | 启动超时 | 启动超时（N 秒内未看到该目录的进程） |
| E1006 | 启动即退出 | 启动后进程立即退出（退出码 N） |
| E1007 | 路径不安全 | 该路径不允许删除（盘根/系统目录/默认环境） |
| E1008 | ID 已存在 | 实例 ID 已存在 |
| E1009 | 实例不存在 | 实例文件缺失或损坏 |
| E1010 | 版本被引用 | 该版本正被实例引用，不能删除 |
| E1011 | 数据目录被占用 | 数据目录仍有进程占用，先停止实例 |

## 5.1 构建产物

| 产物 | 位置 | 说明 |
|---|---|---|
| 便携版 | 应用根 `DSH-NEXT-Multi.exe` | 双击即用，不需要安装；`versions\` 与它同级 |
| release 原始产物 | `src-tauri\target\release\DSH-NEXT-Multi.exe` | 同上，构建目录里的原件 |
| 安装包 | `src-tauri\target\release\bundle\nsis\*.exe` | 只有 `pnpm tauri build` 全量成功才有 |

## 6. 开发与构建

工具链（阶段 0 实测全绿）：Node ≥ 20（本机 DSH 自带 v24.21.0）、pnpm 11、Rust 1.98、WebView2 140、Tauri 2。

```powershell
cd "E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher"

pnpm install                 # 前端依赖
pnpm tauri dev               # 开发（Vite 1420 + WebView）
pnpm build                   # 只构建前端 → dist/
pnpm check                   # svelte-check（0 error / 0 warning）
pnpm tauri build --no-bundle # release exe → src-tauri\target\release\DSH-NEXT-Multi.exe
pnpm tauri build             # 额外出 NSIS 安装包（需普通用户态终端）
cargo test --manifest-path src-tauri/Cargo.toml   # 43 项 Rust 单测

node tools/extract-tokens.mjs --out design/tokens.lock.json --css src/styles/tokens.css
node tools/verify-tokens.mjs                      # 原生对齐门
node tools/e2e.mjs                                # 启动管道端到端验收
```

### 原生对齐门（verify-tokens）

四条判据，任一不过退出码 1：

- **A** `src\**\*.{css,svelte}` 里每个 `var(--x)` 必须出现在 `design/tokens.lock.json`；
- **B** `src\styles\tokens.css` 的 sha256 必须等于锁里记录的（改一个字节就红）；
- **C** `design\parts\*.css` 逐份 sha256 必须与锁一致；
- **D** 手写样式里不允许出现 hex / `rgb()` / `hsl()`（颜色只能来自 token）。

DSH-NEXT 升级后的对账流程：重跑 `extract-tokens`（带 `--css`）→ 看 diff → 重跑 `verify-tokens` 通过后抬基线版本号。

## 7. 故障排查

| 症状 | 先看什么 |
|---|---|
| 启动后窗口没出现，状态停在「启动中 Ns…」 | `%LOCALAPPDATA%\DSH-NEXT-Multi\logs\` 当天日志；确认 `versions\<tag>\DSH NEXT.exe` 存在 |
| 失败 E1003 | 该 home 已有实例在跑（任务管理器里找 `DSH NEXT.exe` 的 `--user-data-dir`） |
| 失败 E1005 | 90 秒内没看到目标 home 的进程：版本树不完整、被杀软拦截、或指针目录无写权限 |
| 重启后进了默认实例 | 指针在启动成功后即还原为默认（设计如此，保证双击图标进日常环境）。**重启请用启动器的「重启 ▾」**，不要在 DSH 内部点重启 |
| 失败 E1006 | 版本树损坏（退出码在错误详情里）；`app.asar` 或 `DSH NEXT.exe` 不匹配 |
| 双击桌面图标进错环境 | 顶栏看「全局指针」；点「恢复默认」即可 |
| 双击后弹「无法创建窗口」 | 弹框会列出**已尝试的 4 个 WebView2 数据目录**及各自原因；把 exe 换到可写目录（桌面 / D 盘）再试，或用 `DSH_MULTI_WEBVIEW_DATA` 指定目录 |
| 顶栏出现「指针目录不可写」黄条 | 这个进程对 `%APPDATA%\DSH NEXT\home` 没有写权限（常见于从 DSH 窗口/终端里启动）。请在**资源管理器里双击**运行；否则点「启动」会失败 E1001 |
| 双击后没反应 / 窗口一闪就退 | 看 `%LOCALAPPDATA%\DSH-NEXT-Multi\logs\launcher-YYYYMMDD.log`；若日志里是「窗口已创建」之前就断了，看 %TEMP%\DSH-NEXT-Multi-startup-error.txt。**旧版本确实会静默退出**，新版本失败会弹框并写明原因 |
| 启动器本身起不来 | 先跑 `DSH-NEXT-Multi.exe --selftest`；release 版是 GUI 子系统没有控制台，报告同时写 `<data_dir>\cache\last-cli.json` |

命令行自检（不进 GUI）：

```powershell
DSH-NEXT-Multi.exe --selftest        # 七项自检：路径/配置/版本/进程/实例/原子写/指针
DSH-NEXT-Multi.exe --e2e-status      # 当前指针 + 版本 + 实例 + 运行中实例
DSH-NEXT-Multi.exe --scan-debug      # 排障：逐条打印扫到的 DSH 进程与解析出的 home
```

> release 构建是 Windows GUI 子系统（双击不留黑框），没有控制台：
> 每条命令的报告都会同时写到 `<data_dir>\cache\last-cli.json`（可用 `DSH_MULTI_CLI_OUT` 指定别的路径）。

## 10. 代码签名（消除「未知发布者」）

exe 默认**没有数字签名**，Windows 会弹两道检查：

| 弹窗 | 触发原因 | 能否消除 |
|---|---|---|
| 「打开文件 - 安全警告：无法验证发布者」 | exe 未签名 | ✅ 自签名后消失 |
| 「Windows 已保护你的电脑」（SmartScreen 红屏） | 未签名 + 无信誉；每次重建哈希都变 | ❌ 自签名无效，需 EV 证书或信誉积累；点「更多信息 → 仍要运行」 |

本机自签名：

```powershell
pwsh -File tools\sign-exe.ps1
```

脚本会：复用/创建 `CN=DSH-NEXT Multi-Launcher` 代码签名证书 → 加入当前用户「受信任的根」与「受信任的发布者」→
用 `Set-AuthenticodeSignature` 签名（带时间戳，失败则退回无时间戳）→ 复核 `Status = Valid`。

注意：

- **每次重建 exe 都要重新签名**（哈希变了，签名随之失效）；
- 证书是**本机信任**，换机器要重新执行一次；
- 自签名证书有效期内（5 年）签名有效；无时间戳的签名在证书过期后会失效。

图标是代码绘制的（`tools/icon-designs.mjs`，纯 Node + zlib，无第三方依赖），三个方向：

| 方案 | 表达 | 文件 |
|---|---|---|
| A 堆叠实例（**采用**） | 三张错位卡片 = 多份隔离数据，白卡 = 当前实例，绿点 = 运行中 | `design/icons/A-stacked.png` |
| B 四宫格 | 四个窗口，一个高亮 = 多开 + 选中 | `design/icons/B-grid.png` |
| C 启动器 | 窗口 + 播放按钮 = 启动器 | `design/icons/C-launcher.png` |
| D 鲸鱼联动 | 复用 DSH-NEXT 官方鲸鱼剪影 + 同构胶囊（三个点 = 多实例） | `design/icons/D-linked-whale.png` |
| E 四宫格 + 鲸鱼（**最终采用**） | 2×2 四格 = 多开；激活格品牌蓝 + 官方鲸鱼剪影 = 当前实例 + 品牌联动 | `design/icons/E-grid-whale.png` |

对比图：`design/icons/preview.png`。

界面左上角的品牌方块也用同一图标（`src/assets/brand.png`，128px 由 E 方案缩放而来），
不再是占位的 "M" 字。

**E 方案（采用）怎么来的**：四宫格（间距 24px）表达多开；激活格用品牌蓝 `#2563EB` 实心 + 官方鲸鱼剪影（蒙版缩放到格内 420px），其余三格压暗 `#181C24` 保证小尺寸下对比。

**鲸鱼素材怎么来的**：官方图标在 `E:\FXS\APP\DSH NEXT\resources\app.asar.unpacked\build\app-icon.png`
（16-bit PNG）。`tools/icon-linked.mjs` 自带最小 PNG 解码器（含 16-bit 与五种滤波），
按亮度阈值提取**鲸鱼蒙版**（限定 y∈[130,770]，排除顶部高光与官方 NEXT 胶囊），
再以近黑底（#151515）+ 白色鲸鱼 + 右下角三点胶囊合成——与官方同底、同鲸鱼、同胶囊位置，
一眼能看出是同一家族的多开版。

```powershell
node tools/icon-linked.mjs    # → design/icons/D-linked-whale.png
```

换图标：

```powershell
node tools/icon-designs.mjs                 # 重新生成三个方案
Copy-Item design/icons/A-stacked.png src-tauri/icons/source.png -Force
pnpm tauri icon src-tauri/icons/source.png  # 派生 ico/icns/各尺寸
cargo clean --manifest-path src-tauri/Cargo.toml -p dsh-next-multi-launcher --release
pnpm tauri build --no-bundle                # 必须重建：图标在 build script 里嵌入
```

> 坑：只改 `icons/` 再 `pnpm tauri build` **不会**更新 exe 内嵌图标——
> `tauri-build` 的 build script 有缓存，必须 `cargo clean -p dsh-next-multi-launcher --release`
> （或改一下 `build.rs` 的时间戳）才会重新嵌入。

## 8. 许可与出处

界面设计变量（颜色 / 圆角 / 字号 / 阴影）全部从本机 DSH-NEXT 的 `resources\app.asar` 抽取，
见 `design\tokens.lock.json` 与 `design\controls.md`；启动器不修改 DSH-NEXT 本体。
