// 统计模块入口 —— 给前端的"图表 dashboard"和"会话级统计"两条流提供数据。
//
// 子模块：
//   - pricing    模型 → $/token 表 + 成本计算
//   - classifier "By Activity" 13 类分类（codeburn 风格规则）
//   - shell      Bash 工具调用首词 / MCP server 名提取
//   - types      内部 Turn / CallRecord 等中间类型
//   - extract    把一个 JSONL 解析成 Turn 列表（per-agent）
//   - aggregate  把 Turn 流喂进聚合器，吐出前端可见的 AgentStats
//   - stream     start_agent_stats / cancel_stats 的事件流编排
//
// 这一层不直接读 / 写文件 —— 所有 IO 都委托给 agents::source(...) 拿到的
// SessionSource，保持"如何读 JSONL"和"读完之后怎么算"两件事的解耦。
//
// 状态：foundation 已落地（pricing + classifier + shell + types + 公共聚合形状）；
// stream 命令本身在下一批改动里接入，届时本 mod.rs 会暴露 start_agent_stats /
// cancel_stats 两个公共入口。

// foundation 阶段：aggregator / stream 还没接入，下面这些模块对外只通过单元测试
// 使用。allow(dead_code) 让 cargo clippy 不报"未使用"——等 stream.rs 接入后取消。
#![allow(dead_code)]

pub mod aggregate;
pub mod classifier;
pub mod pricing;
pub mod shell;
pub mod stream;
pub mod tray;
pub mod types;
