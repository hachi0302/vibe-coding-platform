use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::ai_rules::write_ai_rules;
use super::docs::write_project_docs;
use super::path_guard::validate_target_dir;
use super::types::{
    CreateProjectRequest, CreateProjectResult, ProjectVerificationResult,
    StackRecommendationPayload,
};

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, content).map_err(|error| error.to_string())
}

fn project_slug(project_name: &str) -> String {
    let slug: String = project_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect();
    if slug.is_empty() {
        "app".to_string()
    } else {
        slug
    }
}

fn pascal_case(project_name: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for character in project_name.chars() {
        if character.is_ascii_alphanumeric() {
            if capitalize {
                result.push(character.to_ascii_uppercase());
                capitalize = false;
            } else {
                result.push(character);
            }
        } else {
            capitalize = true;
        }
    }
    if result.is_empty() {
        "App".to_string()
    } else {
        result
    }
}

fn normalized_choices(recommendation: &StackRecommendationPayload) -> Vec<String> {
    let explicit_decision = |category: &str| {
        recommendation
            .decisions
            .iter()
            .any(|decision| decision.category == category)
    };
    let mut values = recommendation
        .frontend
        .iter()
        .chain(recommendation.backend.iter())
        .cloned()
        .collect::<Vec<_>>();
    if !explicit_decision("persistence") {
        values.extend(recommendation.database.iter().cloned());
    }
    if !explicit_decision("cache") {
        values.extend(recommendation.cache.iter().cloned());
    }
    if !explicit_decision("messaging") {
        values.extend(recommendation.messaging.iter().cloned());
    }
    values.extend(
        recommendation
            .decisions
            .iter()
            .filter(|decision| decision.status == "adopt")
            .flat_map(|decision| decision.choices.iter().cloned()),
    );
    values
        .into_iter()
        .map(|value| value.to_lowercase())
        .collect()
}

/// 将已确认的技术决策转换为 Spring Initializr 官方依赖标识。
/// 未在选型中采用的中间件不会被默认加入工程。
pub fn spring_initializr_dependencies(recommendation: &StackRecommendationPayload) -> Vec<String> {
    let choices = normalized_choices(recommendation);
    let has = |keyword: &str| choices.iter().any(|choice| choice.contains(keyword));
    let mut dependencies = vec!["web".to_string(), "actuator".to_string()];

    if has("mysql") {
        dependencies.extend(["data-jpa".to_string(), "mysql".to_string()]);
    } else if has("postgres") {
        dependencies.extend(["data-jpa".to_string(), "postgresql".to_string()]);
    } else if has("mongodb") {
        dependencies.push("data-mongodb".to_string());
    }
    if has("redis") {
        dependencies.push("data-redis".to_string());
    }
    if has("rabbitmq") {
        dependencies.push("amqp".to_string());
    }
    if has("kafka") {
        dependencies.push("kafka".to_string());
    }
    if has("spring security") {
        dependencies.push("security".to_string());
    }

    dependencies.sort();
    dependencies.dedup();
    dependencies
}

fn has_relational_database(recommendation: &StackRecommendationPayload) -> bool {
    let choices = normalized_choices(recommendation);
    choices
        .iter()
        .any(|choice| choice.contains("mysql") || choice.contains("postgres"))
}

fn spring_dependency_xml(dependency: &str) -> Option<&'static str> {
    match dependency {
        "data-jpa" => Some("    <dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-data-jpa</artifactId></dependency>"),
        "mysql" => Some("    <dependency><groupId>com.mysql</groupId><artifactId>mysql-connector-j</artifactId><scope>runtime</scope></dependency>"),
        "postgresql" => Some("    <dependency><groupId>org.postgresql</groupId><artifactId>postgresql</artifactId><scope>runtime</scope></dependency>"),
        "data-redis" => Some("    <dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-data-redis</artifactId></dependency>"),
        "amqp" => Some("    <dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-amqp</artifactId></dependency>"),
        "kafka" => Some("    <dependency><groupId>org.springframework.kafka</groupId><artifactId>spring-kafka</artifactId></dependency>"),
        "security" => Some("    <dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-security</artifactId></dependency>"),
        _ => None,
    }
}

