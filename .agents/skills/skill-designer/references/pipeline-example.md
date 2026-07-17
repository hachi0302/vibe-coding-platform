# Pipeline 模式示例：API文档生成流水线

这是一个强制按步骤执行、有检查点的文档生成pipeline skill。

## SKILL.md

```markdown
# skills/doc-pipeline/SKILL.md
---
name: doc-pipeline
description: Generates API documentation from Python source code through a multi-step pipeline. Use when the user asks to document a module, generate API docs, or create documentation from code.
metadata:
  pattern: pipeline
  steps: "4"
---

You are running a documentation generation pipeline. Execute each step in order. You MUST NOT skip steps or proceed if a step fails.

## Pipeline Steps

### Step 1 — Parse & Inventory

Analyze the user's Python code to extract:
- All public classes
- All public functions (including parameters and return types)
- All public constants

Present the inventory as a checklist:
```
Public API:
- [ ] Class: ClassName
  - method: method_name(param: type) -> type
- [ ] Function: function_name(param: type, param: type) -> type
```

Ask the user: "Is this the complete public API you want documented?"

Wait for confirmation before proceeding.

### Step 2 — Generate Docstrings

GATE: You MUST wait for user confirmation of the inventory before proceeding.

For each function/method lacking a docstring:
1. Load 'references/docstring-style.md' for the required format
2. Generate a docstring following the style guide exactly
3. Present the generated docstring to the user for approval

Do NOT proceed to Step 3 until the user confirms all docstrings.

### Step 3 — Assemble Documentation

GATE: You MUST have confirmed docstrings from Step 2.

1. Load 'assets/api-doc-template.md' for the output structure
2. Compile all classes, functions, and docstrings into the template
3. Ensure every public symbol has documentation

### Step 4 — Quality Check

1. Load 'references/quality-checklist.md'
2. Review the assembled documentation against each quality criterion:
   - Every public symbol documented
   - Every parameter has a type and description
   - Every return type documented
   - At least one usage example per function
3. Report any missing or incomplete documentation
4. Fix reported issues before presenting the final document

## Failure Handling

If any step fails or the user rejects the output:
- Report what failed
- Do NOT skip to the next step
- Do NOT produce a final document with known issues
```

## references/docstring-style.md

```markdown
# Docstring 风格指南

## Google Style（推荐）

```python
def function_name(param1: str, param2: int = 10) -> dict:
    """Short description.

    Longer description if needed. Explain the function's purpose,
    behavior, and any important details.

    Args:
        param1: Description of param1. Include type info.
        param2: Description of param2. (default: 10)

    Returns:
        Description of return value. Include type info.

    Raises:
        ValueError: When this exception is raised.
        TypeError: When this exception is raised.

    Examples:
        >>> result = function_name("hello", 5)
        >>> print(result)
        {'value': 'hello-5'}
    """
```

## 最小要求

Every docstring MUST include:
1. A short description (one sentence)
2. Args section (if function has parameters)
3. Returns section (if function returns a value)

## 禁止事项

- ❌ 不要写 "Calculates the result" - 说清楚怎么计算的
- ❌ 不要写 "Handles errors" - 说清楚处理什么错误
- ❌ 不要留空 "Args: None" - 真的没参数才写None
```
