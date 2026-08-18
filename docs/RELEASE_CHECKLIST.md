# v0.1.0 发布检查清单

## Desktop Direct 迁移

- [x] 监控架构使用 Codex Desktop 本地状态和 rollout 观测。
- [x] 独立 CLI 检测、运行时控制、账户读取和协议检查已移出正常生产初始化流程及 Tauri handler。
- [x] 下方历史 App Server 实机检查已不适用于当前架构，不得作为 Desktop Direct 验收证据。
- [ ] 新鲜的 Desktop 本地数据实机验收已完成。

## 范围与元数据

- [x] `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 版本均为 `0.1.0`。
- [x] 产品显示名称为 `Codex 用量监控器`。
- [x] identifier 保持为 `com.codexusagemonitor.app`。
- [x] Cargo description 描述 Windows Codex 用量监控器。
- [x] DEV-001 至 DEV-025 路线范围已完成；本次汉化和收尾不新增 DEV 编号。

## 构建与质量

- [x] `npm run build`。
- [x] `cargo fmt --all -- --check`。
- [x] `cargo check`。
- [x] `cargo test`（130 项通过）。
- [x] `cargo clippy -- -D warnings`。
- [x] 汉化后的新鲜 `npm run tauri build`。

## Windows 安装包

- [x] 仅使用 NSIS 目标。
- [x] NSIS 安装模式为 `currentUser`。
- [x] 安装器语言为 `SimpChinese`，且不显示语言选择器。
- [x] 所需源图标 `32x32.png`、`128x128.png`、`128x128@2x.png` 和 `icon.ico` 均存在。
- [x] 新鲜的 `Codex 用量监控器_0.1.0_x64-setup.exe` 位于 `src-tauri/target/release/bundle/nsis/` 且大小非零。
- [x] 已记录新鲜发布可执行文件和安装包的 SHA256。
- [ ] 记录签名状态。

迁移前的 NSIS 候选包仅作为历史文件，不作为 Desktop Direct 验收证据。

## 历史应用检查

以下项目属于旧版运行架构的历史检查；Desktop Direct 不再通过 App Server 启停来完成这些检查：

- [ ] 设置在手动启动和重启后保持不变。
- [ ] `关闭到系统托盘=true` 时隐藏窗口并保持监控。
- [ ] `关闭到系统托盘=false` 时退出并完成清理。
- [ ] Windows 开机启动启用/停用状态可读回，并在测试后恢复。
- [ ] 历史运行时启动/停止检查不适用于 Desktop Direct。

上一版 v0.1 候选版本的验收记录保留为历史，不验证本次 Desktop Direct 迁移。

## 数据安全

- [x] 保留已有监控数据；Schema v4 仅新增 Desktop Direct 表，不删除旧行。
- [x] 损坏的 `app_settings_v1` 会回退到默认设置并记录警告。
- [x] 不包含模型请求、登录、更新器、云同步或凭据持久化。
- [x] Access Token、Refresh Token、Cookie、完整邮箱、提示词、助手消息和会话预览均不写入数据库或日志。
- [x] 安装器、`target`、`dist`、数据库、日志、临时文件和生成的 schema 均已忽略或不跟踪。
- [x] 外部 Desktop SQLite 连接使用只读、禁止创建、单连接和 query-only 模式。
- [x] rollout 解析排除对话内容，并将单行限制为 4 MiB。

## 分发

- [x] 不自动安装已有软件。
- [x] 不创建 Git tag。
- [x] 不创建或上传 GitHub Release。
- [x] 不提交安装器二进制文件。

## 最终验收

- Windows 实机冒烟测试：未执行 — 当前验收环境无法提供稳定的 UI 几何控制。
- 三轮重启测试：未执行。
- 未执行 Codex 模型请求：是。
- 原始 Windows 开机启动状态已恢复：未测试；本次未修改开机启动状态。
- 最终安装器文件名：`Codex 用量监控器_0.1.0_x64-setup.exe`。
- 最终安装器大小：3,448,647 bytes。
- 最终安装器 SHA256：`A5A3CE293D63ED5900CAF2361B1E3783E7A6A5B9A3B1882A1333815BB5B53D17`。
- 最终发布可执行文件：`codex-usage-monitor.exe`，14,373,376 bytes，SHA256 `91959CFC1DD7790E9A4301B0416E46E8D3D19F516F3ABB4F3F4760E97D719BCD`。
- 签名：待本次构建后记录。
- 手动安装器安装冒烟测试：未执行。
- 验收结论：`Codex 用量监控器 v0.1.0 Release Candidate 仍需完成实机 UI 验收后才能正式发布。`

## Desktop Direct 验收记录

- Desktop Direct 迁移：开发测试通过；Desktop 实机验收：未执行。
- 状态 DB 只读、动态 schema 回退、rollout 游标、部分行和截断行为：已由自动化测试覆盖。
- 迁移测试未执行 Codex 模型请求、提示词读取、凭据读取、后端请求或额外运行时。
- Release Candidate Ready：`NO` — 新鲜 NSIS 构建已完成，但 Desktop Direct 架构仍需最终实机验收。
