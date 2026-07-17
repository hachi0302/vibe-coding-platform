---
name: skill-designer
description: 教你怎么设计高质量 SKILL.md 内容的 skill。当用户要创建新 skill 时自动触发。
---

# Agent Skill Designer

一个教你怎么设计高质量SKILL.md内容的skill。当用户要创建新skill时自动触发。

## 核心概念

SKILL.md的格式已经标准化了，但**内容设计**才是难点。同样的skill，用不同模式组织，效果天差地别。

**5种设计模式**：

| 模式 | 核心作用 | 何时用 |
|------|---------|--------|
| Tool Wrapper | 让AI临时变成某个库的专家 | 传递框架/库的规范和使用惯例 |
| Generator | 按固定模板生成结构化文档 | 生成固定格式的输出物 |
| Reviewer | 按检查清单评分/审查 | 评审、审计、打分 |
| Inversion | AI先问你，再决定怎么做 | 需求不明确、需要引导 |
| Pipeline | 强制按步骤执行，有检查点 | 复杂任务、不能跳过步骤 |


## 第一步：判断用哪种模式

回答3个问题，快速定位：

**Q1: 这个skill是要产生一个固定的文档/代码结构吗？**
- 是 → Generator
- 否 ↓

**Q2: 这个skill需要先收集用户信息才能工作吗？**
- 是 → Inversion
- 否 ↓

**Q3: 这个skill需要分步骤执行、不能跳过吗？**
- 是 → Pipeline
- 否 ↓

**Q4: 这个skill是让AI"学习"某个库的用法吗？**
- 是 → Tool Wrapper
- 否 ↓

**兜底**: Reviewer（默认模式）


## 第二步：按模式写SKILL.md

### Pattern 1: Tool Wrapper

**适用场景**：让AI在遇到特定技术栈时，自动加载最佳实践

**核心结构**：
```markdown
## 核心规范
读取 'references/conventions.md' 获取完整规范

## 写作时
1. 加载规范文件
2. 严格遵守每一条规则
3. 为所有函数签名添加类型注解

## 审查时
1. 加载规范文件
2. 对照检查每一条
3. 对每个违规，引用具体规则并给出修复建议
```

**关键点**：
- 规范文件独立存放（`references/`）
- 只在真正需要时才加载（渐进式披露）
- 规则要具体、可执行


### Pattern 2: Generator

**适用场景**：生成结构一致的文件（报告、文档、代码模板）

**核心结构**：
```markdown
## 工作流程

Step 1: 加载样式指南
读取 'references/style-guide.md'

Step 2: 加载输出模板
读取 'assets/output-template.md'

Step 3: 询问缺失信息
向用户收集模板中缺失的变量：
- 主题是什么？
- 关键数据点？
- 目标读者？

Step 4: 填充模板
严格按照样式指南的规则填充每个section

Step 5: 输出
返回完整的Markdown文档
```

**关键点**：
- 模板和样式指南独立（`assets/` + `references/`）
- 必须向用户收集缺失变量
- 输出格式固定，不可自定义


### Pattern 3: Reviewer

**适用场景**：代码审查、质量审计、安全检查

**核心结构**：
```markdown
## 审查协议

Step 1: 加载检查清单
读取 'references/review-checklist.md'

Step 2: 理解代码
先读懂代码的目的，再开始批评

Step 3: 逐项检查
对每个违规项：
- 记录位置（行号或大概位置）
- 分类 severity：error / warning / info
- 解释为什么是问题
- 给出具体修复代码

Step 4: 输出结构化报告
- Summary：代码做什么、总体评价
- Findings：按 severity 分组
- Score：1-10分，简短理由
- Top 3 Recommendations：最有价值的改进
```

**关键点**：
- 检查清单独立（可替换）
- 按 severity 分组输出
- 每个问题要解释"为什么"而不只是"是什么"


### Pattern 4: Inversion

**适用场景**：需求不明确、需要AI引导用户思考

