use vibe_coding_platform_lib::project_factory::{
    analyze_with_agent, build_analysis_prompt, build_headless_initialization_prompt,
    check_environment, create_project, create_project_with_verification,
    existing_project_init_status, finalize_existing_project_initialization, install_command_for,
    prepare_existing_project_initialization, preview_target_path, read_requirement_materials,
    spring_initializr_dependencies, validate_target_dir, AnalyzeProjectRequest,
    CreateProjectRequest, ProjectProfilePayload, StackRecommendationPayload,
};

#[test]
fn headless_initialization_prompt_requires_real_chinese_outputs_without_visible_init_chat() {
    let prompt =
        build_headless_initialization_prompt("项目路径：/tmp/demo\n按项目真实代码初始化。", None);

    for required in [
        "后台非会话任务",
        "所有面向用户的新文档、规则和项目专属 skill 必须使用中文",
        "完整读取项目",
        "业务功能总览",
        "API接口总览",
        "物理模型总览",
        "所有后端项目都生成项目专属 `backend-log-diagnose` skill",
        "不得修改业务代码",
        "不得改写 skill-designer",
        "不得生成 worktree skill",
    ] {
        assert!(
            prompt.contains(required),
            "missing prompt contract: {required}"
        );
    }
    assert!(!prompt.contains("请输出 WORKFLOW_CHECKPOINT"));
    assert!(!prompt.contains("在聊天框"));
}

#[test]
fn headless_initialization_repair_prompt_includes_the_real_validation_failure() {
    let prompt = build_headless_initialization_prompt(
        "项目路径：/tmp/demo",
        Some("缺少初始化后的真实文档：docs/backend/latest/接口文档/API接口总览.md"),
    );

    assert!(prompt.contains("上一次产物校验失败"));
    assert!(prompt.contains("API接口总览.md"));
    assert!(prompt.contains("只补齐校验指出的缺口"));
}

#[test]
fn requirement_materials_read_nested_files_in_stable_order_and_skip_build_directories() {
    let root = std::env::temp_dir().join(format!(
        "vibe-requirement-materials-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("brief/nested")).expect("create nested requirement fixture");
    std::fs::create_dir_all(root.join("node_modules/pkg")).expect("create skipped fixture");
    std::fs::create_dir_all(root.join("target/debug")).expect("create skipped target fixture");
    std::fs::write(root.join("z-last.md"), "最后一个需求").expect("write markdown fixture");
    std::fs::write(root.join("brief/a-first.txt"), "第一个需求").expect("write text fixture");
    std::fs::write(root.join("brief/nested/config.yaml"), "feature: enabled")
        .expect("write yaml fixture");
    std::fs::write(root.join("brief/mock.png"), [0_u8, 1, 2, 3]).expect("write image fixture");
    std::fs::write(root.join("node_modules/pkg/ignored.md"), "不能进入上下文")
        .expect("write ignored fixture");
    std::fs::write(root.join("target/debug/ignored.txt"), "不能进入上下文")
        .expect("write ignored target fixture");

    let bundle = read_requirement_materials(root.to_str().expect("utf8 fixture path"))
        .expect("read recursive requirement materials");

    let relative_paths = bundle
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        relative_paths,
        [
            "brief/a-first.txt",
            "brief/mock.png",
            "brief/nested/config.yaml",
            "z-last.md"
        ]
    );
    assert!(bundle.text.contains("第一个需求"));
    assert!(bundle.text.contains("feature: enabled"));
    assert!(bundle.text.contains("最后一个需求"));
    assert!(!bundle.text.contains("不能进入上下文"));
    assert!(bundle
        .text
        .contains(root.join("brief/mock.png").to_string_lossy().as_ref()));
    assert!(bundle
        .warnings
        .iter()
        .any(|warning| warning.contains("图片未在本地转写")));
    assert!(bundle.source_label.contains("文件夹"));

    std::fs::remove_dir_all(root).expect("remove requirement fixture");
}

#[test]
fn requirement_materials_support_a_single_file() {
    let root = std::env::temp_dir().join(format!(
        "vibe-single-requirement-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&root);
    std::fs::write(&root, r#"{"feature":"本机资料"}"#).expect("write single file fixture");

    let bundle = read_requirement_materials(root.to_str().expect("utf8 fixture path"))
        .expect("read a single requirement file");

    assert_eq!(bundle.files.len(), 1);
    assert_eq!(
        bundle.files[0].relative_path,
        root.file_name().unwrap().to_string_lossy()
    );
    assert!(bundle.files[0].included);
    assert!(bundle.text.contains("本机资料"));
    assert!(bundle.source_label.contains("文件 ·"));

    std::fs::remove_file(root).expect("remove single file fixture");
}

fn platform_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must live below the platform root")
        .to_path_buf()
}

fn relative_file_bytes(root: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn visit(
        root: &std::path::Path,
        current: &std::path::Path,
        files: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        let mut entries = std::fs::read_dir(current)
            .expect("read skill directory")
            .map(|entry| entry.expect("read skill entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else if path.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .expect("skill file must be below root")
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, std::fs::read(path).expect("read skill bytes"));
            }
        }
    }

    let mut files = std::collections::BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn platform_skill_designer_template_contains_the_complete_file_tree() {
    let platform = platform_root().join("docs/规范约束/技能模板/公共/skill-designer");
    let files = relative_file_bytes(&platform);

    for expected in [
        "SKILL.md",
        "evals/evals.json",
        "references/decision-tree.md",
        "references/generator-example.md",
        "references/inversion-example.md",
        "references/pipeline-example.md",
        "references/reviewer-example.md",
        "references/tool-wrapper-example.md",
    ] {
        assert!(
            files.contains_key(expected),
            "platform skill-designer template is missing {expected}"
        );
    }
}

#[test]
fn platform_template_library_has_only_document_rule_and_skill_sources() {
    let constraints = platform_root().join("docs/规范约束");
    let mut directories = std::fs::read_dir(&constraints)
        .expect("read template library")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    directories.sort();

    assert_eq!(directories, ["技能模板", "文档模板", "规则模板"]);
    assert!(!constraints.join("文档模板/运维").exists());
    assert!(!constraints.join("规则模板/运维").exists());

    let detail = std::fs::read_to_string(constraints.join("文档模板/公共/详设文档模板.md"))
        .expect("read detail design template");
    for heading in [
        "文档归档位置",
        "前置确认",
        "方案概述",
        "兼容性与影响分析",
        "模块归属与调用链路",
        "复用 vs 新增设计决策",
        "接口与交互契约设计",
        "数据与持久化设计",
        "业务流程设计",
        "代码设计",
        "实现状态门禁清单",
        "TDD 与自测",
        "前端接入说明",
        "关联文档",
        "项目知识与 skill 回流清单",
        "附录 A：禁止事项",
        "附录 B：交付前自检清单",
    ] {
        assert!(
            detail.contains(heading),
            "missing detail heading: {heading}"
        );
    }
    assert!(detail.lines().count() >= 850);

    let progress = std::fs::read_to_string(constraints.join("文档模板/公共/开发进度文档模板.md"))
        .expect("read progress template");
    for heading in [
        "进度文档硬约束",
        "完成状态",
        "功能点清单",
        "TDD 测试进度",
        "开发自测归档目录指针",
        "端到端真实自测",
        "开发前 TDD",
        "用户反馈区",
        "开发完 Review",
        "历史功能影响审视",
        "预期效果验证",
        "前端接入说明同步检查",
        "复盘归纳",
        "latest/ 长期文档同步审视",
    ] {
        assert!(
            progress.contains(heading),
            "missing progress heading: {heading}"
        );
    }
    assert!(progress.lines().count() >= 250);

    let frontend = std::fs::read_to_string(constraints.join("文档模板/公共/前端接入说明模板.md"))
        .expect("read frontend integration template");
    for heading in [
        "文档纪律",
        "文件归档约定",
        "表达形式选择规则",
        "变更日志",
        "变更总览",
        "接口契约",
        "调用时序 / 页面交互流程",
        "限制与错误处理",
        "前端改动清单",
        "多端影响矩阵",
        "数据变更",
        "发布顺序",
        "写作纪律",
    ] {
        assert!(
            frontend.contains(heading),
            "missing frontend heading: {heading}"
        );
    }
    assert!(frontend.lines().count() >= 300);

    for (name, template) in [
        ("detail", &detail),
        ("progress", &progress),
        ("frontend", &frontend),
    ] {
        for forbidden in [
            "legacy-doc-engineering",
            "legacy-record",
            "legacy-trade",
            "legacy-channel-core",
            "legacy-channel-gateway",
            "易宝",
            "智汇分账通",
            "192.168.10.34",
        ] {
            assert!(
                !template.contains(forbidden),
                "{name} template still contains project-specific token: {forbidden}"
            );
        }
    }

    let model = std::fs::read_to_string(constraints.join("文档模板/后端/物理模型总览模板.md"))
        .expect("read physical model template");
    assert!(model.contains("## 表清单"));
    assert!(model.contains("## 表字段"));
    assert!(!model.contains("接口关系"));
    assert!(!model.contains("迁移历史"));
}

#[test]
fn every_project_skill_template_contains_the_runtime_skill_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/规范约束/技能模板");
    for relative in [
        "公共/code-review/SKILL.md",
        "公共/detail-design-writer/SKILL.md",
        "公共/developer/SKILL.md",
        "公共/problem-diagnose/SKILL.md",
        "公共/review-feedback-handler/SKILL.md",
        "前端/frontend-self-test/SKILL.md",
        "后端/backend-self-test/SKILL.md",
        "可选/backend-log-diagnose/SKILL.md",
        "可选/database-read-diagnose/SKILL.md",
        "可选/ddl-review/SKILL.md",
        "可选/external-integration/SKILL.md",
    ] {
        let content = std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        for required in [
            "metadata:",
            "pattern:",
            "## 项目资源",
            "## 执行流程",
            "## 完成 Gate",
            "## 失败处理",
        ] {
            assert!(
                content.contains(required),
                "{relative} missing template contract: {required}"
            );
        }
    }
}

fn fixture_spring_project(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("vibe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src/main/java/com/example"))
        .expect("create backend source directory");
    std::fs::write(
        root.join("pom.xml"),
        "<project><parent><artifactId>spring-boot-starter-parent</artifactId></parent><dependencies><dependency><artifactId>spring-boot-starter-data-jpa</artifactId></dependency><dependency><artifactId>mysql-connector-j</artifactId></dependency></dependencies></project>",
    )
    .expect("write spring manifest");
    std::fs::write(
        root.join("src/main/java/com/example/SamplePaymentService.java"),
        "package com.example;\n\nimport org.springframework.stereotype.Service;\n\n@Service\npublic class SamplePaymentService {\n    public void processOrder() {}\n}\n",
    )
    .expect("write backend source evidence");
    root
}

fn fixture_frontend_project(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("vibe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("create frontend source directory");
    std::fs::write(
        root.join("package.json"),
        r#"{"scripts":{"build":"vite build","test":"vitest run"},"dependencies":{"vue":"^3.5.0"},"devDependencies":{"typescript":"^5.7.0","vite":"^6.0.0","vitest":"^3.0.0"}}"#,
    )
    .expect("write frontend manifest");
    std::fs::write(
        root.join("src/App.vue"),
        "<script setup lang=\"ts\">\nconst pageTitle = '示例';\n</script>\n<template><main>{{ pageTitle }}</main></template>",
    )
    .expect("write frontend entry");
    root
}

fn write_real_backend_assets(root: &std::path::Path) {
    for (relative, content) in [
        ("docs/backend/MOC.md", "# 示例支付服务后端文档导航\n\n长期文档位于 latest，历次需求位于版本目录。业务、架构、接口、物理模型和规范模板均从本页进入，并随真实代码变化同步。"),
        ("docs/backend/latest/index.md", "# 示例支付服务项目文档索引\n\n该服务负责商户支付订单处理，使用 Spring Boot 与 JPA，证据来自 pom.xml。源码位于 src/main/java，后续开发必须先读业务总览、系统架构和命中规则。"),
        ("docs/backend/latest/业务/业务功能总览.md", "# 业务功能总览\n\n## 支付订单\n\n订单由 Controller 接收并由 Service 处理。证据：`src/main/java`。当前样例只登记代码可证明的支付入口，不把尚未实现的退款、回调或异步能力写成已支持。"),
        ("docs/backend/latest/系统架构/系统架构详解.md", "# 系统架构详解\n\nSpring Boot 应用按 Web、Service、Repository 分层。证据：`pom.xml`。依赖方向由入口到业务再到持久化，公共错误、日志、事务和测试能力以当前源码为准。"),
        ("docs/backend/latest/接口文档/API接口总览.md", "# API 接口总览\n\n当前样例尚无 Controller，未识别对外接口。证据：完整扫描 `src/main/java`。因此本页明确记录当前没有可确认 API，而不虚构方法、路径、请求字段或成功响应。"),
        ("docs/backend/latest/接口文档/物理模型总览.md", "# 物理模型总览\n\n## 表清单\n\n| 表名 | 中文名称 | 主要用途 |\n|---|---|---|\n| sample_order | 示例订单表 | 保存已确认的订单数据 |\n\n## 表字段\n\n### sample_order\n\n| 字段 | 类型 | 是否为空 | 含义 | 备注 |\n|---|---|---|---|---|\n| id | bigint | 否 | 主键 | 实体证据来自数据访问依赖 |"),
        ("CLAUDE.md", "# 示例支付服务 AI 开发指南\n\n该项目使用 Spring Boot 与 JPA。修改前先读 `docs/backend/latest/index.md`、`.claude/rules/README.md`、命中的 `.claude/skills/` 和同类源码；优先复用现有公共能力，不添加伪默认值、吞错或静默降级。构建使用 `mvn clean package`，自测使用 `mvn test`。详设、TDD、实现、自测和长期文档同步必须形成闭环，提交由用户决定。"),
    ] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(path, content).expect("write real asset");
    }
    for rule in [
        "README.md",
        "公共/开发基线.md",
        "公共/复用与影响面.md",
        "公共/事实与兜底边界.md",
        "公共/开发流程与文档同步.md",
        "公共/自测与交付.md",
        "后端/API与业务实现规则.md",
        "后端/持久化与迁移规则.md",
    ] {
        let path = root.join(".claude/rules").join(rule);
        std::fs::create_dir_all(path.parent().expect("rule parent")).expect("create rule");
        std::fs::write(
            path,
            format!("# {rule}\n\n本规则来自当前 Spring Boot 示例项目。业务服务扩展点是 `src/main/java/com/example/SamplePaymentService.java` 中的 `SamplePaymentService`（`@Service`）；修改前必须读取该服务与关联测试，全局检索同类实现，保持历史行为与契约，完成真实测试后再交付。"),
        )
        .expect("write rule");
    }
    let detail_template = format!(
        "# 详设文档模板\n\n## 前置材料与前置确认\n\n## 变更摘要与方案概述\n\n## 兼容性与影响分析\n\n## 代码设计\n\n## TDD 与自测要点\n\n{}",
        "每次使用必须结合真实源码、接口、数据和测试证据，不保留占位符。\n".repeat(80)
    );
    let progress_template = format!(
        "# 开发进度文档模板\n\n## 完成状态\n\n## 开发清单\n\n## TDD\n\n## 用户反馈\n\n## 文档同步\n\n{}",
        "每项状态只能由真实代码、命令和测试结果推进，失败时保留真实证据。\n".repeat(80)
    );
    for (relative, content) in [
        (
            "docs/backend/latest/规范约束/详设文档模板.md",
            detail_template,
        ),
        (
            "docs/backend/latest/规范约束/开发进度文档模板.md",
            progress_template,
        ),
    ] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("template parent"))
            .expect("create project template directory");
        std::fs::write(path, content).expect("write project template");
    }
    for skill in [
        "detail-design-writer",
        "developer",
        "problem-diagnose",
        "code-review",
        "review-feedback-handler",
        "backend-self-test",
        "backend-log-diagnose",
    ] {
        let path = root.join(".claude/skills").join(skill).join("SKILL.md");
        std::fs::create_dir_all(path.parent().expect("skill parent")).expect("create skill");
        std::fs::write(
            path,
            format!("---\nname: {skill}\ndescription: Use when the current Spring Boot project needs the {skill} workflow.\nmetadata:\n  pattern: pipeline\n---\n\n# {skill}\n\n## 项目资源\n\n- 入口：`CLAUDE.md`\n- 总览：`docs/backend/latest/index.md`\n- 规则：`.claude/rules/README.md`\n- 源码：`src/main/java`\n- 测试：`mvn test`\n\n## 执行流程\n\n1. 读取项目入口、业务总览、架构和命中规则。\n2. 沿真实 Spring Boot 入口、Service、Repository 与测试追踪完整链路。\n3. 保留源码路径、命令与测试结果证据，不覆盖无关历史改动。\n4. 使用项目现有异常、日志、持久化和测试基座，不创建平行框架。\n5. 完成后同步受影响长期文档并报告真实结果。\n\n## 完成 Gate\n\n- 结论有 `src/main/java` 代码或 `mvn test` 结果支撑。\n- 正常、边界、异常、原 Bug 和直接回归均已覆盖。\n- 未伪造成功，未添加未经需求确认的兜底。\n\n## 失败处理\n\n命令失败时报告真实错误和未验证范围；不得删除测试、放宽断言或声称应该通过。"),
        )
        .expect("write skill");
    }
}