fn configure_spring_selection(
    root: &Path,
    project_name: &str,
    recommendation: &StackRecommendationPayload,
) -> Result<(), String> {
    let pom_path = root.join("pom.xml");
    let mut pom = fs::read_to_string(&pom_path).map_err(|error| error.to_string())?;
    for dependency in spring_initializr_dependencies(recommendation) {
        let Some(xml) = spring_dependency_xml(&dependency) else {
            continue;
        };
        let artifact = xml
            .split("<artifactId>")
            .nth(1)
            .and_then(|value| value.split("</artifactId>").next())
            .unwrap_or_default();
        if !artifact.is_empty() && !pom.contains(&format!("<artifactId>{artifact}</artifactId>")) {
            pom = pom.replacen("  </dependencies>", &format!("{xml}\n  </dependencies>"), 1);
        }
    }
    fs::write(&pom_path, pom).map_err(|error| error.to_string())?;

    let slug = project_slug(project_name);
    let base = root.join("src/main/resources/application.yml");
    if has_relational_database(recommendation) {
        write_file(
            &root.join("src/main/resources/application-database.yml"),
            &format!(
                "# 仅在已准备数据库连接时启用：--spring.profiles.active=database\nspring:\n  autoconfigure:\n    exclude: []\n  datasource:\n    url: ${{DATABASE_URL:jdbc:mysql://localhost:3306/{slug}}}\n    username: ${{DATABASE_USERNAME:root}}\n    password: ${{DATABASE_PASSWORD:}}\n"
            ),
        )?;
    } else if let Ok(content) = fs::read_to_string(&base) {
        let cleaned = content
            .replace("  autoconfigure:\n    exclude: org.springframework.boot.autoconfigure.jdbc.DataSourceAutoConfiguration\n", "")
            .replace("spring.autoconfigure.exclude=org.springframework.boot.autoconfigure.jdbc.DataSourceAutoConfiguration\n", "");
        fs::write(base, cleaned).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn spring_java_version(recommendation: &StackRecommendationPayload) -> String {
    normalized_choices(recommendation)
        .iter()
        .find_map(|choice| {
            choice.split_whitespace().find_map(|part| {
                part.parse::<u8>()
                    .ok()
                    .filter(|version| *version >= 17)
                    .map(|version| version.to_string())
            })
        })
        .unwrap_or_else(|| "17".to_string())
}

fn detected_java_version() -> String {
    let output = Command::new("java").arg("-version").output();
    let text = output
        .ok()
        .map(|output| {
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
        .unwrap_or_default();
    text.split(|character: char| !character.is_ascii_digit())
        .find_map(|value| value.parse::<u8>().ok().filter(|version| *version >= 17))
        .map(|version| version.to_string())
        .unwrap_or_else(|| "17".to_string())
}

fn detected_dotnet_target_framework() -> String {
    let definition = super::env::tool_definition("dotnet").expect("dotnet definition must exist");
    let output = Command::new(super::program_path(definition))
        .arg("--version")
        .output();
    let text = output
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
        .unwrap_or_default();
    let major = text
        .split('.')
        .next()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .filter(|version| *version >= 8)
        .unwrap_or(8);
    format!("net{major}.0")
}

fn write_spring_health_controller(root: &Path, project_name: &str) -> Result<(), String> {
    let slug = project_slug(project_name);
    let package_name = format!("com.vibe.{slug}");
    let package_path = format!("src/main/java/com/vibe/{slug}");
    write_file(
        &root.join(format!("{package_path}/HealthController.java")),
        &format!(
            r#"package {package_name};

import java.util.Map;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api")
public class HealthController {{
    @GetMapping("/health")
    public Map<String, String> health() {{
        return Map.of("status", "UP", "application", "{slug}");
    }}
}}
"#
        ),
    )?;
    write_file(
        &root.join("src/main/resources/application.properties"),
        &format!(
            "spring.application.name={slug}-backend\n# 外部数据服务通过环境配置接入；空白骨架默认可直接完成启动检查。\nspring.autoconfigure.exclude=org.springframework.boot.autoconfigure.jdbc.DataSourceAutoConfiguration\n"
        ),
    )
}

fn write_vue_app(root: &Path, title: &str) -> Result<(), String> {
    write_file(
        &root.join("package.json"),
        &format!(
            r#"{{
  "name": "{}-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{ "dev": "vite", "build": "vite build", "test": "vitest run" }},
  "dependencies": {{ "vue": "^3.5.0" }},
  "devDependencies": {{ "@vitejs/plugin-vue": "^6.0.0", "@vue/test-utils": "^2.4.6", "jsdom": "^26.1.0", "typescript": "^5.7.0", "vite": "^7.0.0", "vitest": "^3.2.4" }}
}}
"#,
            project_slug(title)
        ),
    )?;
    write_file(
        &root.join("index.html"),
        &format!(
            r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="UTF-8" /><meta name="viewport" content="width=device-width, initial-scale=1.0" /><title>{title}</title></head>
<body><div id="app"></div><script type="module" src="/src/main.ts"></script></body></html>
"#
        ),
    )?;
    write_file(&root.join("vite.config.ts"), "import { defineConfig } from 'vite'\nimport vue from '@vitejs/plugin-vue'\n\nexport default defineConfig({ plugins: [vue()] })\n")?;
    write_file(&root.join("tsconfig.json"), "{\n  \"compilerOptions\": { \"target\": \"ES2020\", \"module\": \"ESNext\", \"moduleResolution\": \"Bundler\", \"strict\": true, \"jsx\": \"preserve\", \"skipLibCheck\": true },\n  \"include\": [\"src/**/*.ts\", \"src/**/*.vue\"]\n}\n")?;
    write_file(&root.join("src/main.ts"), "import { createApp } from 'vue'\nimport App from './App.vue'\nimport './style.css'\n\ncreateApp(App).mount('#app')\n")?;
    write_file(
        &root.join("src/App.vue"),
        &format!(
            r#"<script setup lang="ts">
const title = '{title}'
</script>

<template>
  <main><p class="eyebrow">Project skeleton</p><h1>{{{{ title }}}}</h1><p>Vue 3 + Vite 前端已就绪。</p></main>
</template>
"#
        ),
    )?;
    write_file(&root.join("src/style.css"), "* { box-sizing: border-box; } body { margin: 0; font-family: Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; color: #1b1f2a; background: #f7f8fa; } main { max-width: 760px; margin: 0 auto; padding: 72px 24px; } .eyebrow { color: #1769e0; font-weight: 600; } h1 { margin: 8px 0; font-size: 36px; }\n")
        .and_then(|_| {
            write_file(
                &root.join("src/App.test.ts"),
                "// @vitest-environment jsdom\nimport { mount } from '@vue/test-utils'\nimport { describe, expect, it } from 'vitest'\nimport App from './App.vue'\n\ndescribe('App', () => {\n  it('renders the project title', () => {\n    expect(mount(App).find('h1').text()).not.toBe('')\n  })\n})\n",
            )
        })
}

fn write_spring_boot(root: &Path, project_name: &str) -> Result<(), String> {
    let slug = project_slug(project_name);
    let class_name = format!("{}Application", pascal_case(project_name));
    let package_name = format!("com.vibe.{slug}");
    let package_path = format!("src/main/java/com/vibe/{slug}");
    write_file(
        &root.join("pom.xml"),
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <parent><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-parent</artifactId><version>3.5.0</version><relativePath/></parent>
  <groupId>{package_name}</groupId><artifactId>{slug}-backend</artifactId><version>0.1.0</version>
  <properties><java.version>{}</java.version></properties>
  <dependencies>
    <dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-web</artifactId></dependency>
    <dependency><groupId>org.springframework.boot</groupId><artifactId>spring-boot-starter-test</artifactId><scope>test</scope></dependency>
  </dependencies>
  <build><plugins><plugin><groupId>org.springframework.boot</groupId><artifactId>spring-boot-maven-plugin</artifactId></plugin></plugins></build>
</project>
"#,
            detected_java_version()
        ),
    )?;
    write_file(
        &root.join(format!("{package_path}/{class_name}.java")),
        &format!(
            r#"package {package_name};

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

@SpringBootApplication
public class {class_name} {{
    public static void main(String[] args) {{
        SpringApplication.run({class_name}.class, args);
    }}
}}
"#
        ),
    )?;
    write_file(
        &root.join(format!(
            "src/test/java/com/vibe/{slug}/{class_name}Tests.java"
        )),
        &format!(
            r#"package {package_name};

import org.junit.jupiter.api.Test;
import org.springframework.boot.test.context.SpringBootTest;

@SpringBootTest
class {class_name}Tests {{
    @Test
    void contextLoads() {{}}
}}
"#
        ),
    )?;
    write_spring_health_controller(root, project_name)?;
    write_file(&root.join("src/main/resources/application.yml"), &format!("spring:\n  application:\n    name: {slug}-backend\n  autoconfigure:\n    exclude: org.springframework.boot.autoconfigure.jdbc.DataSourceAutoConfiguration\nserver:\n  port: 8080\n"))
}

fn command_error(label: &str, output: std::process::Output) -> String {
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.is_empty() {
        format!("{label} 执行失败，退出码：{}", output.status)
    } else {
        format!(
            "{label} 执行失败：{}",
            detail.chars().take(600).collect::<String>()
        )
    }
}

fn run_generator(
    program: &str,
    args: &[String],
    current_dir: &Path,
    label: &str,
) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .output()
        .map_err(|error| format!("无法启动{label}：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(label, output))
    }
}

fn url_component(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => vec![byte as char],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn generate_vue_with_vite(root: &Path) -> Result<(), String> {
    run_generator(
        "npm",
        &[
            "exec".to_string(),
            "--yes".to_string(),
            "create-vite@latest".to_string(),
            "--".to_string(),
            ".".to_string(),
            "--template".to_string(),
            "vue-ts".to_string(),
        ],
        root,
        "Vite 官方脚手架",
    )?;
    ensure_vue_test_baseline(root)
}

fn ensure_vue_test_baseline(root: &Path) -> Result<(), String> {
    let package_path = root.join("package.json");
    let mut package: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&package_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("无法解析 Vue package.json：{error}"))?;
    let object = package
        .as_object_mut()
        .ok_or_else(|| "Vue package.json 顶层必须是对象".to_string())?;
    object
        .entry("scripts")
        .or_insert_with(|| serde_json::json!({}))["test"] = serde_json::json!("vitest run");
    let dev = object
        .entry("devDependencies")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| "Vue devDependencies 必须是对象".to_string())?;
    for (name, version) in [
        ("@vue/test-utils", "^2.4.6"),
        ("jsdom", "^26.1.0"),
        ("vitest", "^3.2.4"),
    ] {
        dev.insert(name.to_string(), serde_json::json!(version));
    }
    fs::write(
        package_path,
        serde_json::to_string_pretty(&package).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())?;
    write_file(
        &root.join("src/App.test.ts"),
        "// @vitest-environment jsdom\nimport { mount } from '@vue/test-utils'\nimport { describe, expect, it } from 'vitest'\nimport App from './App.vue'\n\ndescribe('App', () => {\n  it('renders the project root', () => {\n    expect(mount(App).exists()).toBe(true)\n  })\n})\n",
    )
}

