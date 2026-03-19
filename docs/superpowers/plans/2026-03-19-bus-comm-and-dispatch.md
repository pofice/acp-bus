# Bus Communication & Smart Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix agent-to-agent communication by embedding the MCP server into the main binary, and enable one-round create+dispatch via `/add name adapter task`.

**Architecture:** The MCP server logic from `acp-bus-mcp` gets inlined into `src/main.rs` as a `mcp-server` subcommand. `client.rs` uses `current_exe()` to spawn itself as the MCP server, guaranteeing discoverability. The `/add` command and `execute_agent_commands()` are extended to accept trailing task text, which is dispatched after the agent completes its handshake. `start_agent_bg()` gains `socket_path` and `mcp_command` parameters to fix the existing bug where dynamically-created agents have no bus tools.

**Tech Stack:** Rust, tokio, serde_json, clap, Unix sockets

**Spec:** `docs/superpowers/specs/2026-03-19-bus-comm-and-dispatch-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/main.rs` | Modify | Add `McpServer` subcommand with inlined MCP logic |
| `crates/acp-core/src/adapter.rs` | Modify | Add `mcp_command` field to `AdapterConfig`, update system prompt |
| `crates/acp-core/src/client.rs` | Modify | Use `mcp_command` when building `mcp_servers` in session/new |
| `crates/acp-tui/src/app.rs` | Modify | Fix `start_agent_bg` socket_path bug, extend `/add` command, extend `execute_agent_commands` |

---

### Task 1: Add `mcp-server` Subcommand

Embed the MCP server logic directly into the main binary so agents can always find it.

**Files:**
- Modify: `src/main.rs:11-34` (Commands enum + match arm)

- [ ] **Step 1: Add McpServer variant to Commands enum**

In `src/main.rs`, add to the `Commands` enum after `Channels`:

```rust
    /// Internal: run as MCP server for agent bus tools
    #[command(name = "mcp-server")]
    McpServer,
```

- [ ] **Step 2: Add the match arm with inlined MCP logic**

In `src/main.rs`, add a new match arm in `main()` after `Commands::Channels`:

```rust
        Commands::McpServer => {
            // Inlined from acp-bus-mcp: read env vars, process JSON-RPC over stdio
            let socket_path = std::env::var("ACP_BUS_SOCKET").unwrap_or_default();
            let agent_name =
                std::env::var("ACP_BUS_AGENT_NAME").unwrap_or_else(|_| "unknown".into());

            let stdin = tokio::io::stdin();
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let mut lines = BufReader::new(stdin).lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let msg: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let id = msg.get("id").cloned();
                let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");

                let result = match method {
                    "initialize" => serde_json::json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "acp-bus-mcp", "version": "0.1.0" }
                    }),
                    "notifications/initialized" => continue,
                    "tools/list" => serde_json::json!({
                        "tools": [
                            {
                                "name": "bus_send_message",
                                "description": "Send a message to another agent in the acp-bus channel. Use this to communicate directly with other agents.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "to": { "type": "string", "description": "Target agent name" },
                                        "content": { "type": "string", "description": "Message content" }
                                    },
                                    "required": ["to", "content"]
                                }
                            },
                            {
                                "name": "bus_list_agents",
                                "description": "List all agents in the acp-bus channel with their status",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {}
                                }
                            }
                        ]
                    }),
                    "tools/call" => {
                        let params = msg.get("params").cloned().unwrap_or(serde_json::json!({}));
                        let tool_name =
                            params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let args =
                            params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
                        let resp =
                            mcp_call_socket(&socket_path, &agent_name, tool_name, &args).await;
                        serde_json::json!({ "content": [{ "type": "text", "text": resp }] })
                    }
                    _ => {
                        let resp = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32601, "message": format!("method not found: {method}") }
                        });
                        let mut stdout = std::io::stdout().lock();
                        let _ = serde_json::to_writer(&mut stdout, &resp);
                        let _ = std::io::Write::write_all(&mut stdout, b"\n");
                        let _ = std::io::Write::flush(&mut stdout);
                        continue;
                    }
                };

                let resp = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
                let mut stdout = std::io::stdout().lock();
                let _ = serde_json::to_writer(&mut stdout, &resp);
                let _ = std::io::Write::write_all(&mut stdout, b"\n");
                let _ = std::io::Write::flush(&mut stdout);
            }
        }
```

- [ ] **Step 3: Add the `mcp_call_socket` helper function**

Add this function before `main()` in `src/main.rs`:

