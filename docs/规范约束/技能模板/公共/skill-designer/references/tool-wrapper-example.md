# Tool Wrapper 模式示例：FastAPI 专家 Skill

这是一个教AI如何编写FastAPI代码的Tool Wrapper skill。

## SKILL.md

```markdown
# skills/api-expert/SKILL.md
---
name: api-expert
description: FastAPI development best practices and conventions. Use when building, reviewing, or debugging FastAPI applications, REST APIs, or Pydantic models.
metadata:
  pattern: tool-wrapper
  domain: fastapi
---

You are an expert in FastAPI development. Apply these conventions to all code you write or review.

## Core Conventions

Load 'references/conventions.md' for the complete list of FastAPI best practices.

## When Reviewing Code

1. Load the conventions reference
2. Check the user's code against each convention
3. For each violation, cite the specific rule and suggest the fix

## When Writing Code

1. Load the conventions reference
2. Follow every convention exactly
3. Add type annotations to all function signatures
4. Use Annotated style for dependency injection
```

## references/conventions.md

```markdown
# FastAPI 开发规范

## 1. 路由函数必须使用async def

```python
# ✅ 正确
@app.get("/items/{item_id}")
async def read_item(item_id: int):
    return {"item_id": item_id}

# ❌ 错误
@app.get("/items/{item_id}")
def read_item(item_id: int):
    return {"item_id": item_id}
```

## 2. 使用Pydantic模型进行请求/响应验证

```python
# ✅ 正确
from pydantic import BaseModel

class ItemCreate(BaseModel):
    name: str
    description: str | None = None
    price: float

@app.post("/items", response_model=ItemCreate)
async def create_item(item: ItemCreate):
    return item

# ❌ 错误 - 直接使用dict
@app.post("/items")
async def create_item(item: dict):
    return item
```

## 3. 使用Annotated进行依赖注入

```python
# ✅ 正确
from typing import Annotated

async def get_db():
    db = DBSession()
    try:
        yield db
    finally:
        db.close()

@app.get("/items")
async def read_items(
    db: Annotated[Session, Depends(get_db)]
):
    return db.query(Item).all()

# ❌ 错误 - 在函数内部创建依赖
@app.get("/items")
async def read_items():
    db = DBSession()
    return db.query(Item).all()
```

## 4. 响应模型必须明确指定

```python
# ✅ 正确
@app.get("/items/{item_id}", response_model=ItemResponse)
async def read_item(item_id: int):
    return ItemResponse(id=item_id, name="Test")

# ❌ 错误 - 没有response_model
@app.get("/items/{item_id}")
async def read_item(item_id: int):
    return {"id": item_id, "name": "Test"}
```

## 5. 使用HTTPException进行错误处理

```python
# ✅ 正确
from fastapi import HTTPException

@app.get("/items/{item_id}")
async def read_item(item_id: int):
    item = db.get_item(item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    return item

# ❌ 错误 - 返回dict错误
@app.get("/items/{item_id}")
async def read_item(item_id: int):
    if not db.get_item(item_id):
        return {"error": "Not found"}
    return item
```

## 6. 路径参数要声明类型

```python
# ✅ 正确
@app.get("/items/{item_id}")
async def read_item(item_id: int):  # 类型声明

# ❌ 错误
@app.get("/items/{item_id}")
async def read_item(item_id):  # 没有类型声明
```

## 7. 使用Query/Path/Body明确声明参数来源

```python
from fastapi import Query, Path, Body

@app.get("/items")
async def read_items(
    skip: int = Query(0, ge=0),
    limit: int = Query(10, le=100)
):
    return items[skip:skip+limit]

@app.get("/items/{item_id}")
async def read_item(
    item_id: int = Path(..., gt=0)
):
    return {"item_id": item_id}
```

## 8. 状态码必须符合HTTP语义

| 操作 | 状态码 |
|------|--------|
| 创建资源 | 201 |
| 成功删除 | 204 |
| 成功但无内容 | 204 |
| 成功返回数据 | 200 |
| 资源不存在 | 404 |
| 创建冲突 | 409 |

## 9. 分离路由和业务逻辑

```python
# ✅ 正确 - 路由只处理HTTP，逻辑在service层
@app.post("/items", response_model=ItemResponse, status_code=201)
async def create_item(item: ItemCreate):
    return item_service.create(item)

# ❌ 错误 - 业务逻辑混在路由里
@app.post("/items")
async def create_item(item: ItemCreate):
    db = DBSession()
    new_item = Item(name=item.name, price=item.price)
    db.add(new_item)
    db.commit()
    db.refresh(new_item)
    return new_item
```

## 10. 日志记录使用标准库

```python
import logging

logger = logging.getLogger(__name__)

@app.get("/items/{item_id}")
async def read_item(item_id: int):
    logger.info(f"Reading item {item_id}")
    item = item_service.get(item_id)
    if not item:
        logger.warning(f"Item {item_id} not found")
        raise HTTPException(status_code=404)
    return item
```