fn generate_spring_boot_with_initializr(
    root: &Path,
    project_name: &str,
    recommendation: &StackRecommendationPayload,
) -> Result<(), String> {
    let slug = project_slug(project_name);
    let package_name = format!("com.vibe.{slug}");
    let dependencies = spring_initializr_dependencies(recommendation).join(",");
    let query = [
        ("type", "maven-project".to_string()),
        ("language", "java".to_string()),
        ("groupId", "com.vibe".to_string()),
        ("artifactId", format!("{slug}-backend")),
        ("name", project_name.to_string()),
        ("packageName", package_name),
        ("javaVersion", spring_java_version(recommendation)),
        ("dependencies", dependencies),
    ]
    .into_iter()
    .map(|(key, value)| format!("{key}={}", url_component(&value)))
    .collect::<Vec<_>>()
    .join("&");
    let archive = std::env::temp_dir().join(format!("vibe-spring-{}.zip", uuid::Uuid::new_v4()));
    let archive_value = archive.to_string_lossy().to_string();
    let url = format!("https://start.spring.io/starter.zip?{query}");
    let download = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "10",
            "--max-time",
            "60",
            "--output",
            &archive_value,
            &url,
        ])
        .output()
        .map_err(|error| format!("无法启动 Spring Initializr：{error}"))?;
    if !download.status.success() {
        return Err(command_error("Spring Initializr", download));
    }
    let unzip = Command::new("unzip")
        .args(["-q", &archive_value, "-d", &root.to_string_lossy()])
        .output()
        .map_err(|error| format!("无法解压 Spring Initializr 工程：{error}"))?;
    let _ = fs::remove_file(&archive);
    if !unzip.status.success() {
        return Err(command_error("Spring Initializr 解压", unzip));
    }
    if !root.join("pom.xml").is_file() {
        return Err("Spring Initializr 未生成 pom.xml".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let wrapper = root.join("mvnw");
        if wrapper.is_file() {
            let mut permissions = fs::metadata(&wrapper)
                .map_err(|error| error.to_string())?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(wrapper, permissions).map_err(|error| error.to_string())?;
        }
    }
    write_spring_health_controller(root, project_name)?;
    configure_spring_selection(root, project_name, recommendation)
}

fn create_official_vue_spring_boot(
    request: &CreateProjectRequest,
) -> Result<CreateProjectResult, String> {
    if request.recommendation.structure != "frontend-backend" {
        return Err("vue-spring-boot 模板必须使用前后端分离结构".to_string());
    }
    let frontend_name = split_project_name(
        request.frontend_project_name.as_ref(),
        &request.project_name,
        "frontend",
    );
    let backend_name = split_project_name(
        request.backend_project_name.as_ref(),
        &request.project_name,
        "backend",
    );
    if frontend_name == backend_name {
        return Err("前端项目名和后端项目名不能相同".to_string());
    }
    let frontend = validate_target_dir(&request.parent_path, &frontend_name)?;
    let backend = validate_target_dir(&request.parent_path, &backend_name)?;
    let result = (|| -> Result<CreateProjectResult, String> {
        fs::create_dir_all(&frontend).map_err(|error| format!("无法创建前端项目目录：{error}"))?;
        fs::create_dir_all(&backend).map_err(|error| format!("无法创建后端项目目录：{error}"))?;
        generate_vue_with_vite(&frontend)?;
        generate_spring_boot_with_initializr(&backend, &backend_name, &request.recommendation)?;
        let agent_mode = finalize_project(
            &frontend,
            request,
            &write_frontend_readme(&frontend_name, &backend_name, "http://localhost:8080"),
        )?;
        finalize_project(
            &backend,
            request,
            &write_backend_readme(
                &backend_name,
                &frontend_name,
                "Spring Boot / Java",
                "./mvnw spring-boot:run",
                "http://localhost:8080/api/health",
            ),
        )?;
        Ok(CreateProjectResult {
            project_paths: vec![
                frontend.to_string_lossy().to_string(),
                backend.to_string_lossy().to_string(),
            ],
            agent_mode,
            message: "已使用 Vite 和 Spring Initializr 官方脚手架生成独立前后端项目。".to_string(),
            verification: ProjectVerificationResult {
                status: "pending".to_string(),
                checks: vec![],
                detail: "等待项目自检".to_string(),
            },
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&frontend);
        let _ = fs::remove_dir_all(&backend);
    }
    result
}

fn write_nest_app(root: &Path, title: &str) -> Result<(), String> {
    write_file(
        &root.join("package.json"),
        &format!(
            r#"{{
  "name": "{}-backend",
  "private": true,
  "version": "0.1.0",
  "scripts": {{ "start:dev": "tsx watch src/main.ts", "start": "tsx src/main.ts", "build": "tsc --noEmit" }},
  "dependencies": {{ "@nestjs/common": "^11.0.0", "@nestjs/core": "^11.0.0", "@nestjs/platform-express": "^11.0.0", "reflect-metadata": "^0.2.0", "rxjs": "^7.8.0" }},
  "devDependencies": {{ "@types/node": "^22.0.0", "tsx": "^4.19.0", "typescript": "^5.7.0" }}
}}
"#,
            project_slug(title)
        ),
    )?;
    write_file(&root.join("tsconfig.json"), "{\n  \"compilerOptions\": { \"module\": \"commonjs\", \"target\": \"ES2021\", \"experimentalDecorators\": true, \"emitDecoratorMetadata\": true, \"strict\": true, \"esModuleInterop\": true }\n}\n")?;
    write_file(&root.join("src/main.ts"), "import 'reflect-metadata'\nimport { NestFactory } from '@nestjs/core'\nimport { AppModule } from './app.module'\n\nasync function bootstrap() {\n  const app = await NestFactory.create(AppModule)\n  await app.listen(Number(process.env.PORT ?? 3000), '127.0.0.1')\n}\nvoid bootstrap()\n")?;
    write_file(&root.join("src/app.module.ts"), "import { Controller, Get, Module } from '@nestjs/common'\n\n@Controller('api')\nclass HealthController { @Get('health') health() { return { status: 'UP' } } }\n\n@Module({ controllers: [HealthController] })\nexport class AppModule {}\n")
}

fn write_fastapi_app(root: &Path, project_name: &str) -> Result<(), String> {
    let slug = project_slug(project_name);
    write_file(
        &root.join("pyproject.toml"),
        &format!(
            r#"[project]
name = "{slug}"
version = "0.1.0"
description = "FastAPI service"
requires-python = ">=3.9"
dependencies = ["fastapi", "uvicorn[standard]"]

[project.scripts]
{slug} = "app.main:run"
"#
        ),
    )?;
    write_file(
        &root.join("app/main.py"),
        &format!(
            r#"from fastapi import FastAPI

app = FastAPI(title="{project_name}")


@app.get("/api/health")
async def health() -> dict[str, str]:
    return {{"status": "UP", "application": "{slug}"}}


def run() -> None:
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
"#
        ),
    )?;
    write_file(&root.join("app/__init__.py"), "")
}

