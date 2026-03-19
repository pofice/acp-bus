# Bus Communication & Smart Dispatch Design

Date: 2026-03-19

## Problem

1. **Agent 间通信失败**: `acp-bus-mcp` 二进制不在 agent PATH 中，导致 MCP server 无法启动，agent 没有 `bus_send_message` 工具。Worker 间完全无法通信。
2. **Main agent 两轮低效**: 当前 system prompt 引导 main 分两轮操作（先 /add 创建，再 @mention 派发）。用户期望一轮完成，且 `/add` 命令应支持直接带任务。

## Design

### Part 1: MCP Server 内置化

**现状**: `acp-bus-mcp` 是独立 crate，编译为单独二进制。`session/new` 时通过 `"command": "acp-bus-mcp"` 注册 MCP server。agent 运行时找不到该二进制。

**方案**: 将 MCP server 功能合并到 `acp-bus` 主二进制，新增 `mcp-server` 子命令。

#### 改动

**1. `src/main.rs`** — 新增 `McpServer` 子命令：
```rust
enum Commands {
    Tui { ... },
    Serve { ... },
    Channels { ... },
    /// Run as MCP server for agent bus tools (internal use)
    McpServer,
}
```

`McpServer` 处理逻辑直接复用 `acp-bus-mcp/src/main.rs` 中的代码，读取 `ACP_BUS_SOCKET` 和 `ACP_BUS_AGENT_NAME` 环境变量。

**2. `crates/acp-core/src/client.rs`** — 使用 `current_exe()` 获取自身路径：
```rust
// 改前
"command": "acp-bus-mcp"

// 改后
let exe_path = std::env::current_exe()
    .unwrap_or_else(|_| PathBuf::from("acp-bus"))
    .to_string_lossy()
    .to_string();
// ...
"command": exe_path, "args": ["mcp-server"]
```

需要把 `exe_path` 通过 `AdapterConfig` 传入 `AcpClient::start()`。

具体方案：在 `AdapterConfig` 中新增 `mcp_command: Option<String>` 字段。`app.rs` 启动时通过 `std::env::current_exe()` 获取路径，存入 config。`client.rs` 构建 `mcp_servers` 时使用该路径。

**3. `crates/acp-bus-mcp/`** — 保留但标记 deprecated，后续可删除。

#### 流程变化

```
之前: session/new → spawn "acp-bus-mcp" (找不到) → MCP 工具不可用
之后: session/new → spawn "/path/to/acp-bus mcp-server" (自身) → MCP 工具可用
```

### Part 2: `/add` 命令支持带任务

**现状**: `/add w1 claude` 只创建 agent，不派发任务。

**方案**: 扩展语法为 `/add <name> <adapter> [task...]`

#### 改动

**1. `crates/acp-tui/src/app.rs` `handle_command()`** — `/add` 命令解析：
```
/add w1 claude                    → 创建 w1，无初始任务
/add w1 claude 调研竞品定价策略     → 创建 w1，连接后自动派发任务
```

解析逻辑：parts[0]="/add", parts[1]=name, parts[2]=adapter, parts[3..]=task（join 为字符串）。

**2. 任务派发**: 创建 agent 后启动后台任务，等待 agent 出现在 clients map（复用 `wait_for_agents` 逻辑），然后调用 `do_prompt()` 发送初始任务。

伪代码：
```rust
"/add" => {
    let name = parts[1];
    let adapter = parts[2];
    let task = parts[3..].join(" ");  // 可能为空

    self.start_agent(name, adapter).await;

    if !task.is_empty() {
        // 后台等待 agent ready 并派发
        let ch = self.channel.clone();
        let cl = self.clients.clone();
        tokio::spawn(async move {
            wait_for_agents(&[name], &cl, 30).await;
            // post directed task message
            // do_prompt(name, task, ch, cl)
        });
    }
}
```

**3. `execute_agent_commands()`** — 同样支持 `/add name adapter task...` 格式，使 main agent 也能在回复中一轮创建+派发：

```
main 回复：
/add w1 claude 调研竞品的定价策略，完成后 @main 汇报
/add w2 claude 分析技术架构可行性，完成后 @main 汇报
```

这比 @mention 方式更直接——不需要两轮交互。

### Part 3: System Prompt 优化

**Main agent prompt 改动**:

1. 示例改为一轮（/add 带任务）：
```
/add w1 claude 调研 X，完成后 @main 汇报
/add w2 claude 调研 Y，完成后 @main 汇报
```

2. 强调一轮完成原则：
```
重要：创建 agent 时必须同时指定任务。不要空创建再二次派发。
```

3. 保留 @mention 用于后续轮次的任务追加（agent 已存在的情况）。

**Worker agent prompt** — 不变，已经够简洁。

## Implementation Order

1. **`src/main.rs`**: 添加 `McpServer` 子命令，内联 MCP server 逻辑
2. **`AdapterConfig`**: 添加 `mcp_command` 字段
3. **`app.rs`**: 启动时获取 `current_exe()` 并传入 config
4. **`client.rs`**: 构建 `mcp_servers` 时使用 `mcp_command`
5. **`app.rs`**: `/add` 命令解析扩展（带任务）
6. **`execute_agent_commands()`**: 支持 `/add name adapter task` 格式
7. **`adapter.rs`**: 更新 main agent system prompt
8. **测试**: 验证 MCP server 启动、bus_send_message 可用、一轮派发

## Files Affected

- `src/main.rs` — 新增 McpServer 子命令
- `crates/acp-core/src/adapter.rs` — AdapterConfig + system prompt
- `crates/acp-core/src/client.rs` — mcp_servers 构建逻辑
- `crates/acp-tui/src/app.rs` — /add 命令扩展 + execute_agent_commands
