# 更新日志



格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 变更

- **样式改造：以原生「插件详情页」为标尺**。逐项对齐原生控件的几何、字阶与表面，
  规格与取证见 `docs-private/样式改造.md`（原生 CSS 真值存档在 `docs-private/ref-plugin-detail-css.txt`）：
  - 圆角按元素层级分档：卡片 `8 → radius-lg(16)`、输入框 `8 → radius-md(12)`；
    此前控件层几乎全用 `radius-sm`，是"不像原生"的最大单一来源
  - 输入框对齐原生 `Input`：描边 `border-l3 → l4`、底色 `bg-layer-2 → bg-layer-1`、内边距 `10 → 8`；
    卡片内边距 `12/14 → 12`、间距 `10 → 8`
  - 页标题 `15px/600 → 20/28/500`（原生 `pageTitle`）；向导标题 `15/600 → 18/26/600`（原生 `wizardTitle`）
  - 13 处裸 `font-size` 改为 token 或「原生档位 + 显式行高」，`font-weight: 700` 收回 600
  - 顶栏视图切换、版本渠道切换、语言、主题统一成原生 `SegmentedControl` 形态：
    轨道 `radius-md` + `interactive-bg-hover`，选中项是抬起的 `bg-layer-1` 胶囊 + `shadow-lv1`
  - 顶部提示条改为原生 `guide` 块（`border-l4` + `radius-lg` + `bg-layer-1`）；
    黄条 / 红条改为原生 `approval` 块（`radius-lg` + `warn-tertiary` 底 + 40% 混色描边）
  - 「下一条」由裸文字按钮改为小号 ghost 规格（24px / `radius-sm` / `border-l3`）
  - 焦点环改走 `--dsw-focus-ring-color` + 输入模态分流（鼠标点击不画环、键盘导航才画）；
    补 `prefers-reduced-motion` 降级；折叠条改由 `aria-expanded` 驱动箭头旋转
- **两类浮层分清楚**（此前一律按同一种画，导致"半透明看得见穿"）：
  - 菜单（重启等）用官方 **MenuSurface 三件套**：`--dsw-menu-surface-fill` + `blur(40px) saturate(150%)`
    + `elevation-prominent`；行高 32px / `radius-md` / 16px 图标位
  - 选择器（版本等）用 **registry 浮层**：`registryToggle` 闭合态 + `registry` / `registryOption` 展开态
    （选中行加粗 + 对勾）；原生 `<select>` 的弹出列表无法上样式，已由 `SelectMenu.svelte` 取代
- **顶栏改三列栅格**：左右各 `1fr`、中间 `auto`，分段控件永远停在窗口正中，
  不再随右侧按钮数量（启动页多一个「新建实例」）左右漂移
- **页头位置统一**：指针条与全部横幅上移到三视图共用区，启动视图补上 `page-head`
  （标题「启动」+ 运行中/实例数）；三视图标题实测落在同一个 y 与左边距
- 补原生 `Switch`（36×20 / 16 拇指 / 状态键在 `aria-checked`）：设置页「恢复指针」「自动检查更新」
  由复选框换成开关；**多选**（版本候选、删除确认）继续用 `checkbox`，按原生 `Checkbox` 对齐 16×16 + `accent-color`
- **图标**：新增原生图标渲染口，path 逐字取自 `dsh-client-ui-primitives` 的 `Icon*Artwork`
  （16×16 / `fill:none` / 描边继承 `currentColor`），替换 `✕`、`▾`×2、`▸`、`+` 四处字符图形
  （「新建实例」文案同步去掉前置加号）；重启按钮箭头改为 12px 原生 chevron
- 「新建实例」主操作用中号按钮（36px，原生 `Button.md`），与顶栏其余小号按钮区分层级

### 修复

- **菜单在窗口底部被裁**：详情面板底部那颗菜单总是向下开，开出窗口一半不可见。
  加 `position-try-fallbacks: flip-block`，空间不够自动向上翻
- **菜单悬停看不见**：`button:hover` 用了与浮层底色相同的 `bg-layer-2`，改成 `interactive-bg-hover`
- **横幅被拉成整屏高**：`.app` 原本写死 5 行栅格，而横幅数量在 0~7 条之间变化，
  多出来的一条会落进 `1fr` 行；改成纵向 flex + 内容区 `flex: 1`
- **版本页「本地导入」**的输入框占位符是开发者本机路径（且反斜杠被双重转义），改为字典文案；
  该行补上「选择目录」按钮

