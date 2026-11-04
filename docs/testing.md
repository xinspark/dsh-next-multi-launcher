# 测试与验收

> 本文件记录**可复跑**的验证：每条要么是命令输出，要么是产出的文件。
> 排障经过与缺陷记录已分别迁到 [experience/](experience/README.md) 与 [feedback/](feedback/README.md)。

- 最后更新：2026-11-02
- 参考环境：Windows，DSH-NEXT `2.0.21-next` / 内核 `0.2.1-rc.4`，Rust 1.7x，WebView2 140+
- 临时数据默认落在 `src-tauri\target\` 内

## 1. 自动化测试

### 1.1 单元测试（78 项）

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

| 模块 | 覆盖 |
|---|---|
| `asar` | 索引解析、缺 kernel 降级为 `?`、坏头不 panic、`offset` 为字符串也能读、unpacked 条目 |
| `scan` | 引号/无引号/转义引号的 `--user-data-dir`、host 的 `\profiles\`（含**路径带空格**）、主进程角色判定、两 home 分组、安全模式识别 |
| `pointer` | 写→读→回滚、两空格缩进格式、缺文件/坏 JSON、恢复默认、**目录里不再留 `.launcher-bak`**、`last-home` 与指针同目录/写→回滚/恢复默认 |
| `store` | 配置默认值/夹取、实例 CRUD、home 重复 E1003、版本缺失 E1004、ID 规则 E1008、盘根拒绝、`home_root` 只提供默认值 |
| `versions` | 扫描与 invalid 标记、登记校验、删除被引用 E1010、真实目录非空拒绝删、**原地升级后改名登记并改实例引用**、真实目录不改名 |
| `launcher` | 入队串行与位置、重复入队 E1012、找不到 exe → E1004 且不动指针、取消、状态归位 |
| `guard` | 盘根 / 默认 home / `%USERPROFILE%` / 上级目录 / `..` / 数据目录 / 大小写不敏感 |
| `discover` | `\\?\` 前缀剥离、UNC 还原、路径大小写不敏感比较、同目录只保留首次来源 |
| `util` | 路径规范化、原子写无残留 tmp、大小写不敏感比较 |

实测：

```
running 78 tests
test result: ok. 78 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 1.2 端到端（替身进程，15 项断言）

`tools/e2e-stub` 编译一个**行为与真身一致**的替身 `DSH NEXT.exe`：读指针 → 拉起
`--user-data-dir=<home>\electron-user-data` 子进程与 `--expose-internals … <home>\profiles\desktop` 的 host → 存活。

```powershell
node tools/e2e.mjs
```

```
PASS  A1-a lab1 启动成功
PASS  A1-b lab2 启动成功且两个 home 同时在线
PASS  A2 同 home 被拒 E1003
PASS  A2 指针未被改动
PASS  A3 版本无效 E1004
PASS  A3 指针回滚
PASS  A4 启动超时 E1005
PASS  A4 指针回滚
PASS  A5 改指针后原实例仍指向自己的 home
PASS  A6 启动即退出 E1006
PASS  A6 指针回滚
PASS  强制停止 lab1 / lab2
PASS  停止后没有残留的 e2e 实例
PASS  指针文件恢复为测试前内容
[e2e] 15/15 通过
```

| 用例 | 覆盖点 |
|---|---|
| A1 | 两个不同 home 先后启动，各自独立 `--user-data-dir`，两条 host |
| A2 | 同 home 第二个实例被拒（E1003），指针未变 |
| A3 | 版本目录缺 exe → E1004，指针不变（校验发生在写指针之前） |
| A4 | 超时未拉起子进程 → E1005 + 指针回滚 |
| A5 | 运行中改指针，原实例仍用旧 home |
| A6 | 主进程启动即退出 → E1006 + 指针回滚 |

> A6 的 20 秒宽限期是必需的：DSH-NEXT 启动后会**自我重启一次**（父进程退出、子进程接管），
> 一见父进程退出就判失败会误杀正常启动。

### 1.3 设计 token 门禁

```powershell
node tools/verify-tokens.mjs
```

