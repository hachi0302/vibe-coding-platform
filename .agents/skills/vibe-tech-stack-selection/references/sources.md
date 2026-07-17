# 方法来源与适配记录

## 直接安装的外部 skill

- `firatcand/founder-skills@software-architect`
- 安装位置：`.agents/skills/software-architect/` 与 `.claude/skills/software-architect/`
- 使用方式：保留为通用架构决策能力。涉及架构形态、后端运行时、数据层时由本项目 skill 按需读取其对应参考文件。

- `adamos486/skills@production-ready`
- 安装位置：`.agents/skills/production-ready/` 与 `.claude/skills/production-ready/`
- 使用方式：已完整阅读。仅借鉴上线基线中健康检查、结构化日志、密钥隔离、迁移、备份、CI、安全与可观测性的检查维度；不执行外部脚本，不把扫描工具或具体平台写成项目工厂默认依赖。

## 本项目改写内容

`vibe-tech-stack-selection` 不复制外部 skill 原文。它只把适用于本产品的决策边界落成可执行约束：

- 最多三项补充问题。
- 项目场景、阶段、已有基础设施优先于本机工具与默认值。
- 用户偏好高权重但不能违背平台与负载约束。
- 模块化单体优先、微服务和中间件需满足明确前提。
- 开发工具、外部服务和上线基线分开展示。
- 每项基础设施必须说明采用、延后或不采用的理由与触发条件。
- 技术方案必须能进入环境检查、预览、脚手架生成和构建验证。

## 未作为依据的候选来源

此前搜索结果中出现但未完整阅读的 skill 或仓库，不在本版本的方法依据中。后续需要采用时，必须先安装或完整阅读，再记录来源、适配范围和验证结果。