### 新增

- `tools/lint-css.mjs`（`pnpm lint:css`，已接入 CI）：样式门禁四条规则——禁裸颜色、圆角只用 token、
  **字号必须与行高成对**、字重不出界
- `src/lib/Icon.svelte` + `src/lib/icons.ts`：原生图标的唯一渲染口与 path 名册
- `src/lib/Switch.svelte`：原生开关
- `src/lib/SelectMenu.svelte`：原生风格下拉选择（替代原生 `<select>`）
- `src/lib/DirPickButton.svelte`：「选择目录」按钮，设置 / 向导 / 版本导入三处共用

### 维护

- **样式按层拆分**：`app.css`（1519 行）→ `base.css` / `chrome.css` / `panes.css` / `views.css`，
  导入顺序即层叠顺序；拆分前后声明序列逐条比对一致（1219 条）
- **生成物不再记录本机路径**：`extract-tokens` 的 asar 位置改为 `--asar` → `DSH_ASAR` → 常见安装目录，
  lock / controls / 生成的 `tokens.css` 头部只写可移植路径；
  `icon-linked` 同理（`DSH_APP_ICON`）
- 删除一次性宣传脚本 `tools/promo/`（引用私有路径与凭据文件）；测试夹具里的本机路径统一为中性示例

## [1.0.5] - 2026-11-03

### 新增

- **主题系统**：设置 → 常规里可选 `跟随系统 / 浅色 / 深色`，默认跟随系统；
  实现上 `system` 交给 CSS 的 `prefers-color-scheme`（token 里已有该分支），
  `light`/`dark` 用 `data-theme` 显式覆盖，保存后立即生效。
  **原生窗口外框**同步切（`apply_window_theme`，否则浅色界面会顶一个深色标题栏），
  **应用图标**也换成浅色变体（浅色底板 + 保留品牌蓝鲸鱼块）
- **换新图标**：蓝色底 + 白色 DSH 鲸鱼，右下角白色胶囊下标「NUL」（对齐 DSH-NEXT 图标的 NEXT 下标样式）；	auri icon 重新生成整套图标，exe 内嵌图标同步
- 顶部提示条启动时**随机**选一条开始轮播（此前固定从第 1 条开始，后面几条几乎看不到）
- **关于更新**新增常驻「前往浏览器下载」按钮（有更新时指向该 Release，否则指向 Releases 列表），
  并提示「下载后需要手动替换旧版本」——本启动器不自动替换文件
## [1.0.4] - 2026-11-03

### 变更

- **启动器数据目录移到「文档」**：`<文档>\DSH-NEXT-Multi`（跟随用户重定向，可能在 D: 或 OneDrive）。
  老位置的 `%LOCALAPPDATA%\DSH-NEXT-Multi` 会在首次启动时**搬一次**：同卷直接 rename，
  跨卷只搬配置/实例/备份/日志，缓存重建
- **在线状态统一刷新**：版本目录与「关于更新」一起拉——启动时一次，之后**每 10 分钟**一次；
  切换视图/渠道不再触发请求，其余时候只有手动「刷新」
- 设置页标签更明确：「实例数据目录」「新实例数据目录树根」「DSH-NEXT默认数据目录」「启动器维护」
- 版本页：`刷新` 按钮移到渠道 tab 同一行，标题行显示「上次刷新 HH:MM:SS」
- 首次拉取时 DSH-NEXT **正式 + 预览**一次拉齐，切 tab 不再等网络

### 修复

- **便携模式被误判为需要提权**：`needs_elevation` 原先用「数据目录是否等于标准路径」推断，
  导致自定义数据目录时横幅谎称「指针目录不可写」。现在只取决于指针目录能否写
- 向导副标题写着「四步」，实际是五步

## [1.0.3] - 2026-11-03

### 新增

- **三视图布局**：顶栏 `启动 / 版本 / 设置`，`#launch` / `#versions` / `#settings` 可深链；
  提示条跨三视图常驻；设置与导入从弹窗改为独立页面
- **版本页**：已登记版本、在线下载（DSH-NEXT 正式 / 预览 / 原生 DSH 三渠道）、本地导入三段合一
- **在线版本目录**：GitHub Releases 枚举（正式取 `-next`、预览取 `prerelease`），
  每条给出 tag / 日期 / 大小 / 是否已安装；原生 DSH 标注为「仅 latest、无公开哈希」