fn write_real_frontend_assets(root: &std::path::Path) {
    for (relative, content) in [
        ("docs/frontend/MOC.md", "# 示例管理端文档导航\n\n长期文档位于 latest，历次需求位于版本目录。业务、架构、公共组件与规范模板均从本页进入，并随真实代码变化同步。"),
        ("docs/frontend/latest/index.md", "# 示例管理端项目文档索引\n\n该项目使用 Vue、TypeScript 与 Vite 构建管理端页面，证据来自 package.json 与 src/App.vue。后续开发必须先读业务总览、前端架构和命中规则。"),
        ("docs/frontend/latest/业务/业务功能总览.md", "# 业务功能总览\n\n## 示例首页\n\n当前项目由 `src/App.vue` 提供示例首页。这里只记录源码可以证明的页面与交互，不把尚未实现的路由、接口或业务能力写成已支持。"),
        ("docs/frontend/latest/系统架构/前端架构.md", "# 前端架构\n\n项目由 Vue 组件、TypeScript 和 Vite 构成，入口、组件、样式、构建和测试边界均以 package.json 与 src 目录的真实实现为准。"),
        ("docs/frontend/latest/公共能力/组件与公共能力.md", "# 组件与公共能力\n\n当前项目仅确认 `src/App.vue` 这一页面入口；尚未发现可复用组件库、状态容器或请求封装，因此不虚构公共能力。后续新增前必须先检索现有实现。"),
        ("CLAUDE.md", "# 示例管理端 AI 开发指南\n\n该项目使用 Vue、TypeScript 与 Vite。修改前先读 `docs/frontend/latest/index.md`、`.claude/rules/README.md`、命中的 `.claude/skills/` 和同类源码；优先复用现有组件、样式与工具，不添加伪默认值、吞错或静默降级。构建使用 `npm run build`，自测使用 `npm test`。详设、TDD、实现、自测和长期文档同步必须形成闭环，提交由用户决定。"),
    ] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(path, content).expect("write real frontend asset");
    }
    for rule in [
        "README.md",
        "公共/开发基线.md",
        "公共/复用与影响面.md",
        "公共/事实与兜底边界.md",
        "公共/开发流程与文档同步.md",
        "公共/自测与交付.md",
        "前端/前端工程规则.md",
        "前端/前端验证规则.md",
    ] {
        let path = root.join(".claude/rules").join(rule);
        std::fs::create_dir_all(path.parent().expect("rule parent")).expect("create rule");
        std::fs::write(
            path,
            format!("# {rule}\n\n本规则来自当前 Vue 示例项目。页面组件入口是 `src/App.vue` 中的 `App` 组件；修改前必须读取该组件、样式与关联测试，全局检索同类实现，保持历史交互与契约，完成真实构建和测试后再交付。"),
        )
        .expect("write rule");
    }
    let detail_template = format!(
        "# 详设文档模板\n\n## 前置材料与前置确认\n\n## 变更摘要与方案概述\n\n## 兼容性与影响分析\n\n## 代码设计\n\n## TDD 与自测要点\n\n{}",
        "每次使用必须结合真实页面、组件、交互和测试证据，不保留占位符。\n".repeat(80)
    );
    let progress_template = format!(
        "# 开发进度文档模板\n\n## 完成状态\n\n## 开发清单\n\n## TDD\n\n## 用户反馈\n\n## 文档同步\n\n{}",
        "每项状态只能由真实代码、命令和测试结果推进，失败时保留真实证据。\n".repeat(80)
    );
    for (relative, content) in [
        (
            "docs/frontend/latest/规范约束/详设文档模板.md",
            detail_template,
        ),
        (
            "docs/frontend/latest/规范约束/开发进度文档模板.md",
            progress_template,
        ),
    ] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("template parent"))
            .expect("create project template directory");
        std::fs::write(path, content).expect("write project template");
    }
    for skill in [
        "detail-design-writer",
        "developer",
        "problem-diagnose",
        "code-review",
        "review-feedback-handler",
        "frontend-self-test",
    ] {
        let path = root.join(".claude/skills").join(skill).join("SKILL.md");
        std::fs::create_dir_all(path.parent().expect("skill parent")).expect("create skill");
        std::fs::write(
            path,
            format!("---\nname: {skill}\ndescription: Use when the current Vue project needs the {skill} workflow.\nmetadata:\n  pattern: pipeline\n---\n\n# {skill}\n\n## 项目资源\n\n- 入口：`CLAUDE.md`\n- 总览：`docs/frontend/latest/index.md`\n- 规则：`.claude/rules/README.md`\n- 源码：`src`\n- 测试：`npm test`\n\n## 执行流程\n\n1. 读取项目入口、业务总览、架构和命中规则。\n2. 沿真实页面、组件、状态、样式与测试追踪完整交互链路。\n3. 保留源码路径、命令与测试结果证据，不覆盖无关历史改动。\n4. 使用项目现有组件、工具、错误处理和测试基座，不创建平行框架。\n5. 完成后同步受影响长期文档并报告真实结果。\n\n## 完成 Gate\n\n- 结论有 `src` 代码、`npm run build` 或 `npm test` 结果支撑。\n- 正常、边界、异常、原 Bug 和直接回归均已覆盖。\n- 未伪造成功，未添加未经需求确认的兜底。\n\n## 失败处理\n\n命令失败时报告真实错误和未验证范围；不得删除测试、放宽断言或声称应该通过。"),
        )
        .expect("write skill");
    }
}

