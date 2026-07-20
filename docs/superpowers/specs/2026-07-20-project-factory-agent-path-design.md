# 项目工厂 Agent CLI 路径修复设计

## 问题

项目工厂的技术分析与已有项目初始化直接使用 `Command::new("codex")` / `Command::new("claude")`。从 Finder、Explorer 或 MSI 快捷方式启动桌面应用时，GUI 进程继承的 PATH 可能不包含 Homebrew、npm、nvm、fnm 或 volta 安装目录，导致已经安装的 CLI 报 `No such file or directory`。

## 范围

- 覆盖项目工厂的技术分析与已有项目初始化。
- 同时覆盖 Codex CLI 与 Claude Code。
- macOS、Linux 和 Windows 使用与 GUI Chat 相同的命令解析策略。
- 不改变 Agent 参数、权限、工作目录、输出解析或项目产物规则。

## 方案比较

1. 推荐：把 GUI Chat 已验证的跨平台子进程构造器下沉到 `agent_command`，由 GUI Chat 和项目工厂共同复用。单一实现可同时处理 POSIX 登录 shell 与 Windows PATH/NVM 恢复。
2. 为项目工厂手工补 Homebrew、npm、nvm 等路径。实现短，但会遗漏安装器和版本管理器，并与现有逻辑重复。
3. 启动时解析 CLI 的绝对路径并缓存。能够避免 shell，但需要为各平台维护解析与缓存失效规则，复杂度高于当前故障所需。

采用方案 1。

## 设计

`agent_command` 提供一个返回 `std::process::Command` 的统一构造函数。POSIX 平台通过用户登录交互 shell 执行经过单引号转义的命令，并移除可能干扰 npm 全局命令的 `npm_config_prefix`；Windows 通过现有 PowerShell PATH 刷新逻辑合并进程、用户和系统 PATH，同时解析 NVM 符号链接目录。

项目工厂把原有裸命令改为 `AgentCommand` 参数链，再交给统一构造器执行。GUI Chat 同时改为调用该构造器，防止两份实现继续漂移。

## 错误处理

保留现有用户可见错误前缀与 CLI stderr 摘要。命令确实未安装或登录 shell 也无法解析时仍返回明确失败，不伪造成功或静默降级。

## 测试

- 单元测试验证 POSIX 构造器使用登录交互 shell、保留工作目录并正确转义参数。
- Windows 条件测试验证 PowerShell 命令包含 PATH 刷新与安全参数转义。
- 运行 Rust 项目工厂测试、Clippy、前端测试与构建，确认无回归。

