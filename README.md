# Codex 用量监控器 v0.1.0

Codex 用量监控器是一个 Windows 桌面监控工具，用于读取 Codex 桌面版的本地活动。本版本完成桌面直连（Desktop Direct）迁移，并在完成新的桌面版实机验收前保持为发布候选版。

## 架构

- 直接读取 `%USERPROFILE%\.codex`（或 `CODEX_HOME`）。
- 可选地以只读方式使用最新的 `state_*.sqlite` 作为索引。
- 读取 `sessions\YYYY\MM\DD\rollout-*.jsonl`，限制单行大小并持久化字节游标。
- 仅使用 rollout 中的 `session_meta`、`turn_context` 和 `token_count` 记录。
- 推导桌面版 Token 增量、项目/模型汇总、额度观测、消耗速率和计价覆盖率。
- 监控数据单独存储在 `%APPDATA%\com.codexusagemonitor.app\codex-usage-monitor.db`。

本工具不要求独立的 Codex CLI，不会启动额外的本地运行时，不会调用后端 API，不会读取凭据，也不会创建模型活动。不会持久化提示词、响应、推理文本、工具参数或 rollout JSONL 原文。

额度卡片标记为 `官方 · 桌面版观测`；Token 总量以及项目/模型报表标记为 `推导`；消耗速率和额度预测标记为 `估算`。本地观测过期后会显示为等待下一次桌面版活动，不会被当作当前数据。

## 开发

```bash
npm install
npm run tauri dev
npm run build
```

Rust 检查：

```bash
cd src-tauri
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
```

数据库架构版本为 v4，迁移文件是 `src-tauri/migrations/0004_desktop_direct.sql`。旧版 NSIS 候选包不代表最终桌面直连安装包；完成实机验收后应重新构建安装包。

## 发布状态

Codex 用量监控器 v0.1.0 当前为发布候选版，状态为未就绪。本次迁移不创建 Git 标签，也不创建 GitHub 发布。