- **设置页**：常规（语言）、数据目录（树根 + 默认环境）、启动（超时 / 轮询 / 还原指针）、
  维护（打开数据/版本/日志目录、GitHub）、关于更新（检查更新 + 启动静默检查）、
  关于启动器（版本、MIT、求星卡片）；底部脏检测保存栏
- **检查更新**：`update_check` 命令（只提示不下载）+ `--check-update` 排障入口；
  版本号按数字段比较（`1.0.10 > 1.0.9`）
- **打开链接**：`open_url` 命令（仅允许 http/https），供求星、更新说明、下载页使用

### 修复

- **在线下载卡在「正在获取版本列表…」，既不成功也不失败**：
  原先在 `$effect` 里调用 `load()`，而 `load()` 会读并写 `catalogs` / `catalogErrors`，
  effect 被自己的写入反复触发形成微任务死循环，IPC 回包永远排不上队
  （转圈是 CSS 动画，所以看起来还在动）。改为挂载时拉一次，之后由切 tab / 重试显式触发；
  同一渠道已有请求在飞时不再重复发送
- **在线下载优先给安装包**：同一 release 同时挂安装包与便携包时按优先级挑选
  （`Setup` / `Installer` / 非便携 `.exe` → `.zip` → `*Portable.exe`），行内标注「安装包 / 便携版」
- **下载源请求不再冻界面**：`catalog_list` / `update_check` 改为异步命令 + 阻塞线程池。
  此前是同步命令（跑在主线程），GitHub 请求最长 10 秒会把窗口一起卡住，加载动画根本画不出来
- **原生 DSH 提示改正**：明确「本启动器不支持原生 DSH 的多开与版本导入」，
  不再误称安装后可用「自动扫描」导入
- **构建：关闭 JS/CSS 压缩**。vite 6 锁定的 esbuild 0.25.12 在压缩本工程产物时会在
  minifier 里段错误（`mangleIf` → `SimplifyUnusedExpr`，`unexpected fault address`），
  且与源码无关（HEAD 版同样崩、同一份源码时崩时不崩）。`build.minify: false` 后构建稳定；
  代价是 bundle 123 KB → 287 KB（桌面端本地加载，无实际影响）

### 新增

- **在线下载的加载态与失败态**：按渠道独立显示失败原因、可原地「重试」；
  GitHub 渠道失败时附国内网络说明；失败时「打开 Releases / GitHub」入口仍可用
- **新错误码 `E1013 下载源请求失败`**：此前任何目录错误都报成 `E1012 参数无效`（误导）；
  现在 GitHub 403/429 会被翻成「限流：未登录每小时 60 次，HH:MM 后恢复；也可用「打开 Releases」」
- **在线下载默认只列最新 3 个**，其余折叠（`展开全部 N 个` / `收起`）
- **`latest` 条目（实时，不写死）**：每次加载都用当次 GitHub 请求的结果现算——
  取最新 release 的版本号、安装包地址与内核版本，复制出一条 `tag=latest` 的条目；
  行内显示「应用 vX」+「内核 X」，和普通版本一样可下载
- **每个版本标注对应内核版本**：从 release 正文的 `上游版本：DeepSeek Harness vX` 实时解析
  （解析不到就留空，不猜）
- **下载加速前缀（设置里可改，默认空）**：GitHub 在国内不可用时，把安装包地址、
  **版本目录查询（API）**与**启动器自身更新检查**一起拼到该前缀后面
  （实测 `gh-proxy.com` 能代理 `api.github.com`，`ghproxy.net` 只代理下载不代理 API）。
  不写死任何第三方服务——免费加速随时会失效，失效就换一个或清空；
  命令行 `--catalog` / `--check-update` 也读同一份设置，排障时行为与界面一致
- **原生 DSH 行显示「内核 —」**：官网该渠道不公开内核版本
  （`latest.json`/`latest.yml`/`version.json` 全 404，安装器响应头只有 `Last-Modified`），
  悬停给出原因，而不是把字段留空
- **初始化向导（首次启动单页面板）**：① 语言（默认跟随系统）→ ② 环境体检 →
  ③ 默认数据目录树根（只做文件夹选择，留空=跟随系统默认）→ ④ 选 DSH-NEXT →
  ⑤ 实例（已有实例时说清楚，可「新建实例」直接跳主界面新建对话框）。
  从未完成/跳过时自动展开；「跳过」写入 `skippedAt` 后不再打扰；
  设置页「重新运行初始化向导」可随时重开。任何一步失败都不阻塞，底部按钮固定在面板底部。
  规格见 [docs/design-onboarding.md](docs/design-onboarding.md)

