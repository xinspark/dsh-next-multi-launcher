# 更新日志

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### 计划中

- 初始化向导（首次启动单页面板：环境体检 / 版本 / 首个实例）
- 每实例重定向 `%APPDATA%`（实验性，彻底隔离多开重启）

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

[Unreleased]: https://github.com/XinSpark/dsh-next-multi-launcher/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/XinSpark/dsh-next-multi-launcher/releases/tag/v1.0.0