**核心结构**：
```markdown
## 采访式交互

你是主持人。**不要**在收集完所有信息前开始构建。

### Phase 1 — 问题发现（一次问一个，等回答）

Q1: 这个项目为用户解决什么问题？
Q2: 主要用户是谁？技术水平如何？
Q3: 预期规模？（日活、数据量、请求量）

### Phase 2 — 技术约束（仅在Phase 1完成后再问）

Q4: 部署环境是什么？
Q5: 技术栈有什么偏好或限制？
Q6: 有什么硬性要求？（延迟、可用性、合规、预算）

### Phase 3 — 整合（仅在所有问题回答后才执行）

1. 加载输出格式模板
2. 用收集的信息填充每个section
3. 呈现完整方案
4. 询问："这个方案准确吗？有什么要改的？"
5. 根据反馈迭代，直到用户确认
```

**关键点**：
- **必须显式声明"Do NOT start building until..."**
- 严格按phase顺序，一次只问一个问题
- 所有问题回答完才能进入合成阶段


### Pattern 5: Pipeline

**适用场景**：复杂任务，必须按顺序执行、中间有检查点

**核心结构**：
```markdown
## 执行流程（不可跳过步骤）

### Step 1 — 解析 & 盘点
分析代码，提取所有公开的类、函数、常量
列出清单，询问："这是你想文档化的完整API吗？"

### Step 2 — 生成文档字符串（Gate: 必须用户确认后才能进入Step 3）
对每个缺少文档的函数：
- 加载 'references/docstring-style.md'
- 按规范生成文档字符串
- 展示给用户审批

### Step 3 — 组装文档
加载 'assets/api-doc-template.md'
将所有类、函数、文档字符串编译成完整API参考文档

### Step 4 — 质量检查
对照 'references/quality-checklist.md' 检查：
- 每个公开符号都有文档
- 每个参数都有类型和描述
- 每个函数至少一个使用示例
修复问题后再呈现最终文档
```

**关键点**：
- 显式的Gate条件（如"必须用户确认后才能进入Step 3"）
- 每步失败则整个pipeline停止
- 可以在Pipeline里嵌套Reviewer（自检）


## 第三步：组合模式

这些模式不是互斥的，可以组合：

| 组合方式 | 适用场景 |
|---------|---------|
| Pipeline + Reviewer | Pipeline末尾加自检步骤 |
| Generator + Inversion | 先收集变量，再填充模板 |
| Tool Wrapper + Reviewer | 学习规范后进行审查 |


## 快速参考

```
我想要...              → 用什么模式
─────────────────────────────────────
让AI学习某框架规范    → Tool Wrapper
生成固定格式的文档    → Generator
代码审查/质量审计     → Reviewer
先问需求再行动        → Inversion
强制按步骤执行        → Pipeline
```


## 常见错误

**错误1**: 把所有规则直接塞进SKILL.md
```markdown
# ❌ 错误 - 规则太多，AI记不住
## 规则1: xxx
## 规则2: xxx
## 规则3: xxx
...（50条规则）
```

```markdown
# ✅ 正确 - 外部引用，按需加载
## 规则
读取 'references/coding-conventions.md' 获取完整规则列表
```

**错误2**: 模式选择错误
- 需要生成结构化文档，却用了Reviewer
- 需要引导需求，却直接开始写代码

**错误3**: 缺少Gate条件
- Pipeline没有检查点，AI跳步
- Inversion没有"Do NOT start"声明，AI直接开始构建


## 参考资源

详细示例和ADK代码参考：
- `references/tool-wrapper-example.md` - FastAPI专家skill示例
- `references/generator-example.md` - 技术报告生成器示例
- `references/reviewer-example.md` - 代码审查员示例
- `references/inversion-example.md` - 项目规划师示例
- `references/pipeline-example.md` - 文档流水线示例
- `references/decision-tree.md` - 模式选择决策树