fn write_go_app(root: &Path, project_name: &str) -> Result<(), String> {
    let slug = project_slug(project_name);
    write_file(
        &root.join("go.mod"),
        &format!("module github.com/vibe/{slug}\n\ngo 1.22\n"),
    )?;
    write_file(
        &root.join("main.go"),
        &format!(
            r#"package main

import (
    "encoding/json"
    "net/http"
    "os"
)

func main() {{
    port := os.Getenv("PORT")
    if port == "" {{ port = "8080" }}
    http.HandleFunc("/api/health", func(w http.ResponseWriter, r *http.Request) {{
        w.Header().Set("Content-Type", "application/json")
        _ = json.NewEncoder(w).Encode(map[string]string{{"status": "UP", "application": "{slug}"}})
    }})
    _ = http.ListenAndServe(":" + port, nil)
}}
"#
        ),
    )
}

fn write_axum_app(root: &Path, project_name: &str) -> Result<(), String> {
    let slug = project_slug(project_name);
    write_file(
        &root.join("Cargo.toml"),
        &format!(
            r#"[package]
name = "{slug}"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
serde_json = "1"
tokio = {{ version = "1", features = ["macros", "rt-multi-thread", "net"] }}
"#
        ),
    )?;
    write_file(
        &root.join("src/main.rs"),
        &format!(
            r#"use axum::{{routing::get, Json, Router}};
use serde_json::json;

#[tokio::main]
async fn main() {{
    let app = Router::new().route("/api/health", get(|| async {{ Json(json!({{"status": "UP", "application": "{slug}"}})) }}));
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{{port}}")).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}}
"#
        ),
    )
}

fn write_aspnet_app(root: &Path, project_name: &str) -> Result<(), String> {
    let slug = project_slug(project_name);
    write_file(
        &root.join(format!("{project_name}.csproj")),
        &format!(
            r#"<Project Sdk="Microsoft.NET.Sdk.Web">
  <PropertyGroup>
    <TargetFramework>{}</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
  </PropertyGroup>
</Project>
"#,
            detected_dotnet_target_framework()
        ),
    )?;
    write_file(
        &root.join("Program.cs"),
        &format!(
            r#"var builder = WebApplication.CreateBuilder(args);
var app = builder.Build();

app.MapGet("/api/health", () => Results.Ok(new {{ status = "UP", application = "{slug}" }}));

app.Run();
"#
        ),
    )
}

fn write_next_app(root: &Path, title: &str) -> Result<(), String> {
    write_file(
        &root.join("package.json"),
        &format!(
            r#"{{
  "name": "{}",
  "private": true,
  "version": "0.1.0",
  "scripts": {{ "dev": "next dev", "build": "next build", "start": "next start" }},
  "dependencies": {{ "next": "^15.0.0", "react": "^19.0.0", "react-dom": "^19.0.0" }},
  "devDependencies": {{ "@types/node": "^22.0.0", "@types/react": "^19.0.0", "typescript": "^5.7.0" }}
}}
"#,
            project_slug(title)
        ),
    )?;
    write_file(&root.join("tsconfig.json"), "{\n  \"compilerOptions\": { \"target\": \"ES2020\", \"lib\": [\"dom\", \"dom.iterable\", \"esnext\"], \"strict\": true, \"module\": \"esnext\", \"moduleResolution\": \"bundler\", \"jsx\": \"preserve\", \"noEmit\": true, \"esModuleInterop\": true },\n  \"include\": [\"next-env.d.ts\", \"**/*.ts\", \"**/*.tsx\", \".next/types/**/*.ts\"]\n}\n")?;
    write_file(
        &root.join("next-env.d.ts"),
        "/// <reference types=\"next\" />\n/// <reference types=\"next/image-types/global\" />\n",
    )?;
    write_file(&root.join("app/layout.tsx"), &format!("export const metadata = {{ title: '{title}' }}\n\nexport default function RootLayout({{ children }}: Readonly<{{ children: React.ReactNode }}>) {{ return <html lang=\"zh-CN\"><body>{{children}}</body></html> }}\n"))?;
    write_file(&root.join("app/page.tsx"), &format!("export default function Home() {{ return <main style={{{{ maxWidth: 760, margin: '0 auto', padding: 72 }}}}><p>Project skeleton</p><h1>{title}</h1><p>Next.js 应用已就绪。</p></main> }}\n"))
}

fn write_tauri_app(root: &Path, title: &str) -> Result<(), String> {
    write_vue_app(root, title)?;
    let slug = project_slug(title);
    write_file(
        &root.join("package.json"),
        &format!(
            r#"{{
  "name": "{slug}",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{ "dev": "vite", "build": "vite build", "test": "vitest run", "tauri": "tauri" }},
  "dependencies": {{ "@tauri-apps/api": "^2.0.0", "vue": "^3.5.0" }},
  "devDependencies": {{ "@tauri-apps/cli": "^2.0.0", "@vitejs/plugin-vue": "^6.0.0", "@vue/test-utils": "^2.4.6", "jsdom": "^26.1.0", "typescript": "^5.7.0", "vite": "^7.0.0", "vitest": "^3.2.4" }}
}}
"#
        ),
    )?;
    write_file(&root.join("vite.config.ts"), "import { defineConfig } from 'vite'\nimport vue from '@vitejs/plugin-vue'\n\nexport default defineConfig({ plugins: [vue()], server: { port: 1420, strictPort: true } })\n")?;
    write_file(&root.join("src-tauri/Cargo.toml"), &format!("[package]\nname = \"{slug}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[build-dependencies]\ntauri-build = {{ version = \"2\" }}\n\n[dependencies]\ntauri = {{ version = \"2\" }}\n"))?;
    write_file(
        &root.join("src-tauri/build.rs"),
        "fn main() { tauri_build::build() }\n",
    )?;
    write_file(&root.join("src-tauri/src/main.rs"), "fn main() { tauri::Builder::default().run(tauri::generate_context!()).expect(\"启动桌面应用失败\"); }\n")?;
    let icon_path = root.join("src-tauri/icons/icon.png");
    if let Some(parent) = icon_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&icon_path, include_bytes!("../../icons/icon.png"))
        .map_err(|error| error.to_string())?;
    write_file(
        &root.join("src-tauri/tauri.conf.json"),
        &format!(
            r#"{{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "{title}", "version": "0.1.0", "identifier": "com.vibe.{slug}",
  "build": {{ "beforeDevCommand": "npm run dev", "devUrl": "http://localhost:1420", "beforeBuildCommand": "npm run build", "frontendDist": "../dist" }},
  "app": {{ "windows": [{{ "title": "{title}", "width": 1100, "height": 760 }}] }}, "bundle": {{ "active": true }}
}}
"#
        ),
    )?;
    write_file(&root.join("src-tauri/capabilities/default.json"), "{\n  \"$schema\": \"../gen/schemas/desktop-schema.json\", \"identifier\": \"default\", \"description\": \"默认桌面能力\", \"windows\": [\"main\"], \"permissions\": [\"core:default\"]\n}\n")
}