```rust
async fn mcp_call_socket(
    socket_path: &str,
    agent_name: &str,
    tool: &str,
    args: &serde_json::Value,
) -> String {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    if socket_path.is_empty() {
        return r#"{"error":"ACP_BUS_SOCKET not set"}"#.to_string();
    }

    let req = match tool {
        "bus_send_message" => {
            let to = args.get("to").and_then(|v| v.as_str()).unwrap_or("");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            serde_json::json!({ "type": "send_message", "from": agent_name, "to": to, "content": content })
        }
        "bus_list_agents" => {
            serde_json::json!({ "type": "list_agents", "from": agent_name })
        }
        _ => return r#"{"error":"unknown tool"}"#.to_string(),
    };

    let stream = match UnixStream::connect(socket_path).await {
        Ok(s) => s,
        Err(e) => return format!(r#"{{"error":"connect failed: {e}"}}"#),
    };

    let (reader, mut writer) = stream.into_split();
    let mut line = req.to_string();
    line.push('\n');
    if writer.write_all(line.as_bytes()).await.is_err() {
        return r#"{"error":"write failed"}"#.to_string();
    }
    let _ = writer.shutdown().await;

    let mut lines = BufReader::new(reader).lines();
    match lines.next_line().await {
        Ok(Some(resp)) => resp,
        Ok(None) => r#"{"error":"no response"}"#.to_string(),
        Err(e) => format!(r#"{{"error":"read failed: {e}"}}"#),
    }
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build 2>&1`
Expected: compiles without errors

- [ ] **Step 5: Smoke-test the subcommand**

Run: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run -- mcp-server 2>/dev/null`
Expected: JSON response with `bus_send_message` and `bus_list_agents` tool definitions

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: embed MCP server as mcp-server subcommand

Inlines the acp-bus-mcp MCP server logic into the main binary
so agents can always find it via current_exe()."
```

---

### Task 2: Add `mcp_command` to AdapterConfig and Wire Through

Make the MCP command path configurable so `client.rs` can use the correct binary path.

**Files:**
- Modify: `crates/acp-core/src/adapter.rs:4-18` (AdapterConfig struct)
- Modify: `crates/acp-core/src/adapter.rs:195-207` (get() return)
- Modify: `crates/acp-core/src/client.rs:277-289` (mcp_servers construction)
- Modify: `crates/acp-tui/src/app.rs:645-672` (App::start_agent)
- Modify: `crates/acp-tui/src/app.rs:1093-1136` (start_agent_bg)

- [ ] **Step 1: Add field to AdapterConfig**

In `crates/acp-core/src/adapter.rs`, add after `socket_path` field (line 17):

```rust
    /// Full path to acp-bus binary for MCP server (set at runtime)
    pub mcp_command: Option<String>,
```

And in the `get()` function return (line 206), add:

```rust
        socket_path: None,
        mcp_command: None,
```

- [ ] **Step 2: Update client.rs to use mcp_command**

In `crates/acp-core/src/client.rs`, replace lines 277-289:

```rust
        let mcp_servers = if let Some(ref sp) = client.adapter.socket_path {
            let mcp_cmd = client.adapter.mcp_command.as_deref().unwrap_or("acp-bus-mcp");
            serde_json::json!([{
                "name": "acp-bus",
                "command": mcp_cmd,
                "args": if mcp_cmd.contains("acp-bus") && !mcp_cmd.contains("acp-bus-mcp") {
                    serde_json::json!(["mcp-server"])
                } else {
                    serde_json::json!([])
                },
                "env": [
                    { "name": "ACP_BUS_SOCKET", "value": sp },
                    { "name": "ACP_BUS_AGENT_NAME", "value": &agent_name }
                ]
            }])
        } else {
            serde_json::json!([])
        };
```

- [ ] **Step 3: Set mcp_command in App::start_agent**

In `crates/acp-tui/src/app.rs`, the `start_agent` method (line 645). After `config.socket_path = socket_path;` (line 672), add:

```rust
            config.mcp_command = mcp_command;
```

This requires adding `mcp_command` to the captured variables. At line 650, add:

```rust
        let mcp_command = self.mcp_command.clone();
```

And add the `mcp_command` field to App struct. In the App struct definition, add:

```rust
    mcp_command: Option<String>,
```

In `App::new()`, initialize it:

```rust
            mcp_command: std::env::current_exe()
                .ok()
                .map(|p| p.to_string_lossy().to_string()),
```

- [ ] **Step 4: Fix start_agent_bg — add socket_path and mcp_command parameters**

This is the critical bug fix. `start_agent_bg` currently doesn't pass `socket_path` or `mcp_command` to the config, so dynamically-created agents have no MCP tools.

