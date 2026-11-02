# 架构与数据流

## 模块划分

```
src-tauri/src/
├── lib.rs          # 组装：窗口创建、状态构建、命令注册
├── commands.rs     # IPC 层：只做“读状态 / 改状态 / 调管道”
├── launcher.rs     # 启动队列（单 worker）、进度事件、停止逻辑
├── scan.rs         # 进程扫描：识别 DSH 进程、反推 home、判断安全模式
├── pointer.rs      # 指针读写、备份、回滚
├── store.rs        # app.json / 实例文件 / 校验规则
├── versions.rs     # 版本树扫描、junction 登记、asar 版本读取
├── discover.rs     # 自动发现安装目录（快捷方式/注册表/常见路径/进程）
├── catalog.rs      # 版本目录：三渠道清单；安装包优先挑选、latest、内核版本、加速前缀
├── update.rs       # 启动器自身更新检查（只提示，不下载；与目录共用加速前缀）
├── asar.rs         # 最小 asar 读取（package.json）
├── guard.rs        # 删除安全网（盘根/系统目录/默认环境）
├── perm.rs         # 权限探测与提权重启
├── errors.rs       # 错误码与中文文案
├── model.rs        # 序列化结构（Config/Instance/RunningInfo/LaunchStatus…）
├── util.rs         # 路径、原子写、数据目录兜底
└── cli.rs          # --selftest / --e2e-status / --e2e-launch / --scan-debug
```

前端（`src/`）：`App.svelte` 持有快照与轮询，组件只做展示；`lib/api.ts` 统一把 Rust 错误转成可展示结构。

共用的 UI 零件集中在 `src/lib/`，**一处实现、多处引用**（同一个动作不在三个地方各写一遍）：

| 文件 | 职责 |
|---|---|
| `Icon.svelte` + `icons.ts` | 原生 16×16 图标，path 逐字取自官方 `primitives`，不自己画 |
| `Switch.svelte` | 原生开关（36×20，状态键在 `aria-checked`） |
| `SelectMenu.svelte` | 原生风格下拉选择，替代无法上样式的原生 `<select>` |
| `DirPickButton.svelte` | 「选择目录」按钮，设置 / 向导 / 版本导入三处共用 |

样式由两道门守着：`tools/verify-tokens.mjs`（用到的 token 名字必须存在）与
`tools/lint-css.mjs`（禁裸颜色、圆角只用 token、字号必须与行高成对、字重不出界）。

## 初始化向导

`OnboardingPanel.svelte` 是盖在主界面上的单页面板，状态来自 `onboarding_state`
（复用 `permission_report`，不新增探测）。四个 IPC：`onboarding_state` /
`onboarding_complete` / `onboarding_skip` / `onboarding_reset`。
`Config.onboarding` 三字段全空即「从未引导」，所以老 `app.json` 无需迁移。
详见 [design-onboarding.md](../design-onboarding.md)。

## 在线目录与加速前缀

`catalog_list` / `update_check` 都是 **async 命令 + `spawn_blocking`**：
同步命令跑在主线程上，GitHub 请求最长 10 秒会把窗口一起卡住。
配置了 `downloadMirror` 时，API 请求、安装包地址、更新检查共用同一个前缀
（实测 `gh-proxy.com` 可代理 `api.github.com`，`ghproxy.net` 只代理下载）。

## 启动管道（关键时序）

```
点击启动
  → 入队（重复入队报 E1012）
  → 单 worker 取队首
  → 校验：版本 exe/asar（E1004）→ home 是否已被占用（E1003）
  → 写指针 = 目标 home（失败 E1001，备份原内容）
  → spawn DSH NEXT.exe（stdio 全部 null）
  → 轮询（pollIntervalMs）：
       先看成功：该 home 出现进程（含 --user-data-dir=<home>\electron-user-data）
       再看取消
       再看子进程退出：父进程退出后给 20s 宽限（DSH 会自我重启一次）
       最后看超时（launchTimeoutSec → E1005）
  → 成功：记录 lastLaunchAt / safeMode；把指针还原为 defaultHome
  → 失败：回滚指针（E1002 表示回滚也失败）
```

**为什么按"进程出现"判定而不是按 pid**：DSH-NEXT 启动后会自我重启，父进程 2–12 秒内退出，盯着 pid 会误判失败。

## 指针生命周期

| 时机 | 动作 |
|---|---|
| 启动实例前 | 写为目标 home |
| 启动成功 | 还原为 `defaultHome` |
| 启动失败 | 回滚为启动前内容 |
| 手动「恢复默认」 | 写为 `defaultHome` |

**已知限制**：指针是全局唯一的。DSH-NEXT 的"重启"会重新读它，所以在 DSH 内部重启非默认实例会落到默认环境；重启请用启动器的「重启 ▾」。

## 并发模型

- 启动是**串行队列**：同一时刻只有一个 spawn 在进行，避免指针互相覆盖；
- 停止是同步命令（优雅 15s → 需要确认 → 强制整组结束）；
- `snapshot` 是读取型命令，前端 2 秒轮询一次；版本扫描有 5 秒缓存。

## 前端状态

- `App.svelte`：`snapshot`（2s 轮询）+ `launch:progress` 事件；
- 对话框用 `dialog` 联合类型；实例行与详情面板共享同一份 `InstanceView`；
- 所有 IPC 错误经 `lib/api.ts` 归一为 `{ code, message, detail }`。