fn write_root_readme(request: &CreateProjectRequest) -> String {
    let startup = match request.recommendation.id.as_str() {
        "vue-spring-boot" => "## 目录\n\n- `frontend/`：Vue 3 + Vite 前端\n- `backend/`：Spring Boot / Java 后端，提供 `GET /api/health`\n\n## 启动\n\n前端：\n\n```bash\nnpm --prefix frontend install\nnpm --prefix frontend run dev\n```\n\n后端：\n\n```bash\nmvn -f backend/pom.xml spring-boot:run\n```\n\n后端启动后访问 `http://localhost:8080/api/health` 验证服务。",
        "node-nestjs" => "## 目录\n\n- `frontend/`：Vue 3 + Vite 前端\n- `backend/`：NestJS 后端，提供 `GET /api/health`\n\n## 启动\n\n```bash\nnpm --prefix frontend install\nnpm --prefix frontend run dev\nnpm --prefix backend install\nnpm --prefix backend run start:dev\n```",
        "nextjs" => "## 启动\n\n```bash\nnpm install\nnpm run dev\n```",
        "tauri-vue" => "## 启动\n\n```bash\nnpm install\nnpm run tauri dev\n```",
        _ => "## 启动\n\n```bash\nnpm install\nnpm run dev\n```",
    };
    format!("# {}\n\n技术方案：{}\n\n{}\n\n## 外部服务\n\n数据库、缓存和消息队列通过环境配置连接开发环境、Docker 或云服务；创建项目时不会安装它们的服务端。\n", request.project_name, request.recommendation.title, startup)
}

fn write_frontend_readme(
    project_name: &str,
    backend_project_name: &str,
    backend_url: &str,
) -> String {
    format!("# {project_name}\n\nVue 3 + Vite 前端项目。后端独立项目：`{backend_project_name}`。\n\n## 启动\n\n```bash\nnpm install\nnpm run dev\n```\n\n默认后端地址为 `{backend_url}`。\n")
}

fn write_backend_readme(
    project_name: &str,
    frontend_project_name: &str,
    runtime: &str,
    command: &str,
    health_url: &str,
) -> String {
    format!("# {project_name}\n\n{runtime} 后端项目。前端独立项目：`{frontend_project_name}`。\n\n## 启动\n\n```bash\n{command}\n```\n\n启动后访问 `{health_url}` 验证服务。\n")
}

fn enrich_readme(base: &str, request: &CreateProjectRequest) -> String {
    let requirement = if request.concise_requirement.trim().is_empty() {
        request.profile.summary.trim()
    } else {
        request.concise_requirement.trim()
    };
    let constraints = if request.recognized_constraints.is_empty() {
        "- 当前未识别到额外硬性约束；后续以确认后的需求和代码事实为准。".to_string()
    } else {
        request
            .recognized_constraints
            .iter()
            .map(|item| format!("- **{}**：{}", item.label, item.value))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let decisions = if request.recommendation.decisions.is_empty() {
        format!(
            "| 整体方案 | 采用 | {} | {} |",
            request.recommendation.title,
            request.recommendation.reasons.join("；")
        )
    } else {
        request
            .recommendation
            .decisions
            .iter()
            .map(|decision| {
                format!(
                    "| {} | {} | {} | {} |",
                    decision.title,
                    decision.status,
                    if decision.choices.is_empty() {
                        "—".to_string()
                    } else {
                        decision.choices.join("、")
                    },
                    decision.reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "{base}\n## 项目目标\n\n{requirement}\n\n## 已确认约束\n\n{constraints}\n\n## 技术决策\n\n| 决策 | 状态 | 选择 | 原因 |\n|---|---|---|---|\n{decisions}\n\n## 开发资料\n\n- `docs/`：项目长期真源、需求与技术选型、详设/进度模板。\n- `CLAUDE.md` / `AGENTS.md`：每次 Agent 会话自动读取的稳定工程基线。\n- `.claude/rules/`：必须持续遵守的项目规则。\n- `.claude/skills/`：后续会话按任务触发的详设、开发、自测等工作流。\n\n> 项目工厂到生成与校验即结束，不会自动开发业务功能。需要开发时，请在新会话中明确提出需求。\n"
    )
}

fn finalize_project(
    root: &Path,
    request: &CreateProjectRequest,
    readme: &str,
) -> Result<String, String> {
    write_file(&root.join(".gitignore"), "node_modules/\ndist/\ntarget/\nbin/\nobj/\n.venv/\n__pycache__/\n*.py[cod]\n.env\n.idea/\n")?;
    write_file(&root.join("README.md"), &enrich_readme(readme, request))?;
    let git = Command::new("git")
        .arg("init")
        .current_dir(root)
        .output()
        .map_err(|error| format!("无法初始化 Git 仓库：{error}"))?;
    if !git.status.success() {
        return Err(command_error("Git 初始化", git));
    }
    write_project_docs(root, request)?;
    let agent_mode = write_ai_rules(root, request)?;
    Ok(agent_mode)
}

fn split_project_name(value: Option<&String>, base: &str, suffix: &str) -> String {
    value
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{base}-{suffix}"))
}

type BackendWriter = fn(&Path, &str) -> Result<(), String>;

fn create_split_project(
    request: &CreateProjectRequest,
    expected_id: &str,
    runtime: &str,
    command: &str,
    backend_url: &str,
    writer: BackendWriter,
) -> Result<(Vec<String>, String), String> {
    if request.recommendation.id != expected_id
        || request.recommendation.structure != "frontend-backend"
    {
        return Err(format!("{expected_id} 模板必须使用前后端分离结构"));
    }
    let frontend_name = split_project_name(
        request.frontend_project_name.as_ref(),
        &request.project_name,
        "frontend",
    );
    let backend_name = split_project_name(
        request.backend_project_name.as_ref(),
        &request.project_name,
        "backend",
    );
    if frontend_name == backend_name {
        return Err("前端项目名和后端项目名不能相同".to_string());
    }
    let frontend = validate_target_dir(&request.parent_path, &frontend_name)?;
    let backend = validate_target_dir(&request.parent_path, &backend_name)?;
    fs::create_dir_all(&frontend).map_err(|error| format!("无法创建前端项目目录：{error}"))?;
    fs::create_dir_all(&backend).map_err(|error| format!("无法创建后端项目目录：{error}"))?;
    write_vue_app(&frontend, &frontend_name)?;
    writer(&backend, &backend_name)?;
    if expected_id == "vue-spring-boot" {
        configure_spring_selection(&backend, &backend_name, &request.recommendation)?;
    }
    let agent_mode = finalize_project(
        &frontend,
        request,
        &write_frontend_readme(&frontend_name, &backend_name, backend_url),
    )?;
    finalize_project(
        &backend,
        request,
        &write_backend_readme(
            &backend_name,
            &frontend_name,
            runtime,
            command,
            &format!("{backend_url}/api/health"),
        ),
    )?;
    Ok((
        vec![
            frontend.to_string_lossy().to_string(),
            backend.to_string_lossy().to_string(),
        ],
        agent_mode,
    ))
}

fn write_api_readme(project_name: &str, runtime: &str, command: &str, health_url: &str) -> String {
    format!("# {project_name}\n\n{runtime} API 服务。\n\n## 启动\n\n```bash\n{command}\n```\n\n启动后访问 `{health_url}` 验证服务。\n")
}

fn create_api_project(
    request: &CreateProjectRequest,
    expected_id: &str,
    runtime: &str,
    command: &str,
    health_url: &str,
    writer: BackendWriter,
) -> Result<(Vec<String>, String), String> {
    if request.recommendation.id != expected_id || request.recommendation.structure != "single-app"
    {
        return Err(format!("{expected_id} 模板必须使用单项目结构"));
    }
    let target = validate_target_dir(&request.parent_path, &request.project_name)?;
    fs::create_dir_all(&target).map_err(|error| format!("无法创建项目目录：{error}"))?;
    writer(&target, &request.project_name)?;
    let agent_mode = finalize_project(
        &target,
        request,
        &write_api_readme(&request.project_name, runtime, command, health_url),
    )?;
    Ok((vec![target.to_string_lossy().to_string()], agent_mode))
}

pub fn create_project(request: &CreateProjectRequest) -> Result<CreateProjectResult, String> {
    let (project_paths, agent_mode) = match request.recommendation.id.as_str() {
        "vue-spring-boot" => create_split_project(
            request,
            "vue-spring-boot",
            "Spring Boot / Java",
            "mvn spring-boot:run",
            "http://localhost:8080",
            write_spring_boot,
        )?,
        "node-nestjs" => create_split_project(
            request,
            "node-nestjs",
            "NestJS / TypeScript",
            "npm install\nnpm run start:dev",
            "http://localhost:3000",
            write_nest_app,
        )?,
        "vue-fastapi" => create_split_project(
            request,
            "vue-fastapi",
            "FastAPI / Python",
            "python -m uvicorn app.main:app --reload",
            "http://localhost:8000",
            write_fastapi_app,
        )?,
        "vue-go" => create_split_project(
            request,
            "vue-go",
            "Go",
            "go run .",
            "http://localhost:8080",
            write_go_app,
        )?,
        "vue-axum" => create_split_project(
            request,
            "vue-axum",
            "Axum / Rust",
            "cargo run",
            "http://localhost:8080",
            write_axum_app,
        )?,
        "vue-aspnet" => create_split_project(
            request,
            "vue-aspnet",
            "ASP.NET Core / .NET",
            "dotnet run",
            "http://localhost:5000",
            write_aspnet_app,
        )?,
        "fastapi-api" => create_api_project(
            request,
            "fastapi-api",
            "FastAPI / Python",
            "python -m uvicorn app.main:app --reload",
            "http://localhost:8000/api/health",
            write_fastapi_app,
        )?,
        "go-api" => create_api_project(
            request,
            "go-api",
            "Go",
            "go run .",
            "http://localhost:8080/api/health",
            write_go_app,
        )?,
        "axum-api" => create_api_project(
            request,
            "axum-api",
            "Axum / Rust",
            "cargo run",
            "http://localhost:8080/api/health",
            write_axum_app,
        )?,
        "aspnet-api" => create_api_project(
            request,
            "aspnet-api",
            "ASP.NET Core / .NET",
            "dotnet run",
            "http://localhost:5000/api/health",
            write_aspnet_app,
        )?,
        "nextjs" | "tauri-vue" | "vue-vite" => {
            let target = validate_target_dir(&request.parent_path, &request.project_name)?;
            fs::create_dir_all(&target).map_err(|error| format!("无法创建项目目录：{error}"))?;
            match request.recommendation.id.as_str() {
                "nextjs" => write_next_app(&target, &request.project_name)?,
                "tauri-vue" => write_tauri_app(&target, &request.project_name)?,
                "vue-vite" => write_vue_app(&target, &request.project_name)?,
                _ => unreachable!(),
            }
            let agent_mode = finalize_project(&target, request, &write_root_readme(request))?;
            (vec![target.to_string_lossy().to_string()], agent_mode)
        }
        _ => return Err("当前技术模板尚不能生成".to_string()),
    };
    Ok(CreateProjectResult {
        project_paths,
        agent_mode,
        message: "已生成项目骨架、已确认文档、项目规则与 skills；未自动开发业务功能。".to_string(),
        verification: ProjectVerificationResult {
            status: "skipped".to_string(),
            checks: vec![],
            detail: "当前模板尚未执行启动自检。".to_string(),
        },
    })
}

fn run_command_with_timeout(
    program: &Path,
    args: &[&str],
    current_dir: &Path,
    timeout: Duration,
    label: &str,
) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法启动{label}：{error}"))?;
    let started_at = Instant::now();
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("检查{label}状态失败：{error}"))?
        {
            Some(status) if status.success() => return Ok(()),
            Some(status) => return Err(format!("{label}失败，退出码：{status}")),
            None if started_at.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{label}超过 {} 秒仍未结束", timeout.as_secs()));
            }
            None => thread::sleep(Duration::from_millis(250)),
        }
    }
}

