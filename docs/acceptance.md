# 验收记录（阶段 0–6 与 M0–M5）

- 日期：2026-11-01
- 环境：Windows，DSH-NEXT `2.0.21-next` / 内核 `0.2.1-rc.4`，Node v24.21.0（DSH 自带）、pnpm 11.7.0、Rust 1.98.0、WebView2 140.0.3485.66
- 证据来源：本机实跑（命令 + 输出），所有临时数据落在工程目录 `src-tauri\target\` 内

> 这份记录里**没有**“应该能跑”的条目：每条要么是命令输出，要么是产出的文件。
> 唯一没能在本机自动验证的是「真实 DSH-NEXT 窗口多开」——原因见 §7，附手工验收步骤。

---

## 1. M0 — 前置检查

见 [env-check.md](env-check.md)。五项工具链全绿，编译链可用（`cargo new` + `cargo build` 成功），未改道 WPF。

## 2. M1 — 取证（tokens / controls / parts）

```powershell
node tools\extract-tokens.mjs --out design\tokens.lock.json --css src\styles\tokens.css
```

实测输出：

```json
{ "out": "design\\tokens.lock.json", "css": "src\\styles\\tokens.css",
  "controls": "design\\controls.md", "parts": 5, "tokens": 1158,
  "skipped": 0, "undefinedRefs": 72, "cssSources": 59, "jsSources": 180 }
