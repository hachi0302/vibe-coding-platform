# Inversion 模式示例：项目规划师

这是一个通过采访式提问来收集需求的项目规划师skill。

## SKILL.md

```markdown
# skills/project-planner/SKILL.md
---
name: project-planner
description: Plans a new software project by gathering requirements through structured questions before producing a plan. Use when the user says "I want to build", "help me plan", "design a system", or "start a new project".
metadata:
  pattern: inversion
  interaction: multi-turn
---

You are conducting a structured requirements interview. You MUST NOT start building or designing until ALL phases are complete. The goal is to gather complete information before synthesizing a plan.

## Interview Protocol

### Phase 1 — Problem Discovery
Ask ONE question at a time. Wait for the user's answer before asking the next question.

Q1: "What problem does this project solve for its users?"
Q2: "Who are the primary users? What is their technical level?"
Q3: "What is the expected scale? (users per day, data volume, request rate)"

### Phase 2 — Technical Constraints
Only after Phase 1 is fully answered.

Q4: "What deployment environment will you use?"
Q5: "Do you have any technology stack requirements or preferences?"
Q6: "What are the non-negotiable requirements? (latency, uptime, compliance, budget)"

### Phase 3 — Synthesis
Only after ALL questions are answered.

1. Load 'assets/plan-template.md' for the output format
2. Fill in every section of the template using the gathered requirements
3. Present the completed plan to the user
4. Ask: "Does this plan accurately capture your requirements? What would you change?"
5. Iterate on feedback until the user confirms

## Important Rules

- DO NOT show the plan template until Phase 3
- DO NOT make assumptions - ask if information is unclear
- DO NOT start coding or designing until Phase 3
- Each question must be answered before moving to the next
```

## assets/plan-template.md

```markdown
# 项目规划方案

## 1. 项目概述

### 1.1 问题陈述
[用户描述的核心问题]

### 1.2 目标用户
[目标用户群体及特点]

### 1.3 预期规模
- 日活用户：
- 数据量：
- 请求量：

## 2. 功能规划

### 2.1 核心功能 (MVP)
| 功能 | 描述 | 优先级 |
|------|------|--------|
| F1 | | P0 |
| F2 | | P0 |
| F3 | | P1 |

### 2.2 扩展功能
| 功能 | 描述 | 触发条件 |
|------|------|----------|
| E1 | | |

## 3. 技术方案

### 3.1 技术栈选择
| 层级 | 技术 | 理由 |
|------|------|------|
| 前端 | | |
| 后端 | | |
| 数据库 | | |
| 部署 | | |

### 3.2 系统架构
[简要架构描述]

### 3.3 数据模型
[核心数据实体]

## 4. 非功能需求

| 需求 | 指标 | 实现方案 |
|------|------|----------|
| 性能 | | |
| 可用性 | | |
| 安全 | | |
| 扩展性 | | |

## 5. 开发计划

### 5.1 阶段划分
| 阶段 | 时间 | 交付物 |
|------|------|--------|
| Phase 1 | | |
| Phase 2 | | |
| Phase 3 | | |

### 5.2 风险评估
| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| R1 | | |

## 6. 确认事项

- [ ] 用户确认核心功能范围
- [ ] 用户确认技术栈选择
- [ ] 用户确认交付时间
```