fn spawn_probe_command(command: &mut Command, label: &str) -> Result<std::process::Child, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
    }
    command
        .spawn()
        .map_err(|error| format!("无法启动{label}：{error}"))
}

fn stop_probe_process(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        let process_group = -(child.id() as i32);
        let _ = libc::kill(process_group, libc::SIGTERM);
        thread::sleep(Duration::from_millis(250));
        if child.try_wait().ok().flatten().is_none() {
            let _ = libc::kill(process_group, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn available_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("无法分配本地验证端口：{error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("无法读取本地验证端口：{error}"))?
        .port();
    drop(listener);
    Ok(port)
}

fn wait_for_server(
    mut child: std::process::Child,
    url: &str,
    timeout: Duration,
    label: &str,
) -> Result<(), String> {
    let started_at = Instant::now();
    let result = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("检查{label}状态失败：{error}"))?
        {
            break Err(format!("{label}在健康检查前退出，退出码：{status}"));
        }
        if ureq::get(url)
            .timeout(Duration::from_secs(2))
            .call()
            .is_ok()
        {
            break Ok(());
        }
        if started_at.elapsed() >= timeout {
            break Err(format!(
                "{label}在 {} 秒内没有通过健康检查",
                timeout.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(400));
    };
    stop_probe_process(&mut child);
    result
}

fn verify_desktop_process(mut child: std::process::Child, label: &str) -> Result<(), String> {
    thread::sleep(Duration::from_secs(5));
    let result = match child
        .try_wait()
        .map_err(|error| format!("检查{label}状态失败：{error}"))?
    {
        Some(status) => Err(format!("{label}在启动检查期间退出，退出码：{status}")),
        None => Ok(()),
    };
    stop_probe_process(&mut child);
    result
}

fn verify_vue_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    let npm = Path::new("npm");
    run_command_with_timeout(
        npm,
        &["install", "--no-audit", "--no-fund"],
        root,
        Duration::from_secs(180),
        "前端依赖安装",
    )?;
    checks.push("前端依赖安装完成".to_string());
    run_command_with_timeout(
        npm,
        &["run", "test"],
        root,
        Duration::from_secs(120),
        "前端基线测试",
    )?;
    checks.push("前端基线测试通过".to_string());
    run_command_with_timeout(
        npm,
        &["run", "build"],
        root,
        Duration::from_secs(120),
        "前端构建",
    )?;
    checks.push("前端构建通过".to_string());
    let port = available_local_port()?;
    let vite = root.join("node_modules/.bin/vite");
    if !vite.is_file() {
        return Err("未找到 Vite 二进制，无法进行前端启动检查".to_string());
    }
    let port_value = port.to_string();
    let mut command = Command::new(vite);
    command
        .args(["--host", "127.0.0.1", "--port", port_value.as_str()])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = spawn_probe_command(&mut command, "前端开发服务器")?;
    wait_for_server(
        child,
        &format!("http://127.0.0.1:{port}/"),
        Duration::from_secs(30),
        "前端开发服务器",
    )?;
    checks.push("前端开发服务器启动并已关闭".to_string());
    Ok(())
}

fn maven_wrapper(root: &Path) -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "windows")]
    let wrapper = root.join("mvnw.cmd");
    #[cfg(not(target_os = "windows"))]
    let wrapper = root.join("mvnw");
    if wrapper.is_file() {
        Ok(wrapper)
    } else {
        Err("未找到 Maven Wrapper，无法验证 Spring Boot 项目".to_string())
    }
}

