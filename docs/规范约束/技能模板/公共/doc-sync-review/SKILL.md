---
name: doc-sync-review
description: 提交代码前审核暂存区改动是否需要同步业务总览、接口/回调、物理模型、枚举、架构与公共能力文档；在 git commit、提交代码、push 前触发。
---

# 提交前文档一致性审核

1. 读取 `.claude/rules/code/doc-sync-review.md`。
2. 执行 `git diff --cached --name-status` 和 `git diff --cached --no-ext-diff`，以暂存区为唯一审核范围。
3. 对照规则中的“项目长期文档”映射，只更新真实受影响的文档，并把文档加入暂存区。
4. 若无需更新文档，明确记录判断依据；不得生成空文档或虚构项目事实。
5. 审核结束执行：

```bash
.claude/skills/doc-sync-review/scripts/doc-sync-gate.sh --record
```

6. 记录后若暂存区再次变化，必须重新执行本 skill；hook 会拒绝过期审核凭证。