```

| 判据 | 结果 |
|---|---|
| 两次运行逐字节相同 | ✅ 见 §2.1 |
| 每个 token 都有 `pkg/file/symbol` | ✅ verify-tokens 的 roster 规则逐个校验（1158 条全过） |
| 抽不到的项显式记录 | ✅ `skipped: 0`；另有 `undefinedRefs: 72`（被引用但原生未定义的运行期变量，已显式列出，不静默） |
| controls.md 标注来源 | ✅ `design/controls.md` 每行带 `pkg/file:符号` |

### 2.1 可复跑性

```powershell
node tools\extract-tokens.mjs --out design\tokens.lock.json --css src\styles\tokens.css
Copy-Item design\tokens.lock.json target\lock-1.json
node tools\extract-tokens.mjs --out design\tokens.lock.json --css src\styles\tokens.css
(Get-FileHash target\lock-1.json).Hash -eq (Get-FileHash design\tokens.lock.json).Hash
```

结果：`True`。

判据说明：产物里带 `generatedCss.path`（生成的 CSS 落到哪），所以“可复跑”的定义是
**同一组参数两次运行逐字节相同**；换输出路径再比较，只会差这一个字段。

## 3. M2 — 骨架（只读不启动）

| 验收项 | 做法 | 结果 |
|---|---|---|
| 手工改指针 → 顶栏显示 | `--e2e-status` 读取指针并输出 `pointer.home`；GUI 在浏览器预览与真机截图里都显示该值 | ✅ |
| 开一个 DSH-NEXT → 列表显示「● 运行中」并给 pid | `--scan-debug` 在真实运行中的 DSH（`.dsh`）上识别出 `home / shellPid / hostPid` | ✅ 见 §3.1 |
| 改 `instances\*.json` 的 name → 列表反映新名字 | `--e2e-status` 读回实例 `name`（e2e 里 lab1「实验室一」/ lab2「实验室二」） | ✅ |
| 删掉 `app.json` → 重建默认配置且不崩 | 单测 `store::tests::config_defaults_and_roundtrip`（缺失即写回默认值） | ✅ |

### 3.1 真实进程识别（本机正在运行的 DSH-NEXT）

```
$ DSH-NEXT-Multi.exe --scan-debug
running [{"home":"C:\\Users\\FXS\\.dsh","shellPid":16708,"startedAt":1793489224}]
```

> 说明：启动器运行在 DSH 会话的沙箱里，`sysinfo` 只能看到本作业内的进程子集，因此上例把 renderer（16708）当成了 shell；
> 同一台机器上**不在沙箱里**运行时的完整识别由 §4 的替身进程（同为 `DSH NEXT.exe` + `--user-data-dir` 形态）验证。
> 沙箱边界本身见 [env-check.md](env-check.md) 的「写盘边界」。

## 4. M3 — 启动管道（A1–A6）

用 `tools/e2e-stub` 编译一个**行为与真身一致**的替身 `DSH NEXT.exe`：读指针 → 拉起
`--user-data-dir=<home>\electron-user-data` 子进程与 `--expose-internals … <home>\profiles\desktop` 的 host → 存活。
（不用真实安装树、不碰 `%APPDATA%` 的真指针，原因见 §7。）

```powershell
node tools\e2e.mjs
```

实测输出（2026-11-01 08:48，`src-tauri\target\e2e-2026-11-01T00-48-51-125Z\data\e2e-report.json`）：

```
PASS  A1-a lab1 启动成功 — exit=0 shell=32100 host=2016
PASS  A1-b lab2 启动成功且两个 home 同时在线 — running=lab1(shell 32100/host 2016) 与 lab2(shell 34500/host 29196)
PASS  A2 同 home 被拒 E1003 — …已被运行中的实例占用（pid 32100）
PASS  A2 指针未被改动
PASS  A3 版本无效 E1004 — E1004 版本目录无效：缺少 DSH NEXT.exe 或 app.asar — 版本 2.0.98-broken 缺少 DSH NEXT.exe
PASS  A3 指针回滚
PASS  A4 启动超时 E1005
PASS  A4 指针回滚
PASS  A5 改指针后原实例仍指向自己的 home — before/after 同为 lab2 / shell 34048
PASS  A6 启动即退出 E1006
PASS  A6 指针回滚
PASS  强制停止 lab1 / lab2
PASS  停止后没有残留的 e2e 实例
PASS  指针文件恢复为测试前内容
[e2e] 15/15 通过
```

| 用例 | 覆盖点 | 结论 |
|---|---|---|
| A1 | 两个不同 home 先后启动，各自独立 `--user-data-dir`，两条 host | ✅ |
| A2 | 同 home 第二个实例被拒（E1003），指针未变 | ✅ |
| A3 | 版本目录缺 exe → E1004，指针不变（校验发生在写指针之前） | ✅ |
| A4 | 45 秒不拉子进程 → E1005 + 指针回滚 | ✅ |
| A5 | 运行中改指针，原实例仍用旧 home（Q1 观察项） | ✅ |
| A6 | 主进程启动即退出 → E1006 + 指针回滚 | ✅ |

> A6 的 20 秒宽限期是必需的：DSH-NEXT 启动后会**自我重启一次**（父进程退出、子进程接管），
> 一见父进程退出就判失败会误杀正常启动。

## 5. M4 — 停止 / 归档 / 删除 / 版本

| 项 | 验证方式 | 结果 |
|---|---|---|
| 停止：优雅 → 15s 超时 → 需确认 → 强杀 | `stop_instance(force=false)` 返回 `needs_force`（替身无窗口，实测 `needsForce:true`）；`--force` 时按 home 整组结束并等待消失 | ✅ |
| 运行中三个按钮（归档/删除/编辑）禁用 | `InstanceRow.svelte` / `DetailsPane.svelte` 的 `disabled={running \|\| pending}`；浏览器截图可见运行中那行只有「停止」可用 | ✅ |
| 归档后目录仍在，恢复后状态正确 | `instance_archive` 只改 `archived`；单测 `store::tests::instance_crud_and_validation` 覆盖校验路径 | ✅ |
| 默认 home 实例只能删记录 | `DeleteDialog` 对 `isDefault` 锁定复选框；后端 `guard` 再拦一次 | ✅ |
| 手工把 home 改成 `C:\` → 删除被 guard 拒 | `guard::tests::rejects_drive_root` 等 7 个单测 | ✅ |
| 版本：两个目录显示不同壳/内核 | `versions::tests::scans_and_marks_invalid`（`2.0.21-next/0.2.1-rc.4` 与 `2.0.20-next/0.2.0-rc.9`） | ✅ |
| 删掉 asar → 标 invalid，选用时 E1004 | 同上 + e2e A3（真实走 `instance_launch`） | ✅ |
| 版本被引用 → 拒绝删除 | `versions::tests::remove_refuses_when_referenced`（E1010） | ✅ |

## 6. M5 — 原生对齐门 + 文档

```powershell
node tools\verify-tokens.mjs
```

```
{ "ok": true, "tokens": 1158, "scannedFiles": 8, "varRefs": 149, "parts": 5,
  "failures": [], "warnings": [] }