fn verify_spring_boot_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    let wrapper = maven_wrapper(root)?;
    run_command_with_timeout(
        &wrapper,
        &["-q", "package"],
        root,
        Duration::from_secs(240),
        "Spring Boot 测试与构建",
    )?;
    checks.push("Spring Boot 测试与构建通过".to_string());
    let port = available_local_port()?;
    let jar = fs::read_dir(root.join("target"))
        .map_err(|error| format!("无法读取 Spring Boot 构建产物：{error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().and_then(|extension| extension.to_str()) == Some("jar")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .ends_with(".original")
        })
        .ok_or_else(|| "未找到可启动的 Spring Boot jar".to_string())?;
    let port_argument = format!("--server.port={port}");
    let mut command = Command::new("java");
    command
        .args(["-jar", &jar.to_string_lossy(), port_argument.as_str()])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = spawn_probe_command(&mut command, "Spring Boot")?;
    wait_for_server(
        child,
        &format!("http://127.0.0.1:{port}/api/health"),
        Duration::from_secs(90),
        "Spring Boot 服务",
    )?;
    checks.push("Spring Boot 服务启动、健康检查通过并已关闭".to_string());
    Ok(())
}

fn verify_nest_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    let npm = Path::new("npm");
    run_command_with_timeout(
        npm,
        &["install", "--no-audit", "--no-fund"],
        root,
        Duration::from_secs(180),
        "NestJS 依赖安装",
    )?;
    checks.push("NestJS 依赖安装完成".to_string());
    run_command_with_timeout(
        npm,
        &["run", "build"],
        root,
        Duration::from_secs(120),
        "NestJS 类型检查",
    )?;
    checks.push("NestJS 类型检查通过".to_string());
    let port = available_local_port()?;
    let port_value = port.to_string();
    let tsx = root.join("node_modules/.bin/tsx");
    if !tsx.is_file() {
        return Err("未找到 tsx 二进制，无法进行 NestJS 启动检查".to_string());
    }
    let mut command = Command::new(tsx);
    command
        .args(["src/main.ts"])
        .env("PORT", &port_value)
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = spawn_probe_command(&mut command, "NestJS")?;
    wait_for_server(
        child,
        &format!("http://127.0.0.1:{port}/api/health"),
        Duration::from_secs(45),
        "NestJS 服务",
    )?;
    checks.push("NestJS 服务启动、健康检查通过并已关闭".to_string());
    Ok(())
}

fn verify_fastapi_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    let venv = root.join(".vibe-verify-venv");
    let result = (|| -> Result<(), String> {
        run_command_with_timeout(
            Path::new("python3"),
            &["-m", "venv", ".vibe-verify-venv"],
            root,
            Duration::from_secs(45),
            "Python 虚拟环境创建",
        )?;
        #[cfg(target_os = "windows")]
        let python = venv.join("Scripts/python.exe");
        #[cfg(not(target_os = "windows"))]
        let python = venv.join("bin/python");
        run_command_with_timeout(
            &python,
            &["-m", "pip", "install", "--no-input", "."],
            root,
            Duration::from_secs(180),
            "FastAPI 依赖安装",
        )?;
        checks.push("FastAPI 依赖安装完成".to_string());
        let port = available_local_port()?;
        let port_value = port.to_string();
        let mut command = Command::new(&python);
        command
            .args([
                "-m",
                "uvicorn",
                "app.main:app",
                "--host",
                "127.0.0.1",
                "--port",
                port_value.as_str(),
            ])
            .current_dir(root)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = spawn_probe_command(&mut command, "FastAPI")?;
        wait_for_server(
            child,
            &format!("http://127.0.0.1:{port}/api/health"),
            Duration::from_secs(45),
            "FastAPI 服务",
        )?;
        checks.push("FastAPI 服务启动、健康检查通过并已关闭".to_string());
        Ok(())
    })();
    let _ = fs::remove_dir_all(venv);
    result
}

fn verify_go_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    let binary = root.join(format!(".vibe-verify-go{}", std::env::consts::EXE_SUFFIX));
    let result = (|| -> Result<(), String> {
        run_command_with_timeout(
            Path::new("go"),
            &["build", "-o", ".vibe-verify-go", "."],
            root,
            Duration::from_secs(120),
            "Go 构建",
        )?;
        checks.push("Go 构建通过".to_string());
        let port = available_local_port()?;
        let port_value = port.to_string();
        let mut command = Command::new(&binary);
        command
            .env("PORT", &port_value)
            .current_dir(root)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = spawn_probe_command(&mut command, "Go 服务")?;
        wait_for_server(
            child,
            &format!("http://127.0.0.1:{port}/api/health"),
            Duration::from_secs(30),
            "Go 服务",
        )?;
        checks.push("Go 服务启动、健康检查通过并已关闭".to_string());
        Ok(())
    })();
    let _ = fs::remove_file(binary);
    result
}

fn verify_axum_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    run_command_with_timeout(
        Path::new("cargo"),
        &["build"],
        root,
        Duration::from_secs(240),
        "Axum 构建",
    )?;
    checks.push("Axum 构建通过".to_string());
    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "无法识别 Axum 项目名称".to_string())?;
    let binary = root.join("target/debug").join(format!(
        "{}{}",
        project_slug(project_name),
        std::env::consts::EXE_SUFFIX
    ));
    if !binary.is_file() {
        return Err("未找到 Axum 可执行文件".to_string());
    }
    let port = available_local_port()?;
    let port_value = port.to_string();
    let mut command = Command::new(binary);
    command
        .env("PORT", &port_value)
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = spawn_probe_command(&mut command, "Axum 服务")?;
    wait_for_server(
        child,
        &format!("http://127.0.0.1:{port}/api/health"),
        Duration::from_secs(45),
        "Axum 服务",
    )?;
    checks.push("Axum 服务启动、健康检查通过并已关闭".to_string());
    Ok(())
}

fn verify_aspnet_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    let dotnet = super::program_path(
        super::env::tool_definition("dotnet").expect("dotnet definition must exist"),
    );
    run_command_with_timeout(
        &dotnet,
        &["build", "--nologo"],
        root,
        Duration::from_secs(240),
        ".NET 构建",
    )?;
    checks.push(".NET 构建通过".to_string());
    let port = available_local_port()?;
    let url = format!("http://127.0.0.1:{port}");
    let mut command = Command::new(dotnet);
    command
        .args(["run", "--no-build", "--urls", url.as_str()])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = spawn_probe_command(&mut command, ".NET 服务")?;
    wait_for_server(
        child,
        &format!("{url}/api/health"),
        Duration::from_secs(45),
        ".NET 服务",
    )?;
    checks.push(".NET 服务启动、健康检查通过并已关闭".to_string());
    Ok(())
}