```
{ "ok": true, "tokens": 1158, "varRefs": 275, "parts": 5, "failures": [], "warnings": [] }
[verify] PASS：275 个 var() 全部来自名册；5 份 parts 未改动
```

红/绿测试（确认门禁真的会拦）：

| 动作 | 期望 | 实测 |
|---|---|---|
| 写 `var(--dsw-nonexistent-probe)` | 规则 A 失败，退出码 1 | ✅ |
| 改 `design/parts/scrollbar.css` 一个字节 | 规则 C 失败，退出码 1 | ✅ |
| 还原 | PASS，退出码 0 | ✅ |

### 1.3.1 样式门禁（1.1 起）

`verify-tokens` 管"token 名字存不存在"，这道门管"**token 用对没用对**"：

```powershell
node tools/lint-css.mjs
```

```
[lint-css] PASS：R1/R2/R3/R3b 全 0
```

四条规则：禁裸颜色 / 圆角只用 token / **字号必须与行高成对** / 字重不出界（判据与出处见 `design.md` §7.3）。
改造前的基线是 **14 处**（13 处字号落单 + 1 处 `font-weight: 700`），逐条修完转绿。

红/绿对照：把任一 `font: var(--dsw-font-xs-13)` 改回 `font-size: 13px`（不带 `line-height`）→
必须报两条红（"该档有 token" + "没有配 line-height"）；还原 → 绿。

### 1.3.2 视觉验收（截图）

样式类改动没法用断言证明"像原生"，所以留了一步截图核对。做法是真机窗口 + CDP：

```powershell
# 1) 用隔离数据目录起真机窗口，并打开调试端口
$env:DSH_MULTI_DATA_DIR='<临时目录>\data'
$env:DSH_MULTI_WINDOW_SIZE='1400x1010'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9333'
.\DSH-NEXT-Multi.exe
# 2) 连 CDP 切视图 + Page.captureScreenshot（DPR 1.4）
# 3) 与 docs/images/ui-*.png 并排比对：控件高度、圆角、字号、间距逐项看
```

判据分两类：

- **可读计算值**：圆角、字号、行高、表面色——`Runtime.evaluate` 读 `getComputedStyle`
  （本轮实测 `card 16px` / `input 12px` / `segment track 12px` / 菜单 `blur(40px) saturate(1.5)`）；
- **只能看图**：页头位置、横幅高度、行内按钮是否挤在一起。本轮两次缺陷都是看图发现的——
  横幅落进 `1fr` 行被拉成整屏高、菜单在窗口底部被裁。

`docs/images/ui-main.png` / `ui-versions.png` / `ui-settings.png` / `ui-onboarding.png`
随每次样式改动重截（浅色、1400×1010@1.4），避免文档展示的是过期界面。

### 1.4 初始化向导（1.0.3 实测）

用独立数据目录起一个干净环境，逐项点过：

```powershell
$env:DSH_MULTI_DATA_DIR='<临时目录>\data'
.\DSH-NEXT-Multi.exe
```

| 场景 | 期望 | 实测 |
|---|---|---|
| 全新数据目录首次启动 | 自动展开；五块为 ① 语言 ② 环境体检 ③ 默认数据目录树根 ④ 选择 DSH-NEXT ⑤ 实例 | ✅ |
| 点「跳过」 | 面板关闭，`app.json` 写入 `skippedAt` | ✅ |
| 跳过后重启 | 不再自动展开，直接进主界面 | ✅ |
| 设置 →「重新运行初始化向导」 | 面板重新出现，`skippedAt` 清空 | ✅ |
| 点「完成并进入」 | 面板关闭，`completedAt` 写入、`lastStep=5`，并按填写建出实例文件 | ✅ |
| 无可用版本 | ④ 给出说明 + 两个导入入口；「完成并进入」置灰并写明原因 | ✅ |
| 已有「默认」实例 | ⑤ 列出实例 + 「新建实例」；点击后关向导、写 `skippedAt`、打开新建对话框 | ✅ |
| 树根留空保存 | 视为跟随 `%USERPROFILE%`，不报 E1012 | ✅ |
| 树根为 `E:\…`（非系统盘） | 「推荐改到非系统盘」提示自动消失，「恢复默认」出现 | ✅ |