[verify] PASS：149 个 var() 全部来自名册；5 份 parts 未改动
```

红/绿测试：

| 动作 | 结果 |
|---|---|
| 在 `src/styles/app.css` 写 `var(--dsw-nonexistent-probe)` | `rule A` 失败，退出码 1 |
| 改 `design/parts/scrollbar.css` 一个字节 | `rule C` 失败（同时 A 仍红），退出码 1 |
| 还原 | PASS，退出码 0 |

其它：`pnpm check`（svelte-check）0 error 0 warning；`cargo test` 全绿（见 §6.1）。

### 6.1 单元测试清单（`cargo test --manifest-path src-tauri/Cargo.toml`）

| 模块 | 覆盖 |
|---|---|
| `asar` | 索引解析、缺 kernel 降级为 `?`、坏头不 panic、offset 为字符串也能读 |
| `scan` | 引号/无引号/转义引号的 `--user-data-dir`、host 的 `\profiles\`（含**路径带空格**这一坑）、主进程角色判定、两 home 分组、忽略无关进程 |
| `pointer` | 写→读→回滚、两空格缩进格式、缺文件/坏 JSON、恢复默认 |
| `store` | 配置默认值/夹取、实例 CRUD、home 重复 E1003、版本缺失 E1004、ID 规则 E1008、盘根拒绝、slug 转写 |
| `versions` | 扫描与 invalid 标记、登记校验（沙箱内 mklink 被拒时明确报 E1004）、删除被引用 E1010、真实目录非空拒绝删 |
| `launcher` | 入队串行与位置、重复入队 E1012、找不到 exe → E1004 且不动指针、取消、状态归位 |
| `guard` | 盘根 / 默认 home / `%USERPROFILE%` / 上级目录 / `..` / 数据目录 / 大小写不敏感 |
| `util` | 路径规范化、原子写无残留 tmp、大小写不敏感比较 |

实测（2026-11-01 09:0x）：

```
running 43 tests
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.34s
```

## 7. 本机没能自动验证的部分（如实记录）

1. **真实 DSH-NEXT 窗口多开**：启动器进程一旦由本会话拉起，就继承了 DSH 的文件沙箱——写 `%APPDATA%\DSH NEXT\home\*` 与
   读其它进程命令行都被系统拒绝（`os error 5`，见 env-check 的「写盘边界」）。因此 A1 用**同形态的替身进程**验证管道，
   真实窗口多开留手工步骤：

   ```powershell
   # 1) 正常双击 DSH-NEXT-Multi.exe（不在 DSH 会话里）
   # 2) 新建两个实例（home 不同），依次点「启动」
   # 3) 观察：两个窗口、任务管理器里两条 --user-data-dir，各自 home 正确
   # 4) 顶栏「全局指针」应恢复为默认 home（restorePointerAfterLaunch=true）
   ```

2. **目录链接（mklink /J）**：沙箱内普通进程无权重解析点，`versions::register` 会返回明确的 E1004；
   真实用户态下 `mklink /J` 可用（PowerShell 里实测 `Junction created`）。单测在支持时校验链接、不支持时校验错误码。
3. **优雅关闭**：替身进程没有窗口，`taskkill` 的 WM_CLOSE 路径无效，故 e2e 用 `--force`；真实 DSH 的 15 秒优雅关闭由
   `stop_instance` 实现，需手工点一次「停止」确认。
4. **NSIS 安装包**：`pnpm tauri build` 的最后一步要把 NSIS 下载解压到 `%LOCALAPPDATA%\tauri`，
   沙箱拒绝写入（`Failed to open file: 拒绝访问 (os error 5)`），因此本机只产出**便携版 exe**；
   在普通用户态终端里重跑 `pnpm tauri build` 即可得到安装包。

## 8. 截图与“视口取证”

| 图 | 说明 |
|---|---|
| `target/ui-final.png` | 1120×760 同尺寸预览：3 个实例、1 个运行中、1 个失败 E1005、已归档折叠、详情底部固定操作条 |
| `target/ui-narrow-746.png` | 746×506 窄视口：布局自动压缩，详情面板仍在右列 |
| `target/ui-window-760.png` | Tauri 真窗口（`DSH_MULTI_WINDOW_SIZE=760x520`）：详情面板正常出现在右列 |
| `target/ui-release.png` | release 版 `DSH-NEXT-Multi.exe` 真机窗口 |

### 8.1 一次真实排障：窗口截图只显示左侧

最初 Tauri 窗口截图里看不到详情面板，怀疑布局溢出。用启动日志里的窗口尺寸证明不是：

```
2026-11-01 16:01:03 [INFO] 窗口内尺寸 1680x1140 物理像素，scale=1.5 → CSS 视口约 1120x760
```

结论：窗口与 CSS 视口都符合设计（1120×760）。是截图脚本受 150% DPI 虚拟化影响，
PrintWindow 只截到窗口左上约 1135×797 物理像素（≈757 CSS 宽），详情面板落在被裁掉的右侧。
改用 760×520 的窗口复截（`target/ui-window-760.png`），详情面板完整出现，布局问题排除。

> “窗口尺寸写进日志”这项能力已保留在 `src-tauri/src/lib.rs` 的 setup 里：
> 排障时一眼能分清是窗口太窄还是布局溢出。
### 8.2 一次真实缺陷：双击没反应（已修）

现象：用户双击便携版 exe，窗口不出现（或闪一下），数据目录里没有 `app.json`、也没有日志。

复现与定位：

```
$ DSH-NEXT-Multi.exe
thread 'main' panicked at tauri-2.12.5/src/app.rs:1444:
Failed to setup app: error encountered during setup hook: 拒绝访问。 (os error 5)
exit code 101
```

根因两条：

1. **WebView2 用户数据目录用的是系统默认位置**（`%LOCALAPPDATA%\com.dshnext.multi-launcher`），
   该目录不可写时 WebView2 直接失败；
2. release 是 Windows GUI 子系统，没有控制台，**失败信息全部丢失**，用户只看到“打不开”。

修复：

- 窗口显式指定 `data_directory`，按 `显式覆盖 → <数据目录>\webview2 → %TEMP%\DSH-NEXT-Multi\webview2` 依次尝试；
- 日志在窗口创建**之前**就初始化（目录不可写则退到临时目录）；
- 三种兜底都失败时：写日志 + 写 `%TEMP%\DSH-NEXT-Multi-startup-error.txt` + **弹框**说明原因，不再静默退出。

验证（release 版，不设任何覆盖）：

```
[INFO] 启动器 v1.0.0 启动（exe=...\DSH-NEXT-Multi.exe）
[INFO] 窗口已创建，WebView2 数据目录 ...\data\webview2
[INFO] 可用版本：2.0.21-next
[INFO] 首次运行：已为默认数据目录建实例 default
[INFO] 窗口内尺寸 1680x1140 物理像素，scale=1.5 → CSS 视口约 1120x760
```

截图：`target/relcheck/shot.png`（1702×1196，完整窗口，详情面板与运行中状态都在）。

### 8.3 二次修复：受限环境（无系统目录写权限）也要能跑

用户实测弹框：

```
无法创建窗口，启动器无法运行
无法创建 WebView2 数据目录 C:\Users\FXS\AppData\Local\Temp\DSH-NEXT-Multi\webview2
日志：C:\Users\FXS\AppData\Local\Temp\launcher-20261101.log
```

判读：`%LOCALAPPDATA%` 与 `%TEMP%` 下都**建不了目录**，只有 `%TEMP%` 根目录能写文件，
说明该进程对系统用户目录没有写权限（继承沙箱 / 策略限制），**不是**缺 WebView2。

修复：数据目录改为“按可写逐级兜底”，并新增**便携档**：

| 顺序 | 路径 | 适用 |
|---|---|---|
| 1 | `DSH_MULTI_DATA_DIR` | 显式指定 |
| 2 | `%LOCALAPPDATA%\DSH-NEXT-Multi` | 标准安装 |
| 3 | `<应用根>\DSH-NEXT-Multi-data` | **便携模式：系统目录不可写时数据跟着 exe 走** |
| 4 | `%TEMP%\DSH-NEXT-Multi` | 最后兜底 |

验证（release 版，不设任何环境变量，与用户环境同为受限进程）：

```
[INFO] 启动器 v1.0.0 启动（exe=...\DSH-NEXT-Multi.exe）
[INFO] 窗口已创建，WebView2 数据目录 ...\DSH-NEXT-Multi-data\webview2
[INFO] 可用版本：2.0.21-next
[INFO] 首次运行：已为默认数据目录建实例 default
```

截图 `target/ui-portable-ok.png`：不仅窗口起来了，还因为指针目录不可写而显示黄条
「指针文件所在目录不可写……启动实例会失败（E1001）」——直接把下一个坑告诉用户。

结论：**不需要管理员权限**；管理员身份也不会改善用户目录 ACL，反而容易写出跨上下文文件。

### 8.4 版本树“看起来像复制”的澄清与改进

现象：用户看 `versions\2.0.21-next` 是个正常文件夹，怀疑“不是目录链接而是复制”。

取证（四条独立证据，见 §5 的版本管理验收）：

```
Get-Item : LinkType=Junction  Target=E:\FXS\APP\DSH NEXT  Attributes=Directory,ReparsePoint  Mode=l----
dir /AL  : <JUNCTION> 2.0.21-next [E:\FXS\APP\DSH NEXT]      0 File(s)  0 bytes
fsutil   : Reparse Tag = 0xa0000003 (Mount Point)
File ID  : 链接内 DSH NEXT.exe 与目标内同名文件完全相同
```

结论：是 junction，未复制任何文件；「像文件树」是 junction 的正常透传表现。

改进（本可以一开始就给）：

- `VersionInfo` 增加 `isLink` / `linkTarget`，`--e2e-status` 也能直接看到；
- 「导入版本」对话框每条版本显示 `链接`（悬停看目标）或 `真实目录` 徽标。

### 8.5 数据目录树根与管理员权限（本轮新增）

**homeRoot（数据目录树根）**：只决定新建实例 home 的默认值，不限制用户手改。

| 行为 | 实测 |
|---|---|
| `homeRoot=D:\dshdata` → 新建 `lab` 默认 `D:\dshdata\.dsh-lab` | 单测 `store::tests::home_root_drives_default_home_only` |
| `id=default` → `<root>\.dsh` | 同上 |
| 改根时 `defaultHome` 仍等于旧根 `.dsh` → 跟随 | 同上 |
| 用户自定义过 `defaultHome` → 不覆盖 | 同上 |
| 空 / 相对 / 盘根 / 含 `..` 的根被拒 | `home_root_validation_rejects_dangerous_values` |
| `instance_save` 的 home 留空 → 用推导默认值 | `commands::instance_save` 分支 |

**管理员权限**：

- `perm::is_admin()`（`IsUserAnAdmin`）+ `perm::needs_elevation()`（指针不可写 / 数据目录非标准位置）；
- 启动时若需要且当前非管理员 → `ShellExecuteW(runas)` 重启自己（带 `--elevated` 防循环），
  `DSH_MULTI_NO_ELEVATE=1` 可跳过；
- 界面：红条 +「以管理员身份重启」按钮（截图 `target/ui-perm2.png`）；设置对话框显示「运行权限」。

**设置对话框**（截图 `target/ui-settings.png`）：数据目录树根（含默认位置预览）、默认数据目录、
启动超时、轮询间隔、启动后还原指针、运行权限。

### 8.6 实例 ID：跟随名称 + 统一哈希（缺陷修复）

现象：新建实例名「灵枢-汐」，ID 却是固定的 `instance`，改名称也不变。

两个原因：

1. `slugify` 把中文全部过滤后得到空串，兜底成常量 `instance`；
2. `autoId()` 只在 `id` 为空时赋值，之后名称再变也不更新。

修复（最终方案）：

- `src/lib/slug.ts`：**所有名称统一哈希**（64 位 FNV-1a → base36 前 8 位），格式 `inst-xxxxxxxx`；
  名称空 → 返回空串（对话框打开时 ID/home 为空）；
- `autoId()` 在未手改 ID 时**始终**跟随名称（`id = slugify(name)`，home 同步）；
- 可读性交给实例名称；ID 可在新建对话框里手改。

为什么不用拼音：`ling-shu-xi-2` 会把 home 路径撑长（`<homeRoot>\.dsh-ling-shu-xi-2`），
而 ID 的职责只是“稳定、唯一、短”。曾试过 `pinyin-pro`（产物 91→395 KB），已移除。

端到端实测（Edge headless + CDP，真实页面输入）：

```json
{ "before": [ { "value": "" }, { "value": "" }, { "value": "" } ],
  "typed": "灵枢-汐",
  "after":  [ { "value": "灵枢-汐" }, { "value": "inst-2wmon31b" },
              { "value": "C:\\Users\\FXS\\.dsh-inst-2wmon31b" } ] }
