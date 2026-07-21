use serde::Serialize;

/// A platform template contributes only a stable documentation shape.  Project
/// facts are always supplied by the scanned inventory and source evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DocumentTrigger {
    Project,
    Backend,
    Frontend,
    Api,
    PhysicalDataModel,
    Enum,
    Integration,
    Messaging,
    FrontendClient,
    FrontendComponents,
    FrontendRoutes,
}

impl DocumentTrigger {
    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::Project => "初始化项目资料工程",
            Self::Backend => "发现后端源码或后端模块",
            Self::Frontend => "发现前端源码或前端模块",
            Self::Api => "发现真实 HTTP 路由、Controller、Router 或 OpenAPI 声明",
            Self::PhysicalDataModel => "发现 DDL/迁移/表声明或 ORM 实体模型",
            Self::Enum => "发现项目业务 enum、常量集合或配置字典声明",
            Self::Integration => "发现 Feign/HTTP client/SDK/adapter 等外部边界声明",
            Self::Messaging => "发现消息 topic/consumer/producer 或 MQ 配置声明",
            Self::FrontendClient => "发现前端 API client、请求封装或契约类型声明",
            Self::FrontendComponents => "发现前端组件目录或组件源码",
            Self::FrontendRoutes => "发现前端路由、页面或视图声明",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DocumentTemplateSpec {
    pub id: &'static str,
    pub target_path: &'static str,
    pub layer: &'static str,
    pub trigger: DocumentTrigger,
    pub template_path: &'static str,
    pub required_sections: &'static [&'static str],
}

const KNOWN_GAPS: &str = "待补信息";

