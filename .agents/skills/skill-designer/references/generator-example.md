# Generator 模式示例：技术报告生成器

这是一个生成结构化技术报告的Generator skill。

## SKILL.md

```markdown
# skills/report-generator/SKILL.md
---
name: report-generator
description: Generates structured technical reports in Markdown. Use when the user asks to write, create, or draft a report, summary, or analysis document.
metadata:
  pattern: generator
  output-format: markdown
---

You are a technical report generator. Follow these steps exactly to produce consistent, well-structured reports.

## Workflow

### Step 1: Load Style Guide
Read 'references/style-guide.md' for tone, formatting rules, and writing conventions.

### Step 2: Load Report Template
Read 'assets/report-template.md' for the required output structure. Every section in the template MUST be present in the output.

### Step 3: Gather Information
Ask the user for any missing information needed to fill the template:
- Topic or subject
- Key findings or data points
- Target audience (technical, executive, general)
- Time period or scope (if applicable)

Do NOT proceed until you have all required information.

### Step 4: Generate Report
Fill the template following the style guide rules exactly. Write in the appropriate tone for the target audience.

### Step 5: Deliver
Return the completed report as a single Markdown document.
```

## references/style-guide.md

```markdown
# 技术报告写作风格指南

## 总体原则

1. **清晰优于华丽** - 技术报告不是文学作品
2. **数据驱动** - 每个声明都要有数据支撑
3. **结构驱动** - 让读者能跳读

## 语气

| 受众 | 语气 | 示例 |
|------|------|------|
| 技术 | 精确，专业、可操作 | "系统延迟从50ms降至20ms" |
| 管理者 | 简洁、关注业务价值 | "性能提升60%，用户体验显著改善" |
| 通用 | 通俗、不使用行话 | "系统变快了，用户等待时间减少" |

## 格式规范

### 标题
- 使用 Markdown 标题 (`#`, `##`, `###`)
- 标题不包含标点符号
- 最多使用三级标题

### 列表
- 优先使用无序列表
- 每项以动词开头
- 列表项不超过20字

### 表格
- 用于对比数据
- 包含表头
- 数值右对齐

### 代码块
- 指定语言
- 不包含行号
- 关键代码加注释

## 禁止事项

- ❌ 不要使用"我认为"、"我觉得"
- ❌ 不要使用模糊的时间描述（"很快"、"很久"）
- ❌ 不要在报告中变换人称
- ❌ 不要使用表情符号
```

## assets/report-template.md

```markdown
# [报告标题]

## 执行摘要
[2-3句话概括：背景、核心发现、建议行动]

## 1. 背景
[项目或问题的背景介绍]

## 2. 关键发现
### 2.1 [发现1标题]
[描述发现，提供数据支撑]

### 2.2 [发现2标题]
[描述发现，提供数据支撑]

### 2.3 [发现3标题]
[描述发现，提供数据支撑]

## 3. 影响分析
[分析这些发现的影响，包括：业务影响、技术影响、风险]

## 4. 建议行动
| 优先级 | 行动项 | 预期收益 | 实施难度 |
|--------|--------|----------|----------|
| P0 | [立即行动] | - | - |
| P1 | [短期内] | - | - |
| P2 | [长期规划] | - | - |

## 5. 结论
[总结报告要点，重申核心建议]

---

报告生成时间：[自动时间戳]
```
