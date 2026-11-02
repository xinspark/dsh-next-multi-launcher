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
├── catalog.rs      # 版本目录：三渠道可下载清单（GitHub Releases / 官网直链）
├── update.rs       # 启动器自身更新检查（只提示，不下载）
├── asar.rs         # 最小 asar 读取（package.json）
├── guard.rs        # 删除安全网（盘根/系统目录/默认环境）
├── perm.rs         # 权限探测与提权重启
├── errors.rs       # 错误码与中文文案
├── model.rs        # 序列化结构（Config/Instance/RunningInfo/LaunchStatus…）
├── util.rs         # 路径、原子写、数据目录兜底
└── cli.rs          # --selftest / --e2e-status / --e2e-launch / --scan-debug
```

前端（`src/`）：`App.svelte` 持有快照与轮询，组件只做展示；`lib/api.ts` 统一把 Rust 错误转成可展示结构。

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