const BACKEND_DOCUMENTS: &[DocumentTemplateSpec] = &[
    DocumentTemplateSpec {
        id: "backend-system-architecture",
        target_path: "docs/backend/latest/系统架构/系统架构详解.md",
        layer: "backend",
        trigger: DocumentTrigger::Backend,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &[
            "目录",
            "架构总览",
            "分层架构设计",
            "模块架构详解",
            KNOWN_GAPS,
        ],
    },
    DocumentTemplateSpec {
        id: "backend-business-overview",
        target_path: "docs/backend/latest/业务/业务功能总览.md",
        layer: "backend",
        trigger: DocumentTrigger::Backend,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &[
            "系统架构与模块划分",
            "业务能力总览",
            "接口全景索引",
            KNOWN_GAPS,
        ],
    },
    DocumentTemplateSpec {
        id: "api-contracts",
        target_path: "docs/backend/latest/接口文档/API接口总览.md",
        layer: "backend",
        trigger: DocumentTrigger::Api,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &[
            "通用说明",
            "接口总览",
            "Controller 总览",
            "字段语义与请求体细节",
            KNOWN_GAPS,
        ],
    },
    DocumentTemplateSpec {
        id: "physical-data-model",
        target_path: "docs/backend/latest/接口文档/物理模型总览.md",
        layer: "database",
        trigger: DocumentTrigger::PhysicalDataModel,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["数据库目录", "表字段明细", "字段说明", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "enum-catalog",
        target_path: "docs/backend/latest/接口文档/枚举值总览.md",
        layer: "backend",
        trigger: DocumentTrigger::Enum,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["通用说明", "枚举总览", "枚举详情", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "integration-catalog",
        target_path: "docs/backend/latest/第三方集成/第三方服务总览.md",
        layer: "integration",
        trigger: DocumentTrigger::Integration,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["服务总览", "调用契约", "异常与重试", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "messaging-contracts",
        target_path: "docs/backend/latest/第三方集成/消息与事件总览.md",
        layer: "integration",
        trigger: DocumentTrigger::Messaging,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["消息总览", "生产与消费", "消息契约", KNOWN_GAPS],
    },
];

const FRONTEND_DOCUMENTS: &[DocumentTemplateSpec] = &[
    DocumentTemplateSpec {
        id: "frontend-index",
        target_path: "docs/frontend/latest/index.md",
        layer: "frontend",
        trigger: DocumentTrigger::Frontend,
        template_path: "docs/frontend/latest/index.md",
        required_sections: &["文档索引", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "frontend-architecture",
        target_path: "docs/frontend/latest/系统架构/前端架构详解.md",
        layer: "frontend",
        trigger: DocumentTrigger::Frontend,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["目录", "架构总览", "模块与页面", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "frontend-api-integration",
        target_path: "docs/frontend/latest/接口文档/前端接口接入说明.md",
        layer: "frontend",
        trigger: DocumentTrigger::FrontendClient,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["基础配置", "请求拦截", "响应处理", "接口列表", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "frontend-components",
        target_path: "docs/frontend/latest/前端通用文档/组件与公共能力总览.md",
        layer: "frontend",
        trigger: DocumentTrigger::FrontendComponents,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["组件与公共能力总览", "实际目录结构", "复用约定", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "frontend-feature-map",
        target_path: "docs/frontend/latest/业务/业务功能总览.md",
        layer: "frontend",
        trigger: DocumentTrigger::FrontendRoutes,
        template_path: ".vibe-coding-platform/context-memory/document-template-library.md",
        required_sections: &["业务功能总览", "页面与路由", KNOWN_GAPS],
    },
];

const PROJECT_DOCUMENTS: &[DocumentTemplateSpec] = &[
    DocumentTemplateSpec {
        id: "product-index",
        target_path: "docs/product/latest/index.md",
        layer: "common",
        trigger: DocumentTrigger::Project,
        template_path: "docs/product/latest/index.md",
        required_sections: &["产品资料索引", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "test-index",
        target_path: "docs/test/latest/index.md",
        layer: "common",
        trigger: DocumentTrigger::Project,
        template_path: "docs/test/latest/index.md",
        required_sections: &["测试资料索引", KNOWN_GAPS],
    },
    DocumentTemplateSpec {
        id: "backend-index",
        target_path: "docs/backend/latest/index.md",
        layer: "backend",
        trigger: DocumentTrigger::Backend,
        template_path: "docs/backend/latest/index.md",
        required_sections: &["文档索引", KNOWN_GAPS],
    },
];

pub(super) fn document_template_specs() -> impl Iterator<Item = &'static DocumentTemplateSpec> {
    PROJECT_DOCUMENTS
        .iter()
        .chain(BACKEND_DOCUMENTS.iter())
        .chain(FRONTEND_DOCUMENTS.iter())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateCatalogEntry<'a> {
    id: &'a str,
    target_path: &'a str,
    layer: &'a str,
    trigger: &'a str,
    template_path: &'a str,
    required_sections: &'a [&'a str],
}

pub(super) fn catalog_json() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "purpose": "平台模板提供 IPS 资料工程结构；项目内容只能来自本项目已扫描的信息。",
        "templates": document_template_specs().map(|spec| TemplateCatalogEntry {
            id: spec.id,
            target_path: spec.target_path,
            layer: spec.layer,
            trigger: spec.trigger.description(),
            template_path: spec.template_path,
            required_sections: spec.required_sections,
        }).collect::<Vec<_>>(),
    })
}

pub(super) fn plan_contract() -> String {
    let entries = document_template_specs()
        .map(|spec| {
            format!(
                "- `{}` → `{}`（layer=`{}`；触发：{}；章节：{}）",
                spec.id,
                spec.target_path,
                spec.layer,
                spec.trigger.description(),
                spec.required_sections.join(" / "),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "项目专属文档目录：`.vibe-coding-platform/context-memory/document-templates.json`。必须按 IPS 的中文目录、中文文件名和中文展示结构创建下列文档；内容只写本项目已扫描到的事实。没有信息时在 `{KNOWN_GAPS}` 简短列出缺什么，不得用“真实证据/维护规则/文档漂移/可复用资产”等审计章节替代业务资料：\n{entries}"
    )
}

pub(super) fn template_library_markdown() -> &'static str {
    r#"# IPS 资料工程生成规范

## 总原则

- 只把本项目扫描到的接口、表、字段、枚举、页面、组件、配置和调用关系写入资料；不足的信息只在文末“待补信息”简短列出，不能猜。
- `docs` 下目录、文件名、标题、正文均为中文（固定 `index.md` 除外）。`.claude/rules` 与 `.claude/skills` 的目录、文件名为英文 kebab-case，但所有说明、标题、步骤和示例必须中文；仅代码、命令、路径、YAML 键可保留英文。
- 内部扫描证据仅用于保证内容真实，不能在资料中生成“真实证据”“维护规则”“可复用资产总览”“文档漂移”等审计型文档或章节。
- `latest` 是当前全量资料；用户给出版本和具体需求后才创建版本化详设、进度和前端接入文档。

## 后端资料（复用 IPS latest 结构）

### 接口文档/API接口总览.md

按项目真实端前缀或调用方拆分；每份固定采用 IPS 的“通用说明 → 接口总览（按 Controller/路由分组）→ Controller 总览 → 字段语义与请求体细节”结构。接口总览必须列方法、路径、用途、请求/响应类型、鉴权或错误码；字段细节用“字段 / 类型 / 必填 / 含义”表。无法确认的字段写“待补信息”，不凭框架习惯补齐。

### 接口文档/物理模型总览.md

这是一份数据库资料，不是迁移审计记录。先按库或 schema 给出“数据库目录”：`库 / 表名 / 中文业务名 / 用途`；再按每张真实表建立“表字段明细”，字段表必须完整列出 `字段 / 类型 / 长度 / 允许为空 / 默认值 / 含义 / 主键或索引`。表、字段、长度、可空、默认值、索引均以真实 DDL 为准；只有实体没有 DDL 时，只能说明“未发现物理 DDL，待补”。

### 接口文档/枚举值总览.md

先给“枚举总览”，按真实业务域分组；随后每个枚举单独列出 `值（code）/ 中文名称 / 业务含义`，并在适用时写接口字段或数据库字段。不得只写类名、引用链或技术审计说明；不得捏造枚举值。

### 系统架构、业务、第三方集成与消息

沿用 IPS 的“目录 → 总览 → 按模块或业务域分节”写法，给开发者说明项目真实架构、业务能力和边界。只有扫描到对应实现才创建对应文档。

## 前端资料（按真实前端结构裁剪）

发现前端后，按 IPS 前端资料的写法生成架构、接口对接、页面路由、组件/公共能力和实际存在的 API、组件、composables、stores、types 等规范文档。每份说明真实目录、用途、调用方式和复用入口；不生成项目中不存在的类别。

## 原样安装的通用模板

平台会直接原样安装 IPS 的 `详设文档模板.md`、`开发进度文档模板.md`、`前端接入说明模板.md`，它们不是 AI 生成产物，也不应由计划或文档阶段改写。

## 规则与技能

规则和技能只在项目确有对应复杂流程时创建。正文中文，必须写清触发条件、先读哪些现有资料/代码、复用入口、执行步骤、禁止事项、完成标准；信息不足时在末尾“待补信息”写缺什么。禁止生成通用的开发、修复 Bug、重构、审查或代码风格 skill。

## 软链接

- `AGENTS.md` → `CLAUDE.md`
- `.agents/rules` → `../.claude/rules`
- `.agents/skills` → `../.claude/skills`
- `.agents/scripts` → `../.claude/scripts`

Windows 同样创建软链接；权限不允许时明确提示，不降级为副本。"#
}