### 修复

- **`home_root` 允许留空**（= 跟随 `%USERPROFILE%`），此前留空会报 `E1012`
- **两处 i18n key 拼错导致界面露 key 名**：`version.removed`/`version.imported`
  应为 `versions.*`；另补齐 6 个只在中文里存在的 `versions.*` 英文文案
  （现中英各 287 条，无缺口）

### 移除

- 设置弹窗与导入版本弹窗（内容已迁到对应视图）

### 计划中

- 内置下载器（下载 → sha256 校验 → 解压 → 自动登记；当前为浏览器下载）
- 每实例重定向 `%APPDATA%`（实验性，彻底隔离多开重启）

## [1.0.2] - 2026-11-02

### 修复

- **自动扫描扫不出中文路径**：改用 Windows 原生 API（COM `IShellLink` + `winreg`），
  不再调用 PowerShell。此前经子进程 stdout 往返时，PowerShell 5.1 的系统代码页会把
  `D:\豆包\…`、`E:\DSH-工具` 变成 `D:\?????-`，候选全部被判为无效目录；
  现在直接读 UTF-16，且**不依赖任何外部命令**（详见 [docs/experience/07](docs/experience/07-扫描乱码.md)）
- 快捷方式解析用 `SLGP_RAWPATH`，目标被移动后仍能拿到原始路径
- 注册表 `DisplayIcon` 现在会去掉引号与 `,<index>` 后缀再取目录

### 验证

- 53 项 Rust 单测（新增「中文路径原样保留」回归用例）
- 中文路径夹具实测：`target\中文测试\DSH 安装` → `valid=true`、`shellVersion=2.9.9-cn`

## [1.0.1] - 2026-11-02

### 新增

- **自动扫描安装目录**：导入版本时可一键发现本机已安装的 DSH-NEXT，来源依次为
  快捷方式（开始菜单 / 桌面）、注册表卸载项、常见安装路径、运行中的进程；
  结果按有效性与是否已登记排序，可勾选后批量导入
- 排障入口 `--discover`：命令行打印扫描结果（来源 / 路径 / 版本 / 是否有效）

### 修复

- `versions::install_candidates` 里硬编码了某一台机器的安装路径，
  在别人的机器上恒为无效候选；改为只依据环境变量与运行中进程推断

## [1.0.0] - 2026-11-02

首个对外版本。

### 新增

- 多实例管理：每个实例独立的名称、ID、数据目录（home）与 DSH-NEXT 版本
- 串行启动队列：启动前写入全局指针，失败自动回滚，成功后还原为默认 home
- 版本树：以目录链接（junction）登记已解包安装目录，不复制文件
- 实例操作：启动 / 停止（优雅 → 超时 → 强制）/ 归档 / 删除 / 重启
- 重启下拉：重启、重启并进入安全模式、重启并退出安全模式（`--next-safe-mode`）
- 安全模式监测：按进程命令行与 userData 目录识别，界面显示徽标与提示条
- 权限探测：关键目录不可写时自动以管理员身份重启，界面提供手动入口
- 原生对齐：从 DSH-NEXT `app.asar` 抽取设计 token，`verify-tokens` 四规则门禁
- 端到端验收：替身进程验证启动管道（A1–A6），15 项断言
- 本机自签名：`tools/sign-exe.ps1` 消除「未知发布者」提示

### 已知限制

- 非默认实例的重启只能在启动器内进行（DSH-NEXT 内部重启会重读全局指针）
- 多开时全局指针唯一，同时运行多个非默认实例时只有最后启动的那个能正确重启
- SmartScreen 对未签名 exe 的红色拦截无法用自签名消除

[Unreleased]: https://github.com/XinSpark/dsh-next-multi-launcher/compare/v1.0.5...HEAD
[1.0.5]: https://github.com/XinSpark/dsh-next-multi-launcher/releases/tag/v1.0.5
[1.0.4]: https://github.com/XinSpark/dsh-next-multi-launcher/releases/tag/v1.0.4
[1.0.3]: https://github.com/XinSpark/dsh-next-multi-launcher/releases/tag/v1.0.3
[1.0.0]: https://github.com/XinSpark/dsh-next-multi-launcher/releases/tag/v1.0.0