Change the `start_agent_bg` signature in `crates/acp-tui/src/app.rs` (line 1093):

```rust
async fn start_agent_bg(
    name: String,
    adapter_name: String,
    channel: Arc<Mutex<Channel>>,
    clients: ClientMap,
    bus_tx: Option<mpsc::UnboundedSender<BusEvent>>,
    socket_path: Option<String>,
    mcp_command: Option<String>,
) {
```

Inside the function, after `let config = match adapter::get(...)` (line 1124), add:

```rust
        let mut config = config;
        config.socket_path = socket_path;
        config.mcp_command = mcp_command;
```

- [ ] **Step 5: Update all call sites of start_agent_bg**

In `execute_agent_commands()` (line 1043), the call needs the extra params. Change the function signature to accept them:

```rust
async fn execute_agent_commands(
    reply: &str,
    channel: &Arc<Mutex<Channel>>,
    clients: &ClientMap,
    bus_tx: Option<mpsc::UnboundedSender<BusEvent>>,
    socket_path: Option<String>,
    mcp_command: Option<String>,
) -> Vec<String> {
```

And pass them through to `start_agent_bg`:

```rust
                    start_agent_bg(
                        agent_name.clone(),
                        adapter_name,
                        channel.clone(),
                        clients.clone(),
                        bus_tx.clone(),
                        socket_path.clone(),
                        mcp_command.clone(),
                    )
```

Update the call site in `do_prompt_inner` (line 908) — currently passes `None` for bus_tx, needs all params:

```rust
                let added_agents = execute_agent_commands(
                    &reply, &channel, &clients, bus_tx.clone(),
                    socket_path.clone(), mcp_command.clone(),
                ).await;
```

This means `do_prompt_inner` / `do_prompt` needs these params too. Add `socket_path: Option<String>` and `mcp_command: Option<String>` to `do_prompt` and `do_prompt_inner` signatures, and pass them from all call sites.

- [ ] **Step 6: Build and run tests**

Run: `cargo build 2>&1 && cargo test --workspace 2>&1`
Expected: compiles, all 47 tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/acp-core/src/adapter.rs crates/acp-core/src/client.rs crates/acp-tui/src/app.rs
git commit -m "feat: wire mcp_command through AdapterConfig to client

