# 后端运行时与框架选择

先识别工作负载、团队经验和已有生态，再比较语言；不得因当前脚手架偏好而倒推项目需求。

| 候选 | 更适合 | 不应优先的情况 | 常见框架 |
| --- | --- | --- | --- |
| Java/Kotlin | 复杂领域模型、事务一致性、企业集成、长期维护、JVM 团队 | 极轻量脚本、冷启动极敏感且资源紧的函数任务 | Spring Boot、Ktor |
| TypeScript/Node | BFF、I/O API、前后端同语言、快速迭代 | CPU/内存密集核心计算、超长计算任务 | NestJS、Fastify |
| Python | AI 推理、数据处理、科学计算、自动化 | 高吞吐通用业务 API 且没有 Python 团队或异步治理能力 | FastAPI、Django |
| Go | 网关、协议代理、高并发网络服务、云原生基础服务 | 复杂业务规则很多、团队不熟悉 Go 时 | chi、Gin、Fiber |
| Rust | 性能、内存安全、低延迟、系统级能力 | 常规 CRUD 且团队无 Rust 维护能力 | Axum、Actix Web |
| .NET | C# 团队、Windows/Active Directory/Microsoft 云集成 | 团队和部署平台都没有 .NET 基础时 | ASP.NET Core |

## Java 数据访问选择

- MyBatis-Plus：大量标准 CRUD、团队已有 MyBatis 习惯、希望少写基础 Mapper 时采用；复杂 SQL 仍应显式维护。
- MyBatis：SQL 控制、报表/多表查询、遗留数据库兼容要求高时采用。
- JPA/Hibernate：领域模型清晰、聚合关系稳定、团队接受 ORM 生命周期与查询约束时采用。
- jOOQ：SQL-first、类型安全查询和 PostgreSQL 特性价值明显时采用；需接受构建与代码生成成本。

不得把任一种 ORM 作为 Java 服务的默认依赖。数据访问方式应跟随数据模型、查询复杂度、团队经验和已有代码。