fn verify_next_project(root: &Path, checks: &mut Vec<String>) -> Result<(), String> {
    let npm = Path::new("npm");
    run_command_with_timeout(
        npm,
        &["install", "--no-audit", "--no-fund"],
        root,
        Duration::from_secs(180),
        "Next.js 依赖安装",
    )?;
    checks.push("Next.js 依赖安装完成".to_string());
    run_command_with_timeout(
        npm,
        &["run", "build"],
        root,
        Duration::from_secs(180),
        "Next.js 构建",
    )?;
    checks.push("Next.js 构建通过".to_string());
    let port = available_local_port()?;
    let port_value = port.to_string();
    let next = root.join("node_modules/.bin/next");
    if !next.is_file() {
        return Err("未找到 Next.js 二进制，无法进行启动检查".to_string());
    }
    let mut command = Command::new(next);
    command
        .args(["dev", "-H", "127.0.0.1", "-p", port_value.as_str()])
        .current_dir(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = spawn_probe_command(&mut command, "Next.js")?;
    wait_for_server(
        child,
        &format!("http://127.0.0.1:{port}/"),
        Duration::from_secs(45),
        "Next.js 服务",
    )?;
    checks.push("Next.js 服务启动、健康检查通过并已关闭".to_string());
    Ok(())
}

fn verify_tauri_project(
    root: &Path,
    project_name: &str,
    checks: &mut Vec<String>,
) -> Result<(), String> {
    verify_vue_project(root, checks)?;
    let tauri_root = root.join("src-tauri");
    run_command_with_timeout(
        Path::new("cargo"),
        &["build"],
        &tauri_root,
        Duration::from_secs(300),
        "Tauri 原生构建",
    )?;
    checks.push("Tauri 原生构建通过".to_string());
    let binary = tauri_root.join("target/debug").join(format!(
        "{}{}",
        project_slug(project_name),
        std::env::consts::EXE_SUFFIX
    ));
    if !binary.is_file() {
        return Err("未找到 Tauri 可执行文件".to_string());
    }
    let mut command = Command::new(binary);
    command
        .current_dir(&tauri_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = spawn_probe_command(&mut command, "Tauri 桌面应用")?;
    verify_desktop_process(child, "Tauri 桌面应用")?;
    checks.push("Tauri 桌面应用短时启动后已关闭".to_string());
    Ok(())
}

fn verify_created_project(
    request: &CreateProjectRequest,
    paths: &[String],
) -> ProjectVerificationResult {
    let mut checks = Vec::new();
    let verification = match request.recommendation.id.as_str() {
        "vue-vite" => paths
            .first()
            .ok_or_else(|| "未找到前端项目路径".to_string())
            .and_then(|path| verify_vue_project(Path::new(path), &mut checks)),
        "vue-spring-boot" => {
            if paths.len() != 2 {
                Err("前后端项目路径不完整".to_string())
            } else {
                verify_vue_project(Path::new(&paths[0]), &mut checks)
                    .and_then(|_| verify_spring_boot_project(Path::new(&paths[1]), &mut checks))
            }
        }
        "node-nestjs" => {
            if paths.len() == 2 {
                verify_vue_project(Path::new(&paths[0]), &mut checks)
                    .and_then(|_| verify_nest_project(Path::new(&paths[1]), &mut checks))
            } else {
                Err("前后端项目路径不完整".to_string())
            }
        }
        "vue-fastapi" => {
            if paths.len() == 2 {
                verify_vue_project(Path::new(&paths[0]), &mut checks)
                    .and_then(|_| verify_fastapi_project(Path::new(&paths[1]), &mut checks))
            } else {
                Err("前后端项目路径不完整".to_string())
            }
        }
        "vue-go" => {
            if paths.len() == 2 {
                verify_vue_project(Path::new(&paths[0]), &mut checks)
                    .and_then(|_| verify_go_project(Path::new(&paths[1]), &mut checks))
            } else {
                Err("前后端项目路径不完整".to_string())
            }
        }
        "vue-axum" => {
            if paths.len() == 2 {
                verify_vue_project(Path::new(&paths[0]), &mut checks)
                    .and_then(|_| verify_axum_project(Path::new(&paths[1]), &mut checks))
            } else {
                Err("前后端项目路径不完整".to_string())
            }
        }
        "vue-aspnet" => {
            if paths.len() == 2 {
                verify_vue_project(Path::new(&paths[0]), &mut checks)
                    .and_then(|_| verify_aspnet_project(Path::new(&paths[1]), &mut checks))
            } else {
                Err("前后端项目路径不完整".to_string())
            }
        }
        "fastapi-api" => paths
            .first()
            .ok_or_else(|| "未找到 API 项目路径".to_string())
            .and_then(|path| verify_fastapi_project(Path::new(path), &mut checks)),
        "go-api" => paths
            .first()
            .ok_or_else(|| "未找到 API 项目路径".to_string())
            .and_then(|path| verify_go_project(Path::new(path), &mut checks)),
        "axum-api" => paths
            .first()
            .ok_or_else(|| "未找到 API 项目路径".to_string())
            .and_then(|path| verify_axum_project(Path::new(path), &mut checks)),
        "aspnet-api" => paths
            .first()
            .ok_or_else(|| "未找到 API 项目路径".to_string())
            .and_then(|path| verify_aspnet_project(Path::new(path), &mut checks)),
        "nextjs" => paths
            .first()
            .ok_or_else(|| "未找到 Next.js 项目路径".to_string())
            .and_then(|path| verify_next_project(Path::new(path), &mut checks)),
        "tauri-vue" => paths
            .first()
            .ok_or_else(|| "未找到 Tauri 项目路径".to_string())
            .and_then(|path| {
                verify_tauri_project(Path::new(path), &request.project_name, &mut checks)
            }),
        _ => {
            return ProjectVerificationResult {
                status: "skipped".to_string(),
                checks,
                detail: "该模板的自动启动自检尚未接入；项目已生成，但不能标记为已验证可启动。"
                    .to_string(),
            }
        }
    };
    match verification {
        Ok(()) => ProjectVerificationResult {
            status: "passed".to_string(),
            checks,
            detail: "构建与短时启动健康检查均已通过，探测进程已关闭。".to_string(),
        },
        Err(error) => ProjectVerificationResult {
            status: "failed".to_string(),
            checks,
            detail: format!("项目已生成，但启动自检未通过：{error}"),
        },
    }
}

/// 用户创建时优先采用官方脚手架；官方源不可用时保留明确的本地兜底结果。
pub fn create_project_with_verification(
    request: &CreateProjectRequest,
) -> Result<CreateProjectResult, String> {
    let mut result = if request.recommendation.id == "vue-spring-boot" {
        match create_official_vue_spring_boot(request) {
            Ok(result) => result,
            Err(error) => {
                let mut fallback = create_project(request)?;
                fallback.message =
                    format!("官方脚手架暂不可用，已使用本地兼容骨架生成。原因：{error}");
                fallback
            }
        }
    } else {
        create_project(request)?
    };
    result.verification = verify_created_project(request, &result.project_paths);
    result.message = format!("{} {}", result.message, result.verification.detail);
    Ok(result)
}
