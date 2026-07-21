# 项目规则索引

本目录用于约束“如何在当前项目中正确迭代”，不是项目介绍、需求文档或通用技能。目录、文件名采用稳定的 English kebab-case；正文统一使用中文。安装时只保留与扫描到的代码层和真实能力相匹配的规则。

## 规则生成与使用原则

1. 每条项目化结论必须能回到当前项目的真实代码、配置、迁移、测试或已有文档；不能把别的项目事实、常见做法或臆测写成项目规则。
2. 规则正文统一使用中文。代码符号、路径、命令、配置键和协议字段可保留原样。
3. 每条规则至少包含：**触发条件、先读与复用优先、禁止事项、验证要求、待补信息**。项目事实不足时保留待补信息，不以默认实现补齐。
4. 没有检测到对应能力时不生成对应专项规则；已有规则也不得因初始化被覆盖。
5. `.agents/rules` 只应链接到 `.claude/rules`，不得复制出两套内容。

## 触发条件 → 必读规则

| 任务或代码特征 | 必读规则 | 生成条件 |
|---|---|---|
| 任意代码、配置、测试或长期文档修改 | `common/development-baseline.md`、`common/reuse-and-impact.md`、`common/facts-and-no-fallbacks.md` | 固定 |
| 新需求、缺陷修复、兼容性调整或有限重构 | `common/development-flow-and-doc-sync.md`、`common/self-test-and-delivery.md` | 固定 |
| 分支、工作区、提交、推送或合并 | `common/git-collaboration-and-history.md` | 固定（无 Git 操作时不触发） |
| 页面、路由、组件、状态、样式、请求或前端测试 | `frontend/engineering.md`、`frontend/verification.md` | 检测到前端源码 |
| 接口入口、业务服务、鉴权、异常、状态或权限 | `backend/api-and-business.md` | 检测到后端源码 |
| 实体、表、查询、索引、迁移或事务 | `backend/persistence-and-migration.md` | 检测到数据库证据 |
| 消息、定时任务、外部服务、软件包、签名或回调 | `backend/async-and-third-party.md` | 检测到对应能力 |

## 待补信息

- 当前项目尚未确认的规则触发入口、目录、符号、验证命令和历史兼容约束，应由初始化扫描结果分别写入对应规则末尾。
- 未确认的信息只能标为待补，不能据此生成默认架构、默认框架、默认命令或默认异常处理方式。
