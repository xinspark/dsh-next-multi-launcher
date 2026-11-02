# 贡献指南

感谢你愿意参与。这个项目是 Windows 桌面应用（Tauri 2 + Svelte 5 + Rust），
面向 DSH-NEXT 的多实例管理。

## 环境要求

| 依赖 | 版本 | 说明 |
|---|---|---|
| Node.js | ≥ 20 | 本仓库用 pnpm 管理 |
| pnpm | ≥ 9 | `corepack enable` 或独立安装 |
| Rust | ≥ 1.77 | MSVC 工具链 |
| WebView2 | 任意较新版本 | Win10/11 通常自带 |

## 本地开发

```powershell
pnpm install
pnpm tauri dev          # 开发模式（Vite + WebView 热更新）
```

常用命令：

```powershell
pnpm check              # svelte-check（0 error / 0 warning 是门槛）
pnpm build              # 只构建前端到 dist/
pnpm verify             # 设计 token 门禁（四规则）
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build --no-bundle
node tools/ui-check.mjs # 端到端 UI 检查（需先 pnpm dev）
```

## 目录约定

| 目录 | 用途 | 入库 |
|---|---|---|
| `src/` | Svelte 前端 | ✅ |
| `src-tauri/` | Rust 后端与 Tauri 配置 | ✅ |
| `tools/` | 抽取、校验、验收、图标、签名脚本 | ✅ |
| `design/` | 设计 token 锁、原生 parts、图标设计稿 | ✅ |
| `docs/` | 公开文档 | ✅ |
| `docs-private/` | 私有文档（内容不入库） | 仅 README |
| `versions/`、`target/`、`node_modules/` | 本机运行时 | ❌ |

## 代码风格

- **Rust**：`cargo fmt`；错误码集中在 `src-tauri/src/errors.rs`，不要在业务代码里拼字符串
- **TypeScript / Svelte**：`pnpm check` 必须 0 error；组件用 Svelte 5 runes（`$state` / `$derived` / `$props`）
- **CSS**：**不允许**出现 hex / `rgb()` / `hsl()`，颜色只能来自 `var(--dsw-*)`；新增变量必须先在 `design/tokens.lock.json` 里存在（`pnpm verify` 会拦）

## 提交与 PR

- 提交信息用中文或英文均可，第一行写清"做了什么"，正文写"为什么"
- 一个 PR 只做一件事；涉及行为的改动请补测试（Rust 单测或 `tools/` 下的验收脚本）
- PR 描述里请写明：改动范围、验证方式、是否影响已发布行为
- 破坏性改动请同时更新 `CHANGELOG.md` 的 Unreleased 段

## 报告问题

用 Issue 模板（Bug / Feature）。Bug 报告请附：

1. 复现步骤；
2. 启动器日志：`%LOCALAPPDATA%\DSH-NEXT-Multi\logs\launcher-*.log`（或便携模式 `DSH-NEXT-Multi-data\logs\`）；
3. `DSH-NEXT-Multi.exe --e2e-status` 的输出（脱敏后）。

## 跨平台适配

当前仅支持 Windows（见 [README](README.md#平台支持)）。如果你有 macOS / Linux 环境，欢迎适配：

1. **先取证**：在目标平台上运行 DSH-NEXT，记录指针文件位置、`home` → userData 的映射、
   进程命令行形态（主进程是否带 `--user-data-dir`、host 的子进程参数）。这是所有改动的前提，
   写进 `docs/reference/compatibility.md` 的"依赖契约"表；
2. **路径层**：`util.rs` 的 `%VAR%` 与反斜杠拼接改为 `PathBuf` + `dirs`；
3. **提权**：`perm.rs` 按平台分支（mac `geteuid`，linux `pkexec`），不要假设 `runas`；
4. **版本树**：`versions.rs` 的 junction 改为 Unix `symlink`；
5. **进程扫描**：`scan.rs` 的路径解析与子进程角色判定按新平台的命令行重写；
6. **打包**：`tauri.conf.json` 补 `targets` 与图标（`.icns` / 多种 png）；
7. **验收**：在 PR 里附该平台的 `cargo test`、`node tools/e2e.mjs`、真实窗口多开步骤与截图。

PR 请标注适配的平台与版本（如 `macOS 15 arm64`）。没有真实环境证据的适配 PR 会被要求补证据。

## 许可证

提交即表示你同意以 [MIT](./LICENSE) 许可证授权你的贡献。