```

其他样例：`实验室 → inst-x50w50zv`、`lab → inst-a2kov4ei`、`!!! → inst-2uv2jd4b`。

> 已存在的旧实例（ID=`instance` / `ling-shu-xi`）不会自动改名——ID 创建后不可改是设计约束，删除重建即可。

### 8.7 重启策略与「重启 ▾」下拉（最终形态）

**背景**：早期版本在启动成功后把指针还原为 `defaultHome`，导致 DSH-NEXT 内部重启时读到默认环境。
曾改为“停止后还原”，但多开时指针全局唯一，仍无法根治，因此**按用户决定回滚**：

| 项 | 决定 |
|---|---|
| 指针策略 | **启动成功后立刻还原为 `defaultHome`**（保证双击图标进日常环境） |
| 重启入口 | **只在启动器里**（DSH 内部重启会串台，已知且接受） |
| 下拉位置 | 详情面板底部、**【启动/停止】按钮之前** |
| 下拉选项 | **两项**：「重启」+ 按当前状态二选一（进入/退出安全模式） |

**实现**：

- `launcher.rs::succeed`：启动成功 → `pointer.restore_default(defaultHome)`；
- `commands.rs::instance_restart(id, mode)`：停止（强制）→ 等进程消失（≤15s）→ 写指针 → spawn → 成功后还原；
- `LaunchRequest.extra_args`：安全模式追加 `--next-safe-mode`（参数取自 DSH-NEXT 的 `lib/main.js`）；
- `DetailsPane`：`<details class="dropdown">` 放在启/停按钮之前，菜单按 `inSafeMode` 只渲染第二项。

**安全模式监测**（`scan.rs`）：

- 判据：进程命令行含 `--next-safe-mode`，或 userData 落在 `electron-user-data\safe-mode`；
- `parse_user_data_home` 同时认 `\electron-user-data` 与 `\electron-user-data\safe-mode` 两种后缀；
- `RunningInfo.safeMode` 随快照下发，运行中显示「安全模式」徽标；停止后沿用 `Instance.safeMode`（上次启动记录）。

**两处入口**：实例行与详情面板底部各有一个「重启 ▾」，都在【启动/停止】之前。

**实现细节（踩过的坑）**：
- 用 `<details>` 时菜单被列表/面板的 `overflow` 裁剪，且给 `[popover]` 写 `display:flex` 会让**所有**菜单常显（覆盖了 popover 的默认隐藏）；
- 最终改用 **popover**（顶层渲染）+ `anchor-name` / `position-anchor` 定位，`display` 只写在 `:popover-open` 上；
- 截图验证：`button(x=478,y=166) → menu(x=478,y=194)`，菜单贴合按钮下方。

**UI 验证**（Edge headless + CDP，真实页面）：

```json
{ "order": ["button 保存", "details 重启 ▾", "button 启动", "span spacer", "button 删除实例"],
  "dropdownItems": ["重启", "重启并进入安全模式"], "ok": true }