Fixes critical bug where dynamically-created agents (via main's /add)
had no socket_path or mcp_command, making MCP bus tools unavailable.
Uses current_exe() to ensure the MCP server binary is always findable."
```

---

### Task 3: Extend `/add` Command to Support Inline Tasks

Enable `/add w1 claude task text here` to create an agent and immediately dispatch a task.

**Files:**
- Modify: `crates/acp-tui/src/app.rs:521-538` (handle_command /add branch)
- Modify: `crates/acp-tui/src/app.rs:1022-1072` (execute_agent_commands)

- [ ] **Step 1: Change splitn limit in handle_command**

In `crates/acp-tui/src/app.rs`, line 522, change the split to capture more parts:

```rust
    async fn handle_command(&mut self, text: &str) {
        let parts: Vec<&str> = text.splitn(4, ' ').collect();
```

- [ ] **Step 2: Extend /add branch with task dispatch**

Replace the `/add` branch (lines 526-538) with:

```rust
            "/add" => {
                if parts.len() < 3 {
                    let mut ch = self.channel.lock().await;
                    ch.post(
                        "系统",
                        "用法: /add <name> <adapter> [task]\n可用 adapters: claude, c1, c2, gemini, codex",
                        true,
                    );
                    return;
                }
                let name = parts[1].to_string();
                let adapter_name = parts[2].to_string();
                let task = if parts.len() >= 4 {
                    Some(parts[3].to_string())
                } else {
                    None
                };
                self.start_agent(name.clone(), adapter_name).await;

                if let Some(task) = task {
                    let ch = self.channel.clone();
                    let cl = self.clients.clone();
                    tokio::spawn(async move {
                        wait_for_agents(&[name.clone()], &cl, 30).await;
                        {
                            let mut chan = ch.lock().await;
                            chan.post_directed("用户", &name, &task,
                                MessageKind::Task, MessageTransport::Ui, MessageStatus::Delivered);
                        }
                        do_prompt(name, task, ch, cl).await;
                    });
                }
            }
```

Note: `do_prompt` signature may have changed in Task 2 — add the extra params (`socket_path`, `mcp_command`) as needed.

- [ ] **Step 3: Extend execute_agent_commands for /add with task**

In `execute_agent_commands()`, change the split (line 1034) from `splitn(3, ' ')` to `splitn(4, ' ')`:

```rust
            let parts: Vec<&str> = trimmed.splitn(4, ' ').collect();
```

After the `start_agent_bg` call and `added.push(agent_name)` (line 1051), collect the task:

```rust
                if !exists {
                    start_agent_bg(
                        agent_name.clone(),
                        adapter_name,
                        channel.clone(),
                        clients.clone(),
                        bus_tx.clone(),
                        socket_path.clone(),
                        mcp_command.clone(),
                    )
                    .await;
                    added.push(agent_name.clone());

                    // If /add has inline task, dispatch after agent connects
                    if parts.len() >= 4 {
                        let task = parts[3].to_string();
                        let ch = channel.clone();
                        let cl = clients.clone();
                        tokio::spawn(async move {
                            wait_for_agents(&[agent_name.clone()], &cl, 30).await;
                            {
                                let mut chan = ch.lock().await;
                                chan.post_directed("main", &agent_name, &task,
                                    MessageKind::Task, MessageTransport::MentionRoute,
                                    MessageStatus::Delivered);
                            }
                            do_prompt(agent_name, task, ch, cl).await;
                        });
                    }
                }
```

Again, adjust `do_prompt` call to match updated signature from Task 2.

- [ ] **Step 4: Build and run tests**

Run: `cargo build 2>&1 && cargo test --workspace 2>&1`
Expected: compiles, all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/acp-tui/src/app.rs
git commit -m "feat: /add command supports inline task dispatch

/add w1 claude task text — creates agent and dispatches task after
handshake completes. Works in both TUI command and agent output parsing."
```

---

### Task 4: Update Main Agent System Prompt

Change the system prompt to teach one-round create+dispatch.

**Files:**
- Modify: `crates/acp-core/src/adapter.rs:222-248` (main agent prompt)
- Modify: `crates/acp-core/src/adapter.rs:298-308` (bus_system_prompt test)

- [ ] **Step 1: Replace main agent system prompt**

In `crates/acp-core/src/adapter.rs`, replace the main agent prompt (lines 222-248) with:

```rust
    if is_main {
        format!(
            r#"你是 {agent_name}，运行在 acp-bus 多 agent 协作 TUI 中（频道 {channel}）。

你可以在回复文本中直接写命令来调度 agent，TUI 自动解析执行：
- /add <名字> <adapter> <任务> — 创建 agent 并立即派发任务（adapter: claude, c1, c2, gemini, codex）
- @<名字> <消息> — 给已存在的 agent 发消息
- /remove <名字> — 移除 agent

重要：创建 agent 时必须同时指定任务。不要空创建再二次派发。

示例回复（一轮完成创建+派发）：
/add w1 claude 调研 X，完成后 @{agent_name} 汇报
/add w2 claude 调研 Y，完成后 @{agent_name} 汇报

后续追加任务（agent 已存在时用 @mention）：
@w1 再补充一下 Z 的部分
@w2 把结果整理成表格

判断标准：可拆分的任务一律派发，只有单步简单操作才自己做。

agent 也可以用 acp-bus MCP 工具互相通信：
- `bus_send_message` — 给其他 agent 发私聊消息
- `bus_list_agents` — 列出频道 agent 及状态"#,
        )
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p acp-core 2>&1`
Expected: all tests pass (the `bus_system_prompt` test checks for agent name and channel, both still present)

- [ ] **Step 3: Commit**

```bash
git add crates/acp-core/src/adapter.rs
git commit -m "feat: update main agent prompt for one-round create+dispatch

Teaches main to use /add w1 claude task instead of two-round
create-then-dispatch. Simplifies MCP tool description."
```

---

### Task 5: Integration Verification

**Files:**
- No new files. Manual verification.

- [ ] **Step 1: Full build and test**

Run: `cargo build 2>&1 && cargo test --workspace 2>&1`
Expected: compiles, all tests pass

- [ ] **Step 2: Verify MCP server subcommand works**

Run: `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run -- mcp-server 2>/dev/null`
Expected: JSON with `bus_send_message` and `bus_list_agents` tools

- [ ] **Step 3: Verify current_exe path is reasonable**

Run: `cargo run -- mcp-server <<< '{"jsonrpc":"2.0","id":1,"method":"initialize"}' 2>/dev/null`
Expected: JSON response with `protocolVersion`

- [ ] **Step 4: Final commit (if any fixups needed)**

```bash
git add -A
git commit -m "fix: integration fixups for bus comm and dispatch"
```