#[test]
fn preparing_existing_project_installs_only_required_templates_and_original_skill_designer() {
    let root = fixture_spring_project("existing-prepare");
    std::fs::create_dir_all(root.join("docs")).expect("create original docs");
    std::fs::write(root.join("docs/已有说明.md"), "# 原有说明\n\n不要覆盖。")
        .expect("write original docs");

    let result = prepare_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("prepare existing project");

    assert!(result.layers.backend);
    assert!(result
        .detected_stack
        .iter()
        .any(|item| item == "Spring Boot"));
    assert!(root.join("docs/已有说明.md").is_file());
    assert!(root
        .join("docs/backend/latest/规范约束/详设文档模板.md")
        .is_file());
    assert!(root
        .join("docs/backend/latest/规范约束/开发进度文档模板.md")
        .is_file());
    assert!(root
        .join(".claude/skills/skill-designer/references/decision-tree.md")
        .is_file());
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/文档模板/后端/API接口总览模板.md")
        .is_file());
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/规则模板/公共/事实与兜底边界.md")
        .is_file());
    assert!(root
        .join(
            ".vibe-coding-platform/init-reference-v3/技能模板/可选/database-read-diagnose/SKILL.md"
        )
        .is_file());
    assert!(
        !root
            .join(".vibe-coding-platform/init-reference-v3/文档模板/前端")
            .exists(),
        "纯后端项目不得安装前端文档模板"
    );
    assert!(
        !root
            .join(".vibe-coding-platform/init-reference-v3/规则模板/前端")
            .exists(),
        "纯后端项目不得安装前端规则模板"
    );
    assert!(
        !root
            .join(".vibe-coding-platform/init-reference-v3/技能模板/前端")
            .exists(),
        "纯后端项目不得安装前端技能模板"
    );
    assert_eq!(
        relative_file_bytes(&root.join(".claude/skills/skill-designer")),
        relative_file_bytes(&platform_root().join("docs/规范约束/技能模板/公共/skill-designer")),
        "prepared projects must receive every platform skill-designer template file byte for byte"
    );
    assert!(!root.join("CLAUDE.md").exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn preparing_existing_project_installs_a_staged_diff_document_sync_gate() {
    let root = fixture_spring_project("existing-prepare-doc-sync-gate");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("run git command")
    };
    assert!(git(&["init", "-q"]).status.success());

    prepare_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("prepare existing project");

    let rule = root.join(".claude/rules/code/doc-sync-review.md");
    let skill = root.join(".claude/skills/doc-sync-review/SKILL.md");
    let gate = root.join(".claude/skills/doc-sync-review/scripts/doc-sync-gate.sh");
    let hook = root.join(".githooks/pre-commit");
    assert!(rule.is_file(), "初始化必须安装提交前文档审核规则");
    assert!(skill.is_file(), "初始化必须安装文档审核 skill");
    assert!(gate.is_file(), "初始化必须安装确定性审核脚本");
    assert!(hook.is_file(), "初始化必须安装 pre-commit hook");
    assert_eq!(
        String::from_utf8_lossy(&git(&["config", "--get", "core.hooksPath"]).stdout).trim(),
        ".githooks"
    );

    std::fs::write(root.join("src/main/java/Demo.java"), "class Demo {}\n")
        .expect("write staged source");
    assert!(git(&["add", "src/main/java/Demo.java"]).status.success());
    let blocked = std::process::Command::new(&hook)
        .current_dir(&root)
        .output()
        .expect("run hook without review receipt");
    assert!(!blocked.status.success(), "没有审核凭证时必须阻止提交");
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("文档一致性审核"));

    assert!(std::process::Command::new(&gate)
        .arg("--record")
        .current_dir(&root)
        .status()
        .expect("record staged diff review")
        .success());
    assert!(
        std::process::Command::new(&hook)
            .current_dir(&root)
            .status()
            .expect("run hook with current receipt")
            .success(),
        "同一份暂存区完成审核后必须允许提交"
    );

    std::fs::write(
        root.join("src/main/java/Demo.java"),
        "class Demo { int value; }\n",
    )
    .expect("change staged source");
    assert!(git(&["add", "src/main/java/Demo.java"]).status.success());
    assert!(
        !std::process::Command::new(&hook)
            .current_dir(&root)
            .status()
            .expect("run hook with stale receipt")
            .success(),
        "暂存区发生变化后旧审核凭证必须失效"
    );

    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn preparing_frontend_project_excludes_backend_reference_assets() {
    let root = fixture_frontend_project("existing-prepare-frontend");

    let result = prepare_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("prepare frontend project");

    assert!(result.layers.frontend);
    assert!(!result.layers.backend);
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/文档模板/前端/前端架构模板.md")
        .is_file());
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/规则模板/前端/前端工程规则.md")
        .is_file());
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/技能模板/前端/frontend-self-test/SKILL.md")
        .is_file());
    assert!(!root
        .join(".vibe-coding-platform/init-reference-v3/文档模板/后端")
        .exists());
    assert!(!root
        .join(".vibe-coding-platform/init-reference-v3/规则模板/后端")
        .exists());
    assert!(!root
        .join(".vibe-coding-platform/init-reference-v3/技能模板/后端")
        .exists());
    assert!(!root
        .join(".vibe-coding-platform/init-reference-v3/技能模板/可选/backend-log-diagnose")
        .exists());

    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn preparing_fullstack_project_includes_frontend_and_backend_reference_assets() {
    let root = fixture_spring_project("existing-prepare-fullstack");
    std::fs::write(
        root.join("package.json"),
        r#"{"dependencies":{"react":"^19.0.0"},"devDependencies":{"vite":"^6.0.0"}}"#,
    )
    .expect("write frontend manifest");
    std::fs::write(
        root.join("src/App.tsx"),
        "export default () => <main>示例</main>;",
    )
    .expect("write frontend entry");

    let result = prepare_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("prepare fullstack project");

    assert!(result.layers.frontend);
    assert!(result.layers.backend);
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/文档模板/前端/前端架构模板.md")
        .is_file());
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/文档模板/后端/系统架构详解模板.md")
        .is_file());
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/技能模板/前端/frontend-self-test/SKILL.md")
        .is_file());
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3/技能模板/后端/backend-self-test/SKILL.md")
        .is_file());

    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_rejects_empty_template_documents_and_leaves_project_uninitialized() {
    let root = fixture_spring_project("existing-finalize-empty");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join("docs/backend/latest/index.md"),
        "# {{项目名称}}\n\n待填写",
    )
    .expect("write template");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("empty templates must not finalize");

    assert!(error.contains("index"));
    assert!(
        !existing_project_init_status(root.to_str().expect("valid path"))
            .expect("read status")
            .initialized
    );
    let entry = std::fs::read_to_string(root.join("CLAUDE.md")).expect("preserve existing entry");
    assert!(!entry.contains("vibe-coding-platform:init:"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_marks_only_real_project_specific_assets_as_initialized() {
    let root = fixture_spring_project("existing-finalize-real");
    write_real_backend_assets(&root);

    let result = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("finalize real project assets");
    let status = existing_project_init_status(root.to_str().expect("valid path"))
        .expect("read initialized status");

    assert!(status.initialized);
    assert_eq!(status.marker_version.as_deref(), Some("v3"));
    assert!(result
        .generated
        .iter()
        .any(|item| item == "docs/backend/latest/index.md"));
    assert!(std::fs::read_to_string(root.join("CLAUDE.md"))
        .expect("read entry")
        .contains("vibe-coding-platform:init:v3"));
    assert!(!root
        .join(".vibe-coding-platform/init-reference-v3")
        .exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn finalization_creates_all_shared_agent_links_after_validation() {
    let root = fixture_spring_project("existing-finalize-shared-links");
    write_real_backend_assets(&root);

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("finalize real project assets");

    for name in ["rules", "skills", "scripts"] {
        let path = root.join(".agents").join(name);
        let metadata = std::fs::symlink_metadata(&path).expect("shared link metadata");
        assert!(
            metadata.file_type().is_symlink(),
            "{name} must be a symlink"
        );
        assert_eq!(
            std::fs::read_link(path).expect("read shared link"),
            std::path::PathBuf::from(format!("../.claude/{name}"))
        );
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_never_overwrites_a_regular_agents_directory() {
    let root = fixture_spring_project("existing-finalize-regular-agents-directory");
    write_real_backend_assets(&root);
    let protected = root.join(".agents/rules/用户原规则.md");
    std::fs::create_dir_all(protected.parent().expect("protected parent"))
        .expect("create protected rules directory");
    std::fs::write(&protected, "# 用户原规则\n\n必须保留该目录和文件。")
        .expect("write protected rule");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("a regular shared directory must block finalization");

    assert!(error.contains(".agents/rules"));
    assert_eq!(
        std::fs::read_to_string(&protected).expect("protected rule survives"),
        "# 用户原规则\n\n必须保留该目录和文件。"
    );
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3")
        .is_dir());
    assert!(!std::fs::read_to_string(root.join("CLAUDE.md"))
        .expect("read entry")
        .contains("vibe-coding-platform:init:"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn pure_backend_finalization_rejects_frontend_formal_outputs_without_deleting_them() {
    let root = fixture_spring_project("existing-finalize-backend-layer-pollution");
    write_real_backend_assets(&root);
    let unexpected = root.join("docs/frontend/latest/index.md");
    std::fs::create_dir_all(unexpected.parent().expect("unexpected parent"))
        .expect("create frontend formal directory");
    std::fs::write(&unexpected, "# 用户原有前端文档\n\n平台不得删除这个文件。")
        .expect("write original doc");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("pure backend must reject frontend formal outputs");

    assert!(error.contains("纯后端"));
    assert!(error.contains("docs/frontend/latest/index.md"));
    assert_eq!(
        std::fs::read_to_string(&unexpected).expect("original doc survives"),
        "# 用户原有前端文档\n\n平台不得删除这个文件。"
    );
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3")
        .is_dir());
    assert!(!std::fs::read_to_string(root.join("CLAUDE.md"))
        .expect("read entry")
        .contains("vibe-coding-platform:init:"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn pure_frontend_finalization_rejects_backend_formal_outputs_without_deleting_them() {
    let root = fixture_frontend_project("existing-finalize-frontend-layer-pollution");
    prepare_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("prepare frontend project");
    write_real_frontend_assets(&root);
    let unexpected = root.join("docs/backend/latest/index.md");
    std::fs::create_dir_all(unexpected.parent().expect("unexpected parent"))
        .expect("create backend formal directory");
    std::fs::write(&unexpected, "# 用户原有后端文档\n\n平台不得删除这个文件。")
        .expect("write original doc");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("pure frontend must reject backend formal outputs");

    assert!(error.contains("纯前端"));
    assert!(error.contains("docs/backend/latest/index.md"));
    assert_eq!(
        std::fs::read_to_string(&unexpected).expect("original doc survives"),
        "# 用户原有后端文档\n\n平台不得删除这个文件。"
    );
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3")
        .is_dir());
    assert!(!std::fs::read_to_string(root.join("CLAUDE.md"))
        .expect("read entry")
        .contains("vibe-coding-platform:init:"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_never_writes_marker_before_agent_link_validation_finishes() {
    let root = fixture_spring_project("existing-marker-before-link");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join("AGENTS.md"),
        "# 独立入口\n\n该文件尚未与 CLAUDE.md 合并，初始化必须先拒绝并保留现场。",
    )
    .expect("write conflicting agent entry");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("regular AGENTS.md must block finalization");

    assert!(error.contains("AGENTS.md"));
    assert!(root
        .join(".vibe-coding-platform/init-reference-v3")
        .is_dir());
    assert!(!std::fs::read_to_string(root.join("CLAUDE.md"))
        .expect("read entry")
        .contains("vibe-coding-platform:init:"));
    assert!(
        !existing_project_init_status(root.to_str().expect("valid path"))
            .expect("read status")
            .initialized
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[cfg(unix)]
#[test]
fn finalization_rejects_agent_links_that_point_to_the_wrong_maintenance_source() {
    use std::os::unix::fs::symlink;

    let root = fixture_spring_project("existing-wrong-links");
    write_real_backend_assets(&root);
    std::fs::create_dir_all(root.join(".agents")).expect("create agents dir");
    symlink("../docs", root.join(".agents/rules")).expect("create wrong rules link");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("wrong shared maintenance target must block finalization");
    assert!(error.contains(".agents/rules"));
    assert!(
        !existing_project_init_status(root.to_str().expect("valid path"))
            .expect("status")
            .initialized
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_rejects_generic_agent_entry_without_project_navigation() {
    let root = fixture_spring_project("existing-generic-entry");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join("CLAUDE.md"),
        "# AI 助手说明\n\n请认真开发并遵守最佳实践。这里有足够多的中文文字，但没有当前项目模块、文档、规则、技能、构建和测试导航。",
    )
    .expect("write generic entry");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("generic entry must not receive marker");
    assert!(error.contains("CLAUDE.md"));
    assert!(
        !existing_project_init_status(root.to_str().expect("valid path"))
            .expect("status")
            .initialized
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_rejects_backend_rules_without_real_source_path_and_symbol_evidence() {
    let root = fixture_spring_project("existing-generic-backend-rules");
    write_real_backend_assets(&root);
    for rule in [
        ".claude/rules/后端/API与业务实现规则.md",
        ".claude/rules/后端/持久化与迁移规则.md",
    ] {
        std::fs::write(
            root.join(rule),
            "# 后端通用约束\n\n所有实现以源码为准；未识别的模块后续再补。开发时遵守分层、复用、异常、日志、事务、兼容性与测试等最佳实践，先搜索再修改，完成构建与自测后交付。这里故意写得足够长，用来证明长度和中文数量不能代替当前项目的真实路径、类名与扩展点证据。",
        )
        .expect("replace backend rule with generic shell");
    }

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("generic backend rules must not finalize");

    assert!(error.contains("后端"));
    assert!(error.contains("通用约束") || error.contains("以源码为准"));
    assert!(!std::fs::read_to_string(root.join("CLAUDE.md"))
        .expect("read entry")
        .contains("vibe-coding-platform:init:"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_rejects_project_rules_that_name_no_existing_source_or_symbol() {
    let root = fixture_spring_project("existing-backend-rules-without-code-evidence");
    write_real_backend_assets(&root);
    for rule in [
        ".claude/rules/后端/API与业务实现规则.md",
        ".claude/rules/后端/持久化与迁移规则.md",
    ] {
        std::fs::write(
            root.join(rule),
            "# 后端项目规则\n\n本项目所有业务变化都必须先确认输入、输出、状态、事务和兼容性，再按现有分层完成最小改动。修改前检索同类实现，复用公共能力，保留错误与测试证据；修改后运行真实构建与自测，失败时如实报告，不放宽断言，也不编造成功结果。",
        )
        .expect("replace backend rule without code evidence");
    }

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("rules without a real path and symbol must not finalize");

    assert!(error.contains("真实源码路径"));
    assert!(error.contains("真实符号/类名"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn backend_rules_must_record_a_detected_nested_project_module() {
    let root = fixture_spring_project("existing-backend-module-evidence");
    write_real_backend_assets(&root);
    let module_source = root.join("payments/src/main/java/com/example/PaymentHandler.java");
    std::fs::create_dir_all(module_source.parent().expect("module source parent"))
        .expect("create nested module");
    std::fs::write(
        module_source,
        "package com.example;\n\npublic class PaymentHandler {\n    public void handle() {}\n}\n",
    )
    .expect("write nested module source");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("an unrecorded real module must block finalization");

    assert!(error.contains("项目模块"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn frontend_rules_must_cover_each_detected_router_store_and_api_client_category() {
    let root = fixture_frontend_project("existing-frontend-category-evidence");
    prepare_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("prepare frontend project");
    write_real_frontend_assets(&root);
    for (relative, content) in [
        (
            "src/router/index.ts",
            "import { createRouter, createWebHistory } from 'vue-router';\nexport const router = createRouter({ history: createWebHistory(), routes: [] });",
        ),
        (
            "src/stores/session.ts",
            "import { defineStore } from 'pinia';\nexport const useSessionStore = defineStore('session', () => ({}));",
        ),
        (
            "src/api/client.ts",
            "import axios from 'axios';\nexport const apiClient = axios.create({ baseURL: '/api' });",
        ),
    ] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("source parent"))
            .expect("create frontend evidence directory");
        std::fs::write(path, content).expect("write frontend category evidence");
    }

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("unrecorded frontend categories must block finalization");

    assert!(error.contains("前端"));
    assert!(error.contains("路由"));
    assert!(error.contains("状态管理"));
    assert!(error.contains("API client"));
    assert!(!std::fs::read_to_string(root.join("CLAUDE.md"))
        .expect("read entry")
        .contains("vibe-coding-platform:init:"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn frontend_finalization_accepts_real_component_path_and_symbol_evidence() {
    let root = fixture_frontend_project("existing-frontend-component-evidence");
    prepare_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("prepare frontend project");
    write_real_frontend_assets(&root);

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("real frontend component evidence must finalize");

    assert!(
        existing_project_init_status(root.to_str().expect("valid path"))
            .expect("read status")
            .initialized
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_rejects_long_english_only_documents() {
    let root = fixture_spring_project("existing-english-doc");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join("docs/backend/latest/业务/业务功能总览.md"),
        "# Business Overview\n\nThis document is intentionally long enough to pass the old length check. It describes controllers, services, repositories, data ownership, compatibility, testing, errors, and existing behavior, but it is not a Chinese project document and therefore must not finalize successfully.",
    )
    .expect("write English document");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("English-only user documentation must not finalize");
    assert!(error.contains("中文"));
    assert!(
        !existing_project_init_status(root.to_str().expect("valid path"))
            .expect("read status")
            .initialized
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn backend_without_route_evidence_does_not_require_an_api_document() {
    let root = fixture_spring_project("existing-no-route");
    write_real_backend_assets(&root);
    std::fs::remove_file(root.join("docs/backend/latest/接口文档/API接口总览.md"))
        .expect("remove API document");

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("backend without routes must not receive an empty API document");
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn database_dependency_without_model_or_migration_does_not_require_a_physical_model() {
    let root = fixture_spring_project("existing-db-dependency-only");
    write_real_backend_assets(&root);
    std::fs::remove_file(root.join("docs/backend/latest/接口文档/物理模型总览.md"))
        .expect("remove physical model");

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("dependency alone must not invent physical tables");
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn database_model_evidence_requires_a_project_specific_ddl_review_skill() {
    let root = fixture_spring_project("existing-db-skill");
    write_real_backend_assets(&root);
    std::fs::create_dir_all(root.join("migrations")).expect("create migrations");
    std::fs::write(
        root.join("migrations/V1__sample_order.sql"),
        "create table sample_order (id bigint not null primary key);",
    )
    .expect("write migration");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("database evidence must require the database review workflow");
    assert!(error.contains("ddl-review"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn every_backend_requires_a_project_specific_log_diagnose_skill() {
    let root = fixture_spring_project("existing-log-skill");
    write_real_backend_assets(&root);
    std::fs::remove_file(root.join(".claude/skills/backend-log-diagnose/SKILL.md"))
        .expect("remove backend log diagnosis skill");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("every backend must provide the log diagnosis workflow");
    assert!(error.contains("backend-log-diagnose"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn callback_route_evidence_requires_a_real_callback_document() {
    let root = fixture_spring_project("existing-callback-doc");
    write_real_backend_assets(&root);
    let source = root.join("src/main/java/PaymentCallbackController.java");
    std::fs::write(
        source,
        "@RestController class PaymentCallbackController { @PostMapping(\"/payment/callback\") void callback() {} }",
    )
    .expect("write callback controller");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("real callback routes must be documented");
    assert!(error.contains("回调接口总览"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn frontend_route_syntax_does_not_invent_a_backend_api_document() {
    let root = fixture_spring_project("existing-frontend-route-only");
    write_real_backend_assets(&root);
    std::fs::remove_file(root.join("docs/backend/latest/接口文档/API接口总览.md"))
        .expect("remove API document");
    std::fs::create_dir_all(root.join("src/frontend")).expect("create frontend source");
    std::fs::write(
        root.join("src/frontend/router.ts"),
        "export const detailRoute = router.route('/orders/:id')",
    )
    .expect("write frontend route");

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("frontend route syntax is not server API evidence");
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn outbound_notify_url_does_not_invent_a_callback_document() {
    let root = fixture_spring_project("existing-outbound-notify-url");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join("src/main/java/OrderController.java"),
        "@RestController class OrderController { @GetMapping(\"/orders\") void list() {} }",
    )
    .expect("write ordinary controller");
    std::fs::write(
        root.join("src/main/java/ProviderRequest.java"),
        "class ProviderRequest { String notify_url; }",
    )
    .expect("write outbound notification field");

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("an outbound notification URL is not an inbound callback route");
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn boundary_enum_evidence_requires_a_real_enum_overview() {
    let root = fixture_spring_project("existing-enum-doc");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join("src/main/java/OrderStatus.java"),
        "public enum OrderStatus { CREATED, SUCCESS, FAILED }",
    )
    .expect("write business enum");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("business enums must be documented");
    assert!(error.contains("枚举值总览"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn external_client_evidence_requires_document_rule_and_skill() {
    let root = fixture_spring_project("existing-external-integration");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join("src/main/java/ProviderClient.java"),
        "@FeignClient(name = \"provider\", url = \"${provider.url}\") interface ProviderClient {}",
    )
    .expect("write external client");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("external integrations must be documented before finalization");
    assert!(error.contains("第三方集成"));

    let document = root.join("docs/backend/latest/第三方集成/第三方集成总览.md");
    std::fs::create_dir_all(document.parent().expect("third-party document parent"))
        .expect("create third-party document directory");
    std::fs::write(
        document,
        "# 第三方集成总览\n\n当前项目通过 `ProviderClient` 调用合作方接口，地址来自 `provider.url` 配置。请求、响应、错误、超时与重试均以真实 Feign 契约为准，不伪造默认成功。",
    )
    .expect("write third-party document");
    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("external integrations must install the matching rule");
    assert!(error.contains("异步与第三方规则"));

    std::fs::write(
        root.join(".claude/rules/后端/异步与第三方规则.md"),
        "# 异步与第三方规则\n\n当前项目使用 ProviderClient 调用合作方。修改前必须读取真实 Feign 契约、配置键和错误处理，保持幂等、超时、重试、签名验签和敏感信息边界，不添加伪成功兜底。",
    )
    .expect("write external integration rule");
    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("external integrations must install the matching skill");
    assert!(error.contains("external-integration"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn database_connection_configuration_requires_a_read_only_diagnosis_skill() {
    let root = fixture_spring_project("existing-database-diagnose");
    write_real_backend_assets(&root);
    std::fs::create_dir_all(root.join("src/main/resources")).expect("create resources");
    std::fs::write(
        root.join("src/main/resources/application.yml"),
        "spring:\n  datasource:\n    url: ${DB_URL}\n    username: ${DB_USERNAME}\n    password: ${DB_PASSWORD}\n",
    )
    .expect("write database configuration evidence");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("database configuration must produce a safe read-only diagnosis skill");
    assert!(error.contains("database-read-diagnose"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn empty_migrations_directory_does_not_invent_a_physical_model() {
    let root = fixture_spring_project("existing-empty-migrations");
    write_real_backend_assets(&root);
    std::fs::create_dir_all(root.join("migrations")).expect("create empty migrations");
    std::fs::remove_file(root.join("docs/backend/latest/接口文档/物理模型总览.md"))
        .expect("remove physical model");

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("an empty migrations folder is not schema evidence");
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn liquibase_create_table_evidence_requires_a_physical_model() {
    let root = fixture_spring_project("existing-liquibase-model");
    write_real_backend_assets(&root);
    std::fs::remove_file(root.join("docs/backend/latest/接口文档/物理模型总览.md"))
        .expect("remove physical model");
    std::fs::create_dir_all(root.join("src/main/resources/db/changelog"))
        .expect("create changelog directory");
    std::fs::write(
        root.join("src/main/resources/db/changelog/order.xml"),
        "<databaseChangeLog><changeSet id=\"1\"><createTable tableName=\"orders\"/></changeSet></databaseChangeLog>",
    )
    .expect("write liquibase schema");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("Liquibase table definitions are physical model evidence");
    assert!(
        error.contains("物理模型总览"),
        "unexpected validation error: {error}"
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_rejects_generic_skill_shells_even_when_the_text_is_long() {
    let root = fixture_spring_project("existing-generic-skill");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join(".claude/skills/developer/SKILL.md"),
        "---\nname: developer\ndescription: Use for development.\n---\n\n这是一个很长但没有项目资源、执行流程、完成 Gate 和失败处理的泛化说明。它重复很多文字以绕过旧长度校验，但没有任何真实路径、命令、框架、公共能力或同类实现证据。",
    )
    .expect("write generic skill");

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("generic skill must not finalize");

    assert!(error.contains("developer"));
    assert!(error.contains("skill"));
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn non_git_project_does_not_require_git_or_worktree_assets() {
    let root = fixture_spring_project("existing-no-git");
    write_real_backend_assets(&root);

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("non-git project can finalize without worktree assets");

    assert!(!root.join(".claude/skills/worktree").exists());
    assert!(!root
        .join(".claude/rules/公共/Git协作与历史保护.md")
        .exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn git_project_does_not_require_an_initialization_worktree_skill() {
    let root = fixture_spring_project("existing-git-without-worktree-skill");
    std::fs::create_dir_all(root.join(".git")).expect("create git marker");
    write_real_backend_assets(&root);
    std::fs::write(
        root.join(".claude/rules/公共/Git协作与历史保护.md"),
        "# Git 协作与历史保护\n\n提交和推送由用户选择；不覆盖已有提交，不删除未知分支、stash 或未提交文件。",
    )
    .expect("write git collaboration rule");

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("worktree is a product capability, not a required initialization skill");

    assert!(!root.join(".claude/skills/worktree").exists());
    assert!(root
        .join(".claude/rules/公共/Git协作与历史保护.md")
        .is_file());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn finalization_accepts_project_rules_in_nested_rule_directories() {
    let root = fixture_spring_project("existing-finalize-nested-rules");
    write_real_backend_assets(&root);

    finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect("nested real rules must finalize");

    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rust_database_dependencies_require_a_real_physical_model_document() {
    let root = std::env::temp_dir().join(format!("vibe-existing-rust-db-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("create rust source");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n[dependencies]\nsqlx = { version = \"0.8\", features = [\"mysql\"] }\n",
    )
    .expect("write cargo manifest");
    std::fs::create_dir_all(root.join("migrations")).expect("create migrations");
    std::fs::write(
        root.join("migrations/V1__sample.sql"),
        "create table sample (id bigint not null primary key);",
    )
    .expect("write migration evidence");
    for (relative, content) in [
        ("docs/backend/MOC.md", "# Rust 数据服务文档导航\n\n长期文档与版本记录均从本页进入。"),
        ("docs/backend/latest/index.md", "# Rust 数据服务项目文档索引\n\n该项目使用 Rust 与 SQLx 处理数据访问，并由 Cargo 管理依赖与构建。"),
        ("docs/backend/latest/业务/业务功能总览.md", "# 业务功能总览\n\n当前样例仅提供数据访问服务骨架，业务能力需要以 `src` 下代码为证据继续补充。"),
        ("docs/backend/latest/系统架构/系统架构详解.md", "# 系统架构\n\nRust 应用由 Cargo 构建，源码位于 `src`，数据访问依赖在 Cargo.toml 中声明。"),
        ("docs/backend/latest/接口文档/API接口总览.md", "# API 接口总览\n\n当前项目未提供 HTTP 路由实现，因此没有可确认的对外接口；结论来自 `src` 扫描。"),
        ("docs/backend/latest/规范约束/详设文档模板.md", "# 详设文档模板\n\n记录需求、现状、设计、实现和验证，每次使用必须结合真实源码。"),
        ("docs/backend/latest/规范约束/开发进度文档模板.md", "# 开发进度模板\n\n记录清单、TDD、自测证据、用户反馈和文档同步。"),
        (".claude/rules/开发约束.md", "# Rust 开发约束\n\n修改前读取 Cargo.toml 与相关模块，优先复用现有错误处理、日志和测试组织方式。"),
        (".claude/skills/developer/SKILL.md", "---\nname: developer\ndescription: Use when实现或修复 Rust 后端代码。\n---\n\n# Rust 开发\n\n实现前先读取模块边界与现有测试，变更后使用 Cargo 的真实命令验证。"),
        (".claude/skills/skill-designer/SKILL.md", "---\nname: skill-designer\ndescription: Use when创建或修改项目技能。\n---\n\n# Skill Designer\n\n新技能必须先选择正确模式，保持工作流可执行并设置检查点。"),
    ] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(path, content).expect("write asset");
    }

    let error = finalize_existing_project_initialization(root.to_str().expect("valid path"))
        .expect_err("Cargo database evidence must require a physical model document");
    assert!(
        error.contains("物理模型总览"),
        "unexpected validation error: {error}"
    );

    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn agent_analysis_prompt_requires_project_skills_and_read_only_json() {
    let prompt = build_analysis_prompt(&AnalyzeProjectRequest {
        text: "做一个订单管理后台，有权限、订单与长期维护".to_string(),
        project_name: None,
        structure_preference: Some("frontend-backend".to_string()),
        clarification_answers: vec![],
    });

    assert!(prompt.contains("vibe-tech-stack-selection/SKILL.md"));
    assert!(prompt.contains("software-architect/SKILL.md"));
    assert!(prompt.contains("不得修改文件"));
    assert!(prompt.contains("只输出 JSON"));
    assert!(prompt.contains("生成一个简洁、合法的 kebab-case 项目名"));
    assert!(prompt.contains("项目结构偏好：frontend-backend"));
    assert!(prompt.contains("用户澄清答案（JSON）：[]"));
}

#[test]
fn agent_analysis_prompt_limits_dynamic_questions_and_requires_recommended_answers() {
    let prompt = build_analysis_prompt(&AnalyzeProjectRequest {
        text: "宠物预约美容，会售卖商品，走微信小程序".to_string(),
        project_name: None,
        structure_preference: Some("auto".to_string()),
        clarification_answers: vec![],
    });

    assert!(prompt.contains("最多 10 项"));
    assert!(prompt.contains("selectionMode"));
    assert!(prompt.contains("推荐答案"));
    assert!(prompt.contains("不得重复询问"));
}

#[test]
fn agent_analysis_prompt_uses_business_language_and_keeps_exclusive_choices_single() {
    let prompt = build_analysis_prompt(&AnalyzeProjectRequest {
        text: "宠物预约美容，会售卖商品，走微信小程序".to_string(),
        project_name: None,
        structure_preference: Some("auto".to_string()),
        clarification_answers: vec![],
    });

    assert!(prompt.contains("非技术用户"));
    assert!(prompt.contains("不得让用户在 Java、TypeScript、MySQL、PostgreSQL"));
    assert!(prompt.contains("互斥的备选项必须使用 single"));
}

#[test]
fn spring_initializr_dependencies_follow_adopted_data_and_messaging_choices() {
    let dependencies = spring_initializr_dependencies(&StackRecommendationPayload {
        id: "vue-spring-boot".to_string(),
        title: "Vue 3 + Spring Boot 3".to_string(),
        frontend: vec!["Vue 3".to_string()],
        backend: vec!["Java 21".to_string(), "Spring Boot 3".to_string()],
        database: vec!["MySQL".to_string()],
        cache: vec!["Redis".to_string()],
        messaging: vec!["RabbitMQ".to_string()],
        decisions: vec![],
        structure: "frontend-backend".to_string(),
        ..Default::default()
    });

    assert!(dependencies.contains(&"web".to_string()));
    assert!(dependencies.contains(&"actuator".to_string()));
    assert!(dependencies.contains(&"data-jpa".to_string()));
    assert!(dependencies.contains(&"mysql".to_string()));
    assert!(dependencies.contains(&"data-redis".to_string()));
    assert!(dependencies.contains(&"amqp".to_string()));
    assert!(!dependencies.contains(&"postgresql".to_string()));
}

#[test]
fn create_request_deserializes_confirmed_requirement_and_complete_recommendation_snapshot() {
    let request: CreateProjectRequest = serde_json::from_value(serde_json::json!({
        "projectName": "order-admin",
        "parentPath": "/tmp",
        "conciseRequirement": "产品形态：订单管理后台；主要用户：运营人员",
        "recognizedConstraints": [
            { "id": "product", "label": "产品形态", "value": "订单管理后台" }
        ],
        "assumptions": ["首期仅支持内部员工"],
        "recommendation": {
            "id": "vue-spring-boot",
            "title": "Vue 3 + Spring Boot",
            "frontend": ["Vue 3"],
            "backend": ["Spring Boot"],
            "database": ["MySQL"],
            "cache": [],
            "messaging": [],
            "decisions": [],
            "structure": "frontend-backend",
            "packageManager": "maven",
            "reasons": ["适合订单与权限业务"],
            "tradeoffs": ["维护两个工程"],
            "preferenceMatched": true
        },
        "profile": { "summary": "订单管理后台", "systemType": "admin" },
        "agentChoice": "both"
    }))
    .expect("deserialize confirmed creation snapshot");

    assert_eq!(
        request.concise_requirement,
        "产品形态：订单管理后台；主要用户：运营人员"
    );
    assert_eq!(request.recognized_constraints[0].value, "订单管理后台");
    assert_eq!(request.assumptions, ["首期仅支持内部员工"]);
    assert_eq!(
        request.recommendation.package_manager.as_deref(),
        Some("maven")
    );
    assert_eq!(request.recommendation.reasons, ["适合订单与权限业务"]);
    assert_eq!(request.recommendation.tradeoffs, ["维护两个工程"]);
    assert!(request.recommendation.preference_matched);
}

#[test]
#[ignore = "real integration test: invokes the locally authenticated Codex or Claude Code CLI"]
fn agent_analysis_returns_a_supported_project_template() {
    let result = analyze_with_agent(&AnalyzeProjectRequest {
        text: "做一个对外品牌官网，需要 SEO、移动端访问和后续内容更新".to_string(),
        project_name: None,
        structure_preference: Some("single-app".to_string()),
        clarification_answers: vec![],
    })
    .expect("logged-in agent CLI must return an analysis");

    assert_eq!(result.provider, "codex");
    assert_eq!(result.recommended.id, "nextjs");
    assert!(!result.recommended.reasons.is_empty());
    assert!(!result.recommended.tradeoffs.is_empty());
}

#[test]
fn windows_node_install_uses_winget() {
    let command = install_command_for("node", "windows").expect("known tool");
    assert_eq!(command.program, "winget");
    assert!(command.args.contains(&"OpenJS.NodeJS.LTS".to_string()));
}

#[test]
fn unknown_tool_has_no_install_command() {
    assert!(install_command_for("arbitrary-shell-command", "macos").is_err());
}

#[test]
fn checks_installed_node_version() {
    let items = check_environment(&["node".to_string()]).expect("known tool");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].tool_id, "node");
    assert!(items[0].installed, "test runtime requires Node.js");
    assert!(items[0]
        .version
        .as_deref()
        .unwrap_or_default()
        .starts_with('v'));
}

#[test]
fn target_path_uses_project_name_below_selected_parent() {
    let target = preview_target_path("/tmp/vibe-projects", "order-admin").expect("valid path");
    assert_eq!(target.to_string_lossy(), "/tmp/vibe-projects/order-admin");
}

#[test]
fn non_empty_target_directory_is_rejected() {
    let root =
        std::env::temp_dir().join(format!("vibe-project-factory-test-{}", std::process::id()));
    let target = root.join("existing-project");
    std::fs::create_dir_all(&target).expect("create target");
    std::fs::write(target.join("keep.txt"), "keep").expect("seed target");

    let result = validate_target_dir(root.to_str().unwrap(), "existing-project");
    assert!(result.is_err());

    std::fs::remove_dir_all(root).expect("cleanup target");
}

#[test]
fn creates_a_runnable_web_skeleton_with_shared_agent_rules() {
    let root = std::env::temp_dir().join(format!(
        "vibe-project-factory-create-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create parent");
    let result = create_project(&CreateProjectRequest {
        project_name: "demo-web".to_string(),
        parent_path: root.to_string_lossy().to_string(),
        frontend_project_name: None,
        backend_project_name: None,
        recommendation: StackRecommendationPayload {
            id: "vue-vite".to_string(),
            title: "Vue 3 + Vite".to_string(),
            frontend: vec!["Vue 3".to_string()],
            backend: vec![],
            database: vec![],
            cache: vec![],
            messaging: vec![],
            decisions: vec![],
            structure: "single-app".to_string(),
            ..Default::default()
        },
        profile: ProjectProfilePayload {
            summary: "做一个可启动的产品官网".to_string(),
            system_type: "web-h5".to_string(),
        },
        agent_choice: "both".to_string(),
        ..Default::default()
    })
    .expect("create project");
    let project = std::path::Path::new(&result.project_paths[0]);

    assert!(project.join("package.json").is_file());
    assert!(project.join("README.md").is_file());
    assert!(project.join("docs/frontend/MOC.md").is_file());
    assert!(project.join("docs/frontend/latest/index.md").is_file());
    assert!(project
        .join("docs/frontend/latest/业务/业务功能总览.md")
        .is_file());
    assert!(project
        .join("docs/frontend/latest/系统架构/前端架构.md")
        .is_file());
    assert!(project
        .join("docs/frontend/latest/公共能力/组件与公共能力.md")
        .is_file());
    assert!(project
        .join("docs/frontend/latest/规范约束/详设文档模板.md")
        .is_file());
    assert!(project.join("docs/项目需求与技术选型.md").is_file());
    assert!(!project.join("docs/frontend/v0.1").exists());
    assert!(!project.join("docs/backend").exists());
    assert!(!project.join("docs/operations").exists());
    assert!(!project.join("docs/规范约束").exists());
    assert!(project.join("CLAUDE.md").is_file());
    assert!(project.join("AGENTS.md").is_file());
    assert!(project.join(".agents/rules").exists());
    assert!(project.join(".claude/rules/公共/开发基线.md").is_file());
    assert!(project
        .join(".claude/rules/公共/Git协作与历史保护.md")
        .is_file());
    assert!(project.join(".claude/skills/developer/SKILL.md").is_file());
    assert!(project
        .join(".claude/skills/skill-designer/SKILL.md")
        .is_file());
    assert!(project
        .join(".claude/skills/skill-designer/references/decision-tree.md")
        .is_file());
    assert_eq!(
        relative_file_bytes(&project.join(".claude/skills/skill-designer")),
        relative_file_bytes(&platform_root().join("docs/规范约束/技能模板/公共/skill-designer")),
        "created projects must receive every platform skill-designer template file byte for byte"
    );
    assert!(project
        .join(".claude/skills/frontend-self-test/SKILL.md")
        .is_file());
    assert!(!project.join(".claude/skills/worktree").exists());
    assert!(!project
        .join(".claude/skills/backend-self-test/SKILL.md")
        .exists());
    #[cfg(unix)]
    assert_eq!(
        std::fs::read_link(project.join("AGENTS.md")).expect("AGENTS must be a symlink"),
        std::path::PathBuf::from("CLAUDE.md")
    );
    for path in [
        "README.md",
        "CLAUDE.md",
        ".claude/rules/公共/开发基线.md",
        ".claude/skills/developer/SKILL.md",
        "docs/项目需求与技术选型.md",
        "docs/frontend/latest/index.md",
    ] {
        let content = std::fs::read_to_string(project.join(path)).expect("read generated asset");
        assert!(!content.contains("{{"), "unfilled placeholder in {path}");
        assert!(!content.contains("待填写"), "empty template in {path}");
    }
    let readme = std::fs::read_to_string(project.join("README.md")).expect("read project README");
    let entry = std::fs::read_to_string(project.join("CLAUDE.md")).expect("read project entry");
    assert!(readme.contains("不会自动开发业务功能"));
    assert!(entry.contains("项目工厂到此结束，不自动开发任何业务功能"));
    assert!(entry.contains("用户明确要求开发后才使用 `developer`"));

    std::fs::remove_dir_all(root).expect("cleanup project");
}

#[test]
fn backend_skeleton_gets_project_specific_backend_docs_but_no_ops_or_frontend_assets() {
    let root = std::env::temp_dir().join(format!(
        "vibe-project-factory-backend-docs-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create parent");
    let result = create_project(&CreateProjectRequest {
        project_name: "catalog-api".to_string(),
        parent_path: root.to_string_lossy().to_string(),
        frontend_project_name: None,
        backend_project_name: None,
        recommendation: StackRecommendationPayload {
            id: "fastapi-api".to_string(),
            title: "FastAPI".to_string(),
            frontend: vec![],
            backend: vec!["FastAPI".to_string()],
            database: vec![],
            cache: vec![],
            messaging: vec![],
            decisions: vec![],
            structure: "single-app".to_string(),
            ..Default::default()
        },
        profile: ProjectProfilePayload {
            summary: "商品目录 API".to_string(),
            system_type: "backend-api".to_string(),
        },
        agent_choice: "both".to_string(),
        ..Default::default()
    })
    .expect("create backend project");
    let project = std::path::Path::new(&result.project_paths[0]);

    assert!(project
        .join("docs/backend/latest/系统架构/系统架构详解.md")
        .is_file());
    assert!(project
        .join("docs/backend/latest/接口文档/API接口总览.md")
        .is_file());
    assert!(project.join("docs/backend/MOC.md").is_file());
    assert!(project
        .join("docs/backend/latest/规范约束/详设文档模板.md")
        .is_file());
    assert!(project
        .join(".claude/skills/backend-self-test/SKILL.md")
        .is_file());
    assert!(project
        .join(".claude/skills/backend-log-diagnose/SKILL.md")
        .is_file());
    assert!(!project.join("docs/operations").exists());
    assert!(!project.join("docs/规范约束").exists());
    assert!(!project.join("docs/frontend").exists());
    assert!(!project.join(".claude/skills/frontend-self-test").exists());

    std::fs::remove_dir_all(root).expect("cleanup project");
}

#[test]
fn backend_external_integration_gets_matching_skill_and_rule_without_messaging() {
    let root = std::env::temp_dir().join(format!(
        "vibe-project-factory-external-integration-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create parent");
    let result = create_project(&CreateProjectRequest {
        project_name: "partner-api".to_string(),
        parent_path: root.to_string_lossy().to_string(),
        frontend_project_name: None,
        backend_project_name: None,
        recommendation: StackRecommendationPayload {
            id: "fastapi-api".to_string(),
            title: "FastAPI + 第三方接口".to_string(),
            frontend: vec![],
            backend: vec!["FastAPI".to_string()],
            database: vec![],
            cache: vec![],
            messaging: vec![],
            decisions: vec![
                vibe_coding_platform_lib::project_factory::TechnologyDecision {
                    category: "integration".to_string(),
                    title: "第三方接口".to_string(),
                    status: "adopt".to_string(),
                    choices: vec!["合作方 REST API".to_string()],
                    reason: "业务需要调用外部合作方".to_string(),
                    provision: "project".to_string(),
                    trigger: None,
                },
            ],
            structure: "single-app".to_string(),
            ..Default::default()
        },
        profile: ProjectProfilePayload {
            summary: "接入合作方 REST API".to_string(),
            system_type: "backend-api".to_string(),
        },
        agent_choice: "both".to_string(),
        ..Default::default()
    })
    .expect("create backend integration project");
    let project = std::path::Path::new(&result.project_paths[0]);

    assert!(project
        .join(".claude/skills/external-integration/SKILL.md")
        .is_file());
    assert!(project
        .join(".claude/rules/后端/异步与第三方规则.md")
        .is_file());

    std::fs::remove_dir_all(root).expect("cleanup project");
}

#[test]
fn creates_two_independent_spring_boot_and_vue_projects() {
    let root =
        std::env::temp_dir().join(format!("vibe-project-factory-java-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create parent");
    let result = create_project(&CreateProjectRequest {
        project_name: "order-admin".to_string(),
        parent_path: root.to_string_lossy().to_string(),
        frontend_project_name: Some("order-console".to_string()),
        backend_project_name: Some("order-service".to_string()),
        recommendation: StackRecommendationPayload {
            id: "vue-spring-boot".to_string(),
            title: "Vue 3 + Spring Boot 3".to_string(),
            frontend: vec![
                "Vue 3".to_string(),
                "TypeScript".to_string(),
                "Vite".to_string(),
            ],
            backend: vec![
                "Spring Boot 3".to_string(),
                "Java 21".to_string(),
                "MyBatis-Plus".to_string(),
            ],
            database: vec!["MySQL 8".to_string()],
            cache: vec!["Redis".to_string()],
            messaging: vec![],
            decisions: vec![],
            structure: "frontend-backend".to_string(),
            ..Default::default()
        },
        profile: ProjectProfilePayload {
            summary: "订单管理后台".to_string(),
            system_type: "admin".to_string(),
        },
        agent_choice: "both".to_string(),
        ..Default::default()
    })
    .expect("create project");
    assert_eq!(result.project_paths.len(), 2);
    let frontend = std::path::Path::new(&result.project_paths[0]);
    let backend = std::path::Path::new(&result.project_paths[1]);

    assert_eq!(
        frontend.file_name().and_then(|name| name.to_str()),
        Some("order-console")
    );
    assert_eq!(
        backend.file_name().and_then(|name| name.to_str()),
        Some("order-service")
    );
    assert!(frontend.join("src/App.vue").is_file());
    assert!(frontend
        .join("docs/frontend/latest/系统架构/前端架构.md")
        .is_file());
    assert!(frontend.join("CLAUDE.md").is_file());
    assert!(frontend.join("AGENTS.md").is_file());
    assert!(backend.join("pom.xml").is_file());
    let pom = std::fs::read_to_string(backend.join("pom.xml")).expect("read generated pom");
    assert!(pom.contains("spring-boot-starter-data-jpa"));
    assert!(pom.contains("mysql-connector-j"));
    assert!(pom.contains("spring-boot-starter-data-redis"));
    assert!(backend
        .join("src/main/resources/application-database.yml")
        .is_file());
    assert!(backend
        .join("src/main/java/com/vibe/orderservice/OrderServiceApplication.java")
        .is_file());
    assert!(backend.join("src/main/resources/application.yml").is_file());
    assert!(backend
        .join("docs/backend/latest/系统架构/系统架构详解.md")
        .is_file());
    assert!(backend.join("docs/backend/latest/index.md").is_file());
    assert!(backend.join("CLAUDE.md").is_file());
    assert!(backend.join("AGENTS.md").is_file());
    let selection = std::fs::read_to_string(backend.join("docs/项目需求与技术选型.md"))
        .expect("read confirmed selection");
    assert!(selection.contains("订单管理后台"));
    assert!(selection.contains("MySQL 8"));
    let readme = std::fs::read_to_string(backend.join("README.md")).expect("read readme");
    assert!(readme.contains("mvn spring-boot:run"));
    assert!(!root.join("order-admin").exists());

    std::fs::remove_dir_all(root).expect("cleanup project");
}

#[test]
fn creates_backend_skeletons_for_supported_non_java_runtimes() {
    let root = std::env::temp_dir().join(format!(
        "vibe-project-factory-runtimes-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create parent");

    let cases = [
        (
            "fastapi-api",
            "python-api",
            "FastAPI",
            "pyproject.toml",
            "app/main.py",
        ),
        ("go-api", "go-api", "Go", "go.mod", "main.go"),
        ("axum-api", "rust-api", "Axum", "Cargo.toml", "src/main.rs"),
        (
            "aspnet-api",
            "dotnet-api",
            "ASP.NET Core",
            "dotnet-api.csproj",
            "Program.cs",
        ),
    ];

    for (template_id, project_name, runtime, manifest, entrypoint) in cases {
        let result = create_project(&CreateProjectRequest {
            project_name: project_name.to_string(),
            parent_path: root.to_string_lossy().to_string(),
            frontend_project_name: None,
            backend_project_name: None,
            recommendation: StackRecommendationPayload {
                id: template_id.to_string(),
                title: runtime.to_string(),
                frontend: vec![],
                backend: vec![runtime.to_string()],
                database: vec![],
                cache: vec![],
                messaging: vec![],
                decisions: vec![
                    vibe_coding_platform_lib::project_factory::TechnologyDecision {
                        category: "runtime".to_string(),
                        title: "后端运行时".to_string(),
                        status: "adopt".to_string(),
                        choices: vec![runtime.to_string()],
                        reason: "需求适配".to_string(),
                        provision: "project".to_string(),
                        trigger: None,
                    },
                ],
                structure: "single-app".to_string(),
                ..Default::default()
            },
            profile: ProjectProfilePayload {
                summary: format!("验证 {runtime} API 骨架"),
                system_type: "backend-api".to_string(),
            },
            agent_choice: "codex".to_string(),
            ..Default::default()
        })
        .expect("create supported runtime skeleton");

        assert_eq!(result.project_paths.len(), 1);
        let project = std::path::Path::new(&result.project_paths[0]);
        assert!(
            project.join(manifest).is_file(),
            "{template_id} should create its manifest"
        );
        assert!(
            project.join(entrypoint).is_file(),
            "{template_id} should create its entrypoint"
        );
        let selection = std::fs::read_to_string(project.join("docs/backend/latest/index.md"))
            .expect("read project index");
        assert!(selection.contains(runtime));
        assert!(selection.contains("技术栈"));
    }

    std::fs::remove_dir_all(root).expect("cleanup projects");
}

#[test]
#[ignore = "real integration test: invokes official generators, builds, starts, health-checks and stops a generated Java workspace"]
fn generated_spring_boot_workspace_passes_startup_verification() {
    let root = std::env::temp_dir().join(format!(
        "vibe-project-factory-java-build-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create parent");
    let result = create_project_with_verification(&CreateProjectRequest {
        project_name: "demo-java".to_string(),
        parent_path: root.to_string_lossy().to_string(),
        frontend_project_name: None,
        backend_project_name: None,
        recommendation: StackRecommendationPayload {
            id: "vue-spring-boot".to_string(),
            title: "Vue 3 + Spring Boot 3".to_string(),
            frontend: vec![
                "Vue 3".to_string(),
                "TypeScript".to_string(),
                "Vite".to_string(),
            ],
            backend: vec!["Spring Boot 3".to_string(), "Java 21".to_string()],
            database: vec![],
            cache: vec![],
            messaging: vec![],
            decisions: vec![],
            structure: "frontend-backend".to_string(),
            ..Default::default()
        },
        profile: ProjectProfilePayload {
            summary: "验证 Java 全栈骨架".to_string(),
            system_type: "admin".to_string(),
        },
        agent_choice: "both".to_string(),
        ..Default::default()
    })
    .expect("create project");
    assert_eq!(
        result.verification.status, "passed",
        "{}",
        result.verification.detail
    );
    assert_eq!(result.project_paths.len(), 2);
    assert!(std::path::Path::new(&result.project_paths[0])
        .join("package.json")
        .is_file());
    assert!(std::path::Path::new(&result.project_paths[1])
        .join("mvnw")
        .is_file());

    std::fs::remove_dir_all(root).expect("cleanup project");
}

#[test]
#[ignore = "manual integration test: installs generated npm dependencies and builds the skeleton"]
fn generated_web_skeleton_builds() {
    let root =
        std::env::temp_dir().join(format!("vibe-project-factory-build-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create parent");
    let result = create_project(&CreateProjectRequest {
        project_name: "demo-build".to_string(),
        parent_path: root.to_string_lossy().to_string(),
        frontend_project_name: None,
        backend_project_name: None,
        recommendation: StackRecommendationPayload {
            id: "vue-vite".to_string(),
            title: "Vue 3 + Vite".to_string(),
            frontend: vec!["Vue 3".to_string()],
            backend: vec![],
            database: vec![],
            cache: vec![],
            messaging: vec![],
            decisions: vec![],
            structure: "single-app".to_string(),
            ..Default::default()
        },
        profile: ProjectProfilePayload {
            summary: "验证可构建的项目骨架".to_string(),
            system_type: "web-h5".to_string(),
        },
        agent_choice: "claude".to_string(),
        ..Default::default()
    })
    .expect("create project");
    let project = std::path::Path::new(&result.project_paths[0]);

    let install = std::process::Command::new("npm")
        .args(["install", "--ignore-scripts", "--no-audit", "--no-fund"])
        .current_dir(project)
        .status()
        .expect("run npm install");
    assert!(
        install.success(),
        "generated project dependencies must install"
    );

    let build = std::process::Command::new("npm")
        .args(["run", "build"])
        .current_dir(project)
        .status()
        .expect("run generated project build");
    assert!(build.success(), "generated project must build");

    std::fs::remove_dir_all(root).expect("cleanup project");
}

#[test]
#[ignore = "real integration test: generates, builds, starts, health-checks and removes every supported template"]
fn every_supported_template_passes_startup_verification() {
    let root = std::env::temp_dir().join(format!(
        "vibe-project-factory-all-templates-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create parent");
    let cases = [
        ("vue-vite", "vue-web", "single-app"),
        ("vue-spring-boot", "spring-web", "frontend-backend"),
        ("node-nestjs", "nest-web", "frontend-backend"),
        ("vue-fastapi", "fastapi-web", "frontend-backend"),
        ("vue-go", "go-web", "frontend-backend"),
        ("vue-axum", "axum-web", "frontend-backend"),
        ("vue-aspnet", "dotnet-web", "frontend-backend"),
        ("fastapi-api", "fastapi-api", "single-app"),
        ("go-api", "go-api", "single-app"),
        ("axum-api", "axum-api", "single-app"),
        ("aspnet-api", "dotnet-api", "single-app"),
        ("nextjs", "next-web", "single-app"),
        ("tauri-vue", "desktop-app", "single-app"),
    ];

    for (template_id, project_name, structure) in cases {
        let result = create_project_with_verification(&CreateProjectRequest {
            project_name: project_name.to_string(),
            parent_path: root.to_string_lossy().to_string(),
            frontend_project_name: None,
            backend_project_name: None,
            recommendation: StackRecommendationPayload {
                id: template_id.to_string(),
                title: template_id.to_string(),
                frontend: if structure == "frontend-backend"
                    || template_id == "vue-vite"
                    || template_id == "tauri-vue"
                {
                    vec!["Vue 3".to_string()]
                } else {
                    vec![]
                },
                backend: vec![],
                database: vec![],
                cache: vec![],
                messaging: vec![],
                decisions: vec![],
                structure: structure.to_string(),
                ..Default::default()
            },
            profile: ProjectProfilePayload {
                summary: format!("验证 {template_id} 模板"),
                system_type: "verification".to_string(),
            },
            agent_choice: "codex".to_string(),
            ..Default::default()
        })
        .unwrap_or_else(|error| panic!("{template_id} 创建失败：{error}"));
        assert_eq!(
            result.verification.status, "passed",
            "{template_id} 自检失败：{}",
            result.verification.detail
        );
    }

    std::fs::remove_dir_all(root).expect("cleanup all generated projects");
}