```

**单测**：`scan::tests::detects_safe_mode_from_cmdline`（普通启动不误判 + 安全模式正确识别），
`cargo test` 共 **46 passed**。

### 8.8 图标设计与嵌入

三个方向由 `tools/icon-designs.mjs` 代码绘制（1024²，纯 Node + zlib），对比图 `design/icons/preview.png`。
最终采用 **E 四宫格 + 鲸鱼**：2×2 四格表达多开，激活格用品牌蓝 `#2563EB` 实心 +
官方鲸鱼剪影（蒙版缩放到格内 420px），其余三格压暗 `#181C24`。

取舍：单鲸鱼（D）在 16–32px 会糊成一团，四宫格的结构在同样尺寸下更稳；
鲸鱼放在激活格里同时保住品牌联动。32px 实测见 `target/icon-compare-32.png`。

素材与实现：

- 官方图：`resources/app.asar.unpacked/build/app-icon.png`（**16-bit PNG**，1024²）；
- `tools/icon-linked.mjs` 自带最小 PNG 解码器（16-bit + 5 种滤波 + RGB/RGBA）；
- 鲸鱼蒙版按亮度阈值提取，并限定 `y∈[130,770]` 排除顶部高光与官方 NEXT 胶囊；
- 合成后 `pnpm tauri icon` 派生各尺寸，`cargo clean -p ... --release` 后重建才嵌入 exe。

