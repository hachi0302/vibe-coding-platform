use vibe_coding_platform_lib::project_factory::{
    analyze_with_agent, build_analysis_prompt, build_headless_initialization_prompt,
    build_v4_stage_prompt, check_environment, create_filtered_workspace, create_project,
    create_project_with_verification, existing_project_init_status,
    finalize_existing_project_initialization, inspect_project, install_command_for,
    prepare_existing_project_initialization, preview_target_path, read_artifact_plan,
    read_requirement_materials, spring_initializr_dependencies, validate_artifact_plan,
    validate_target_dir, AnalyzeProjectRequest, CreateProjectRequest, InitializationStage,
    ProjectProfilePayload, StackRecommendationPayload,
};

#[test]
fn headless_initialization_prompt_is_non_interactive_and_uses_internal_review() {
    let prompt =
        build_headless_initialization_prompt("项目路径：/tmp/demo\n按项目真实代码初始化。", None);

    for required in ["后台非会话任务", "不要询问用户", "完成前进行内部审核"] {
        assert!(
            prompt.contains(required),
            "missing prompt contract: {required}"
        );
    }
    assert!(!prompt.contains("请输出 WORKFLOW_CHECKPOINT"));
    assert!(!prompt.contains("在聊天框"));
    assert!(!prompt.contains("docs/backend/latest/接口文档/API接口总览.md"));
    assert!(!prompt.contains("skill-designer"));
}

#[test]
fn headless_initialization_review_prompt_includes_the_real_review_note() {
    let prompt = build_headless_initialization_prompt(
        "项目路径：/tmp/demo",
        Some("缺少初始化后的真实文档：docs/backend/latest/接口文档/API接口总览.md"),
    );

    assert!(prompt.contains("审核关注项"));
    assert!(prompt.contains("API接口总览.md"));
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
        "公共/doc-sync-review/SKILL.md",
        "公共/problem-diagnose/SKILL.md",
        "公共/review-feedback-handler/SKILL.md",
        "公共/find-skills/SKILL.md",
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
            "## 完成 Gate",
            "## 失败处理",
        ] {
            assert!(
                content.contains(required),
                "{relative} missing template contract: {required}"
            );
        }
        assert!(
            content.contains("## 执行流程") || content.contains("## 执行步骤"),
            "{relative} missing execution contract"
        );
    }
}

#[test]
fn v4_prepare_discovers_without_writing_and_finalize_does_not_recreate_v3_assets() {
    let root = std::env::temp_dir().join(format!("vibe-v4-read-only-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("create fixture");
    std::fs::write(root.join("package.json"), r#"{"dependencies":{"vue":"3"}}"#)
        .expect("write package");
    std::fs::write(root.join("src/App.vue"), "<template><main /></template>")
        .expect("write source");

    let before = std::fs::read_dir(&root).expect("before").count();
    let preparation = prepare_existing_project_initialization(&root.to_string_lossy())
        .expect("read-only preparation");
    let after = std::fs::read_dir(&root).expect("after").count();
    assert!(preparation.layers.frontend);
    assert_eq!(before, after);
    assert!(!root.join("docs").exists());
    assert!(!root.join(".claude").exists());
    assert!(!root.join(".agents").exists());

    let error = finalize_existing_project_initialization(&root.to_string_lossy())
        .expect_err("uninitialized project cannot be finalized");
    assert!(error.contains("current-v4"));
    assert!(!root.join("docs").exists());
    assert!(!root.join(".claude").exists());
    assert!(!root.join(".agents").exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn existing_v3_marker_is_classified_without_mutation() {
    let root = std::env::temp_dir().join(format!("vibe-v3-status-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create fixture");
    let entry = "# Existing\n\n<!-- vibe-coding-platform:init:v3 -->\n";
    std::fs::write(root.join("CLAUDE.md"), entry).expect("write v3 entry");

    let status = existing_project_init_status(&root.to_string_lossy()).expect("status");
    assert_eq!(status.status, "legacy-v3");
    assert_eq!(status.marker_version.as_deref(), Some("v3"));
    assert_eq!(
        std::fs::read_to_string(root.join("CLAUDE.md")).expect("read unchanged"),
        entry
    );
    assert!(!root.join("docs").exists());
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
#[ignore = "manual read-only diagnostic: prepares an isolated IAM workspace and validates its agent plan"]
fn iam_plan_only_diagnostic_uses_an_isolated_workspace() {
    let project = std::path::PathBuf::from(
        std::env::var("VCP_IAM_DRY_RUN_ROOT").expect("VCP_IAM_DRY_RUN_ROOT"),
    );
    let workspace = std::path::PathBuf::from(
        std::env::var("VCP_IAM_DRY_RUN_WORKSPACE").expect("VCP_IAM_DRY_RUN_WORKSPACE"),
    );
    let inventory = inspect_project(&project).expect("inspect IAM project");
    if !workspace.exists() {
        create_filtered_workspace(&project, &workspace, &inventory)
            .expect("create isolated IAM workspace");
    }
    let prompt = build_v4_stage_prompt(InitializationStage::Plan, &inventory, None, &[]);
    let control = workspace.join(".vibe-coding-platform");
    std::fs::create_dir_all(&control).expect("create diagnostic control directory");
    std::fs::write(control.join("plan-prompt.md"), prompt).expect("write diagnostic prompt");
    std::fs::write(
        control.join("inventory.json"),
        serde_json::to_vec_pretty(&inventory).expect("serialize inventory"),
    )
    .expect("write diagnostic inventory");

    let plan_path = control.join("artifact-plan.json");
    if plan_path.is_file() {
        let plan = read_artifact_plan(&workspace).expect("parse IAM artifact plan");
        let issues = validate_artifact_plan(&workspace, &inventory, &plan);
        if !issues.is_empty() {
            let repair_prompt =
                build_v4_stage_prompt(InitializationStage::Plan, &inventory, None, &issues);
            std::fs::write(control.join("repair-prompt.md"), repair_prompt)
                .expect("write diagnostic repair prompt");
        }
        assert!(issues.is_empty(), "IAM artifact plan issues: {issues:#?}");
        println!(
            "IAM_PLAN_OK modules={} source_roots={} files={} artifacts={}",
            inventory.modules.len(),
            inventory.source_roots.len(),
            inventory.files.len(),
            plan.artifacts.len()
        );
        for artifact in plan.artifacts {
            println!("IAM_ARTIFACT {}", artifact.target_path);
        }
    } else {
        println!(
            "IAM_PLAN_READY workspace={} prompt={}",
            workspace.display(),
            control.join("plan-prompt.md").display()
        );
    }
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
    assert!(project
        .join(".claude/rules/common/development-baseline.md")
        .is_file());
    assert!(project
        .join(".claude/rules/common/git-collaboration-and-history.md")
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
    for (path, target) in [
        (".agents/rules", "../.claude/rules"),
        (".agents/skills", "../.claude/skills"),
        (".agents/scripts", "../.claude/scripts"),
    ] {
        assert_eq!(
            std::fs::read_link(project.join(path))
                .unwrap_or_else(|_| panic!("{path} must be a symlink")),
            std::path::PathBuf::from(target)
        );
    }
    for path in [
        "README.md",
        "CLAUDE.md",
        ".claude/rules/common/development-baseline.md",
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
        .join(".claude/rules/backend/async-and-third-party.md")
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
