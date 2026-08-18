# v0.1.0 发布检查清单

## 桌面直连（Desktop Direct）迁移

- [x] 监控架构使用 Codex 桌面版本地状态和 rollout 观测。
- [x] 独立 CLI 检测、运行时控制、账户读取和协议检查已移出正常生产初始化流程及 Tauri handler。
- [x] 下方历史 App Server 实机检查已不适用于当前架构，不得作为桌面直连验收证据。
- [x] 新鲜的桌面版本地数据只读重建已完成；最终 UI 验收仍待用户确认。

## 范围与元数据

- [x] `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 版本均为 `0.1.0`。
- [x] 产品显示名称为 `Codex 用量监控器`。
- [x] identifier 保持为 `com.codexusagemonitor.app`。
- [x] Cargo 描述字段描述 Windows Codex 用量监控器。
- [x] DEV-001 至 DEV-025 路线范围已完成；本次汉化和收尾不新增 DEV 编号。

## 构建与质量

- [x] `npm run build`。
- [x] `cargo fmt --all -- --check`。
- [x] `cargo check`。
- [x] `cargo test`（144 项通过）。
- [x] `cargo clippy -- -D warnings`。
- [x] 汉化后的新鲜 `npm run tauri build`。

## Windows 安装包

- [x] 仅使用 NSIS 目标。
- [x] NSIS 安装模式为 `currentUser`。
- [x] 安装器语言为 `SimpChinese`，且不显示语言选择器。
- [x] 所需源图标 `32x32.png`、`128x128.png`、`128x128@2x.png` 和 `icon.ico` 均存在。
- [x] 新鲜的 `Codex 用量监控器_0.1.0_x64-setup.exe` 位于 `src-tauri/target/release/bundle/nsis/` 且大小非零。
- [x] 已记录新鲜发布可执行文件和安装包的 SHA256。
- [x] 签名状态：未签名（PowerShell `Get-AuthenticodeSignature` 返回 `NotSigned`）。

迁移前的 NSIS 候选包仅作为历史文件，不作为桌面直连验收证据。

## 历史应用检查

以下项目属于旧版运行架构的历史检查；桌面直连不再通过 App Server 启停来完成这些检查：

- [ ] 设置在手动启动和重启后保持不变。
- [ ] `关闭到系统托盘=true` 时隐藏窗口并保持监控。
- [ ] `关闭到系统托盘=false` 时退出并完成清理。
- [ ] Windows 开机启动启用/停用状态可读回，并在测试后恢复。
- [ ] 历史运行时启动/停止检查不适用于桌面直连。

上一版 v0.1 候选版本的验收记录保留为历史，不验证本次桌面直连迁移。

## 数据安全

- [x] 保留已有监控数据；数据库架构版本 v4 仅新增桌面直连表，不删除旧行。
- [x] 损坏的 `app_settings_v1` 会回退到默认设置并记录警告。
- [x] 不包含模型请求、登录、更新器、云同步或凭据持久化。
- [x] Access Token、Refresh Token、Cookie、完整邮箱、提示词、助手消息和会话预览均不写入数据库或日志。
- [x] 安装器、`target`、`dist`、数据库、日志、临时文件和生成的数据库架构文件均已忽略或不跟踪。
- [x] 外部桌面版 SQLite 连接使用只读、禁止创建、单连接和 query-only 模式。
- [x] rollout 解析排除对话内容，并将单行限制为 4 MiB。

## 分发

- [x] 不自动安装已有软件。
- [x] 不创建 Git 标签。
- [x] 不创建或上传 GitHub 发布。
- [x] 不提交安装器二进制文件。

## 最终验收

- Windows 实机冒烟测试：未执行 — 当前验收环境无法提供稳定的 UI 几何控制。
- 三轮重启测试：未执行。
- 未执行 Codex 模型请求：是。
- 原始 Windows 开机启动状态已恢复：未测试；本次未修改开机启动状态。
- 最终安装器文件名：`Codex 用量监控器_0.1.0_x64-setup.exe`。
- 最终安装器大小：3,490,217 bytes。
- 最终安装器 SHA256：`6B7E50BED313261C328F1B3F3CFDEB266966CAE2EFE3C91FC75322F8182368F7`。
- 最终发布可执行文件：`codex-usage-monitor.exe`，14,596,096 bytes，SHA256 `8B741A4D483B0594ADCF842CF3F14F5708D14EE5AF6E82FB74BD2FFF2FF2C406`。
- 签名状态：未签名。
- 手动安装器安装冒烟测试：未执行。
- 验收结论：Codex 用量监控器 v0.1.0 发布候选版仍需完成实机 UI 验收后才能正式发布。

## 桌面直连验收记录

- 桌面直连迁移：开发测试通过；桌面版实机验收：未执行。
- 状态 DB 只读、动态数据库架构回退、rollout 游标、部分行和截断行为：已由自动化测试覆盖。
- 迁移测试未执行 Codex 模型请求、提示词读取、凭据读取、后端请求或额外运行时。
- 发布候选版就绪：否 — 新鲜 NSIS 构建已完成，签名状态为未签名，但桌面直连架构仍需最终实机验收。

## Desktop Direct 实机验收修正记录

- 重建方式：只读使用现有 Codex Desktop rollout；未发送 Prompt、未调用模型、未读取 `auth.json`，仅清理并重建监控器自己的 Desktop 派生索引。
- 索引版本：`desktop_index_revision=2`。
- 重建快照：`393 / 393` 个启动时已发现的 rollout 游标完成；新增的本轮对话文件不纳入本次快照，避免扫描当前对话自身持续增长。
- Desktop 派生数据：已重建；Token 事件 `182,941`，Delta 事件 `182,941`，Baseline-only 事件 `0`，观测线程 `206`，观测 Turn `1,318`。
- Today Tokens：`11,474,771,534`；Observed Tokens：`25,526,693,512`；API Equivalent Cost：`$2,943.9172`。
- Rate-limit 原始事件：`50,251`；解析后的 Observation：`50,251`；窗口明细 `58,603`。5 小时窗口最新值 `2%`，每周窗口最新值 `80%`。
- State DB 对账（重建启动时快照）：检查 `391`，匹配 `203`，不匹配 `188`；其中未发现对应 Desktop rollout 的线程按不匹配计入。差异允许在 Codex 正在运行或本次快照边界内存在，不能替代最终用户实机验收。
- 旧 App Server 历史：保留 `1` 条；用户设置：保留 `1` 条；外部 Codex `state_5.sqlite` 仍只读访问。
- 环境诊断：Codex Home 为 `C:\Users\ADMIN\.codex`；Sessions Path 为 `C:\Users\ADMIN\.codex\sessions`；State DB 为 `C:\Users\ADMIN\.codex\state_5.sqlite`。
- 解析修正：RFC3339/秒/毫秒时间戳、payload-level `rate_limits`、`info=null` rate-limit-only、Primary/Secondary 双窗口、窗口规范化、canonical thread/fork 保护和项目历史日期均已覆盖自动化测试。

## Token 统计终审

- 结论：`CURRENT TOKEN ACCOUNTING VALID`。未因总量偏大而修改 Token delta、Pricing、Desktop source 筛选、Schema 或索引版本。
- 只读工具：`tools/desktop_token_audit.py`，使用 SQLite `file:<path>?mode=ro`，不执行写入 SQL，不读取或输出 Prompt、助手内容、推理文本、工具参数或会话正文。
- 审计快照：Delta 事件 `199,410`；Observed Tokens `27,775,307,476`；Today Tokens `13,723,385,498`；Input `27,699,757,857`；Cached Input `26,925,846,016`；Uncached Input `773,911,841`；Output `75,549,619`；Reasoning `24,278,343`；缓存输入占比 `97.2061%`；API Equivalent Cost `$3,247.9618`。
- Audit A/B：Total/Input/Cached/Output 与 `last_*` 精确匹配率 `99.9995%`，Reasoning `100%`；SUM delta 与 SUM last 的 Total 差 `51,610`，相对差 `0.0001858%`。唯一差异是一个跨更新累计推进事件：Delta `102,397`，`last_total_tokens` `50,787`，累计数学关系仍成立。
- Audit C：累计值数学一致性 eligible `199,195`，正确 `199,195`，错误 `0`，正确率 `100%`。
- Audit D：首事件 A（`total == last` 且计入 delta）`215`；B（`total != last` 且 baseline）`0`；C（其他）`0`。
- Audit F/G：独立 Today 汇总与 UI Today 口径完全一致；最近 30 天有 `14` 个日期产生 Token 活动，历史数据未集中到今天。
- Audit H：单事件最大 `323,923`；P50 `141,839`；P90 `217,615`；P99 `239,560`；P99.9 `245,071`；超过 context window `3` 条，超过 2 倍 `0` 条，无系统性异常超大事件。
- Audit J：State DB 检查 `400`，匹配 `214`，不匹配 `186`；无 Desktop rollout `185`（均为 VSCode 来源会话：用户/自动化，非 Desktop Direct）；Monitor lower `1`（差 `35,123`）；Monitor higher `0`；近 15 分钟活动线程 `2`，均非 mismatch。
- Weekly Rate Limit：当前 `83%`；最近同一 `limit_id/window_kind/duration/resets_at` 的 64 个历史点均为 `83%`，Burn Rate `0.00` 有数据依据。
- UI 语义优化：Dashboard 改为“今日模型处理 Token”“累计模型处理 Token”，新增“非缓存输入 Token”和缓存输入占比，并说明 Token 包含重复上下文/缓存输入；API 等效成本明确为按 API 单价折算，不是 Plus 订阅实际扣费。
- 本轮没有提升 `desktop_index_revision`，仍为 `2`；Release Ready：`NO`，等待用户确认 Token 统计终审结果。