对应的状态机单测 4 项：全新配置自动展开、跳过/完成后不再展开、`lastStep` 越界夹取、老配置无 `onboarding` 字段仍可读。

### 1.5 自动扫描（真实 IPC）

扫描走「界面 → IPC → PowerShell（快捷方式/注册表）→ Rust 校验」，三段都要证据：

```powershell
# 1) 命令行直接看候选
DSH-NEXT-Multi.exe --discover

# 2) 真实 GUI 端点：用 WebView2 远程调试驱动点击（开发期脚本，不随仓库发布）
#    env: WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333
```

实测（2026-11-02，本机）：

```
rows: [{"tag":"快捷方式","mono":"C:\\Program Files\\DSH NEXT","checked":true},
       {"tag":"快捷方式","mono":"C:\\Program Files\\DSH Desktop","checked":false}]
hint: 找到 2 个候选
```

- 有效目录默认勾选，无效目录灰掉不可选；
- 已登记的版本会显示「已登记」徽标且不可重复勾选；
- 结果按「有效 → 未登记 → 路径」排序。


## 2. 验收矩阵（M0–M5）

| 里程碑 | 验收项 | 验证方式 | 结果 |
|---|---|---|---|
| M0 环境 | 五项工具链、编译链 | 手工命令 | ✅ |
| M1 取证 | token 抽取 1158 条、两次运行逐字节相同 | `extract-tokens` + 哈希比对 | ✅ |
| M2 骨架 | 指针对照、真实进程识别、实例名回读、配置自愈 | `--e2e-status` / `--scan-debug` / 单测 | ✅ |
| M3 管道 | A1–A6 | `tools/e2e.mjs` | ✅ 15/15 |
| M4 操作 | 停止 / 归档 / 删除 / 版本 | 单测 + 界面禁用状态 | ✅ |
| M5 对齐 | token 门禁、`pnpm check`、`cargo test` | 命令 | ✅ |

M2 的一处说明：在受限会话里 `sysinfo` 只能看到本作业内的进程子集，
完整识别由 §1.2 的替身进程（同形态）覆盖。

## 3. 本机未能自动验证的部分（手工步骤）

1. **真实 DSH-NEXT 窗口多开**（替身已覆盖管道）：

   ```powershell
   # 1) 正常双击 DSH-NEXT-Multi.exe（不要在受限会话里）
   # 2) 新建两个实例（home 不同），依次点「启动」
   # 3) 观察：两个窗口、任务管理器里两条 --user-data-dir，各自 home 正确
   # 4) 顶栏「全局指针」应恢复为默认 home
   # 5) 关键：切换实例时**不应**出现 DSH-NEXT 的「数据目录与上次不同」确认框
   #    （出现过一次就说明 last-home 没同步；默认按钮会把这次启动改回上一个实例）
   ```

2. **原地升级后版本号跟随**：拿旧版本登记一份链接，再用安装包原地升级它，重开启动器——
   版本页应显示新版本号（`versions\<新版本>`），实例的版本字段也一并改过来。
   单测 `versions::tests::reconcile_renames_link_and_remaps_instances` 覆盖同一路径。

3. **目录链接（mklink /J）**：受限进程无权创建解析点，`versions::register` 返回 E1004；
   普通用户态下 `mklink /J` 可用。单测在支持时校验链接、不支持时校验错误码。

4. **优雅关闭**：替身没有窗口，`taskkill` 的 WM_CLOSE 无效，故 e2e 用 `--force`；
   真实 DSH 的 15 秒优雅关闭需手工点一次「停止」。

5. **NSIS 安装包**：需要把 NSIS 下载到 `%LOCALAPPDATA%\tauri`，受限环境会被拒；
   普通用户态终端重跑 `pnpm tauri build` 即可得到安装包。

## 4. 复跑全部

```powershell
pnpm check
pnpm build
node tools/verify-tokens.mjs
node tools/lint-css.mjs
node tools/check-links.mjs
cargo test --manifest-path src-tauri/Cargo.toml
node tools/e2e.mjs
```
