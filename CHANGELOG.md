# 更新日志

## Codex 用量监控器 v0.1.0 — Release Candidate

### 新增

- Windows 桌面总览，用于查看 Codex Desktop 的官方额度观测和本地活动。
- 被动 Token 活动历史，以及项目和模型汇总。
- 带有明确可信度标记的消耗速率和额度耗尽预测。
- 支持关闭到系统托盘、Windows 开机启动、系统通知、用量阈值和预测时间设置。
- 系统托盘操作、本地告警和有界 SQLite 历史记录。
- Windows x86_64 当前用户 NSIS 安装包配置。

### 隐私与范围

- 监控为只读操作，不执行登录或模型请求。
- 不持久化或记录 Access Token、Refresh Token、Cookie、完整邮箱地址、提示词、助手消息和会话预览。
- 会话覆盖来自本地通知和 rollout 记录；不会使用 `thread/resume` 收集历史。

### 迁移说明

- 本版本将正常运行架构迁移为 Desktop Direct，直接读取 Codex Desktop 本地数据。
- 迁移前版本中的 App Server、账户读取和 CLI 运行控制仅作为历史架构保留，不属于当前 Desktop Direct 运行路径。

### 发布说明

- 在没有真实 Windows 签名证书时，本版本为未签名候选版本。
- 不包含更新器、GitHub Release 上传或自动安装步骤。