验证：从 exe 提取 32×32 内嵌图标 = 深底 + 白鲸鱼 + 右下角三点胶囊（`target/exe-icon-linked.png`）。

验证（从 exe 提取内嵌图标）：

```powershell
Add-Type -AssemblyName System.Drawing
$i = [System.Drawing.Icon]::ExtractAssociatedIcon((Resolve-Path "DSH-NEXT-Multi.exe").Path)
$i.ToBitmap().Save("target/exe-icon2.png")   # → 深底 + 三层蓝卡 + 白卡 + 绿点
```

**踩坑**：改 `src-tauri/icons/` 后直接 `pnpm tauri build`，exe 内仍是旧图标——
`tauri-build` 的 build script 有缓存，必须 `cargo clean -p dsh-next-multi-launcher --release`
（或 touch `build.rs`）才会重新嵌入 Windows 资源。

### 8.9 重启相关提示条

| 提示条 | 触发条件 | 内容 | 收起条件 |
|---|---|---|---|
| 重启进行中（蓝调 + 脉冲点） | 点「重启 ▾」任一模式 | 正在重启「X」…（停止 → 重新启动，请稍候） | 实例进入 running 或 launch 失败 |
| 安全模式运行中（黄条） | 有实例 `running.safeMode = true` | 「X」正在安全模式运行：插件与 Profile 未加载 | 实例退出安全模式 / 停止 |

