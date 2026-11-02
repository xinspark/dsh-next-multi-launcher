# 测试与验收

> 本文件记录**可复跑**的验证：每条要么是命令输出，要么是产出的文件。
> 排障经过与缺陷记录已分别迁到 [experience/](experience/README.md) 与 [feedback/](feedback/README.md)。

- 最后更新：2026-11-02
- 参考环境：Windows，DSH-NEXT `2.0.21-next` / 内核 `0.2.1-rc.4`，Rust 1.7x，WebView2 140+
- 临时数据默认落在 `src-tauri\target\` 内

## 1. 自动化测试

### 1.1 单元测试（70 项）

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

| 模块 | 覆盖 |
|---|---|
| `asar` | 索引解析、缺 kernel 降级为 `?`、坏头不 panic、`offset` 为字符串也能读、unpacked 条目 |
| `scan` | 引号/无引号/转义引号的 `--user-data-dir`、host 的 `\profiles\`（含**路径带空格**）、主进程角色判定、两 home 分组、安全模式识别 |
| `pointer` | 写→读→回滚、两空格缩进格式、缺文件/坏 JSON、恢复默认 |
| `store` | 配置默认值/夹取、实例 CRUD、home 重复 E1003、版本缺失 E1004、ID 规则 E1008、盘根拒绝、`home_root` 只提供默认值 |
| `versions` | 扫描与 invalid 标记、登记校验、删除被引用 E1010、真实目录非空拒绝删 |
| `launcher` | 入队串行与位置、重复入队 E1012、找不到 exe → E1004 且不动指针、取消、状态归位 |
| `guard` | 盘根 / 默认 home / `%USERPROFILE%` / 上级目录 / `..` / 数据目录 / 大小写不敏感 |
| `discover` | `\\?\` 前缀剥离、UNC 还原、路径大小写不敏感比较、同目录只保留首次来源 |
| `util` | 路径规范化、原子写无残留 tmp、大小写不敏感比较 |

实测：

```
running 70 tests
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
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
{ "ok": true, "tokens": 1158, "varRefs": 233, "parts": 5, "failures": [], "warnings": [] }
[verify] PASS：233 个 var() 全部来自名册；5 份 parts 未改动
```

红/绿测试（确认门禁真的会拦）：

| 动作 | 期望 | 实测 |
|---|---|---|
| 写 `var(--dsw-nonexistent-probe)` | 规则 A 失败，退出码 1 | ✅ |
| 改 `design/parts/scrollbar.css` 一个字节 | 规则 C 失败，退出码 1 | ✅ |
| 还原 | PASS，退出码 0 | ✅ |

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
rows: [{"tag":"快捷方式","mono":"E:\\FXS\\APP\\DSH NEXT","checked":true},
       {"tag":"快捷方式","mono":"E:\\FXS\\APP\\DSH","checked":false}]
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
   ```

2. **目录链接（mklink /J）**：受限进程无权创建解析点，`versions::register` 返回 E1004；
   普通用户态下 `mklink /J` 可用。单测在支持时校验链接、不支持时校验错误码。

3. **优雅关闭**：替身没有窗口，`taskkill` 的 WM_CLOSE 无效，故 e2e 用 `--force`；
   真实 DSH 的 15 秒优雅关闭需手工点一次「停止」。

4. **NSIS 安装包**：需要把 NSIS 下载到 `%LOCALAPPDATA%\tauri`，受限环境会被拒；
   普通用户态终端重跑 `pnpm tauri build` 即可得到安装包。

## 4. 复跑全部

```powershell
pnpm check
pnpm build
node tools/verify-tokens.mjs
node tools/check-links.mjs
cargo test --manifest-path src-tauri/Cargo.toml
node tools/e2e.mjs
```
