# 模式选择决策树

## 快速决策

```
用户说想创建一个skill...
│
├─ "让AI学会某个框架/库的用法"
│   └─→ Tool Wrapper
│
├─ "生成固定格式的文档/报告"
│   └─→ Generator
│
├─ "审查/评审/审计代码或文档"
│   └─→ Reviewer
│
├─ "先问用户问题，再决定做什么"
│   └─→ Inversion
│
├─ "强制按步骤执行，中间有检查点"
│   └─→ Pipeline
│
└─ 不确定？
    │
    ├─ 任务是否复杂、有多个阶段？
    │   ├─ 是 → Pipeline
    │   └─ 否 ↓
    │
    ├─ 需要用户确认或反馈吗？
    │   ├─ 是 → Inversion
    │   └─ 否 ↓
    │
    ├─ 输出是固定格式吗？
    │   ├─ 是 → Generator
    │   └─ 否 → Reviewer（默认）
```

## 详细对比

| 模式 | 核心问题 | 输出 | 用户交互 | 步骤控制 |
|------|----------|------|----------|----------|
| Tool Wrapper | "这个库的最佳实践是什么？" | 应用规范 | 无 | 无 |
| Generator | "按模板生成X" | 结构化文档 | 收集变量 | 无 |
| Reviewer | "这段代码有什么问题？" | 评分+问题列表 | 无 | 无 |
| Inversion | "你想做什么？" | 方案/计划 | 引导式提问 | Phase控制 |
| Pipeline | "按顺序完成这些步骤" | 阶段性产出 | 审批检查点 | 强制顺序 |

## 组合场景

### Pipeline + Reviewer
文档生成后自动进行质量审查。

### Generator + Inversion
先收集变量（Inversion），再填充模板（Generator）。

### Tool Wrapper + Reviewer
先加载规范（Tool Wrapper），再按规范审查（Reviewer）。

## 常见错误

| 错误选择 | 应该用 | 问题 |
|----------|--------|------|
| 用Inversion生成报告 | Generator | 报告格式固定，不需要引导 |
| 用Reviewer做代码生成 | Generator | Reviewer是评审，不是生成 |
| 用Tool Wrapper做项目规划 | Inversion | 规划需要收集需求，不是加载规范 |
| 用Pipeline但不加检查点 | Reviewer | 没有强制步骤，Pipeline优势消失 |