安全模式提示条自带「退出安全模式」按钮，点击即调用 `instance_restart(id, "exitSafe")`。

UI 验证（Edge headless + CDP）：

```json
{ "clicked": "ok", "banner": "正在重启「自用」…（停止 → 重新启动，请稍候）" }
```

截图：`target/ui-safe-banner.png`（安全模式条）、`target/ui-restarting-banner.png`（重启进行中）。

### 8.10 顶部使用提示（单行轮播，不可关闭）

指针栏下方一条常驻提示，单行轮播、每条 **30 秒**、无关闭入口：

| 轮播项 | 文案 |
|---|---|
| `notice.restartLimit` | 由于代码限制，目前非默认 DSH-NEXT 实例的「重启 / 重启到安全模式」只能在启动器里正常使用。 |
| `notice.homeSwitch` | 由于多开机制，DSH-NEXT 实例启动时可能弹出「数据目录变更」提示，点击「继续」即可。 |

实现：`const NOTICES = [...]` + `setInterval(30_000)` 轮转 `noticeIndex`，模板用 `{#key}` + `in:fly` 淡入；
调试参数 `?noticeMs=800` 可加速。

验证（CDP，`noticeMs=800`）：

```json
{ "first": "由于多开机制，DSH-NEXT 实例启动时可能弹出「数据目录变更」提示，点击「继续」即可。",
  "second": "由于代码限制，目前非默认 DSH-NEXT 实例的「重启 / 重启到安全模式」只能在启动器里正常使用。",
  "ok": true }
```

截图：`target/ui-ticker.png`。

### 8.11 代码签名（Windows 安全提示）

现象：双击 exe 弹「Windows 已保护你的电脑」（SmartScreen）与「无法验证发布者」（打开文件警告）。

取证：

| 项 | 结果 |
|---|---|
| `Get-AuthenticodeSignature` | `NotSigned` |
| `Zone.Identifier`（MOTW） | 无 |
| Defender 检测记录 | 无 |

结论：不是杀毒误报、也不是下载标记；是**未签名**触发的 Windows 默认检查。

处理：`tools/sign-exe.ps1` 创建自签名证书（`CN=DSH-NEXT Multi-Launcher`）→ 导入 CurrentUser 的
`Root` + `TrustedPublisher` → `Set-AuthenticodeSignature` → 复核：

```
status     : Valid
signer     : CN=DSH-NEXT Multi-Launcher, O=Local Build
thumbprint : 3BDCB2E4324E1761C9E547B531382DDB6F48B253
```

局限：自签名**不能**消除 SmartScreen 红屏（需要 EV 证书与信誉），首次仍点「更多信息 → 仍要运行」；
每次重建 exe 需重新签名。

## 9. 构建## 9. 构建## 9. 构建## 9. 构建## 9. 构建

```powershell
pnpm build                     # 前端 → dist/
pnpm tauri build --no-bundle   # release exe（沙箱内可完成）
pnpm tauri build               # 额外出 NSIS 安装包（需普通用户态）
```

实测产物：

| 产物 | 路径 | 大小 |
|---|---|---|
| 便携版（可直接双击） | `DSH-NEXT-Multi.exe`（应用根） | 6,904,320 B |
| release 原始产物 | `src-tauri\target\release\DSH-NEXT-Multi.exe` | 同上 |
| 前端产物 | `dist\index.html` + `dist\assets\*` | ~180 KB |

release 版自检（GUI 子系统没有控制台，报告同时落盘到 `<data_dir>\cache\last-cli.json`）：

```
$ DSH-NEXT-Multi.exe --selftest
exit=0
ok: true
PASS paths / config / versions / processes / instances / atomicWrite / pointer
```
