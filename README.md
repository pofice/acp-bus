# acp-bus

多 AI Agent 协作总线 — 让 AI 组团干活。

acp-bus 是一个基于 ACP (Agent Communication Protocol) 的多 Agent 协作系统，通过 TUI 界面让用户以"团队管理"的方式调度多个 AI Agent 并行工作、互相通信、自主协作。

## 核心理念

**不是让一个 AI 更强，而是让多个 AI 像团队一样协作。**

- **Main Agent = Team Lead**：理解需求、拆解任务、组建团队、质量把关
- **Worker Agent = 全能工程师**：拥有完整工具链，可自主使用 subagent、superpowers 技能、团队通信
- **Bus = 通信总线**：Agent 间通过 `bus_send_message` 直接点对点通信，无需经过 Main 中转

## 安装与运行

### 前置条件

- **Rust 工具链**：stable（见 `rust-toolchain.toml`）
- **至少一个 ACP Agent**：如 [Claude Code](https://claude.ai/code)（提供 `claude-agent-acp` 命令）

### 从源码构建

```bash
git clone https://github.com/crosery/acp-bus.git
cd acp-bus
cargo build --release
```

### 安装到系统

```bash
cargo install --path .
```

### 运行

```bash
# 方式一：cargo run
cargo run -- tui                       # 在当前目录启动 TUI
cargo run -- tui --cwd /your/project   # 指定工作目录

# 方式二：安装后直接运行
acp-bus tui
acp-bus tui --cwd /your/project

# 其他命令
acp-bus channels                       # 查看已保存的会话快照
acp-bus serve --stdio                  # JSON-RPC server 模式（Neovim 集成）
```

### 环境变量

```bash
RUST_LOG=debug acp-bus tui             # 开启 debug 日志（输出到 stderr）
```

如需使用多 API 线路或代理，在 `~/.env` 或 `~/.config/nvim/.env` 中配置：

```bash
# Claude API 线路
CLAUDE_API1_BASE_URL=https://your-api1.example.com
CLAUDE_API1_TOKEN=your-token

# 代理
CLAUDE_PROXY=http://proxy:port
```

## 操作指南

### 命令

```
/add w1 claude 你的任务是...       # 创建 Agent 并派发任务
@w1 补充一下 X 部分               # 给已有 Agent 发消息
/remove w1                        # 移除 Agent
/list                             # 查看所有 Agent
/save                             # 保存会话快照
/cancel w1                        # 取消 Agent 当前任务
```

### 快捷键

```
Ctrl+c                            # 退出（立即终止所有 Agent）
Ctrl+q                            # 中断选中 Agent 的当前任务
Ctrl+j/k                          # 上下滚动消息
Ctrl+d/u                          # 快速翻页（10行）
Ctrl+n/p                          # 切换 Agent 标签页
鼠标滚轮                          # 滚动消息
```

## 架构

```
                         ┌─────────┐
                         │   You   │
                         └────┬────┘
                              │ @mention / 直接对话
                    ┌─────────┴─────────┐
                    │    acp-bus TUI     │
                    │  ┌─────┐ ┌─────┐  │
                    │  │Input│ │ Msgs│  │
                    │  └──┬──┘ └──┬──┘  │
                    └─────┼───────┼─────┘
                          │       │
          ┌───────────────┼───────┼───────────────┐
          │            Router (@mention)           │
          └───┬───────────┼───────────────┬───────┘
              │           │               │
        ┌─────┴─────┐ ┌──┴────┐ ┌────────┴────┐
        │   Main    │ │Worker1│ │   Worker2    │
        │  (Leader) │ │(Claude│ │(Gemini/Codex)│
        │ claude    │ │  c1)  │ │              │
        └─────┬─────┘ └──┬───┘ └──────┬───────┘
              │           │            │
              │     ACP Protocol (stdio JSON-RPC)
              │           │            │
        ┌─────┴───────────┴────────────┴───────┐
        │         Bus (Unix Socket + MCP)       │
        │                                       │
        │  bus_send_message  bus_list_agents     │
        │  (Agent 间点对点通信，无需经过 Main)     │
        └──────────┬───────────────┬────────────┘
                   │               │
            ┌──────┴──┐     ┌──────┴──┐
            │Scheduler│     │  Store   │
            │(串行队列)│     │(JSON 快照)│
            └─────────┘     └─────────┘
```

### Crate 结构

```
acp-protocol  ← 纯类型，JSON-RPC 2.0 + ACP 协议定义
acp-core      ← 核心逻辑：Channel、Agent、Router、Scheduler、Client、Store
acp-server    ← Neovim 集成（JSON-RPC over stdio）
acp-tui       ← ratatui TUI 界面
```

依赖方向：`acp-protocol ← acp-core ← acp-server / acp-tui ← main`

### 通信机制

**Agent <-> Bus（ACP 协议）**

```
initialize → authenticate? → session/new → session/prompt
                                              ↓
                                        session/update (streaming)
                                              ↓
                                     reverse requests (fs/*, terminal/*)
```

**Agent <-> Agent（MCP 工具）**

```
Agent A → bus_send_message(to: "B", content: "...") → Bus → prompt Agent B
```

### 智能派发

```
用户: "帮我调研 X 和 Y"

Main Agent 回复:
/add w1 claude 你是技术调研专家。
任务：调研 X 的最新进展
输出：结构化报告
完成后 @main 汇报

/add w2 claude 你是市场分析师。
任务：分析 Y 的竞争格局
输出：对比表格
完成后 @main 汇报

TUI 自动解析 → 创建 w1, w2 → 等待连接 → 派发任务
```

## 支持的 Adapter

| Adapter | 命令 | 说明 |
|---------|------|------|
| `claude` | claude-agent-acp | Claude Code (Anthropic) |
| `c1` | claude-agent-acp | Claude Code API 线路 1 |
| `c2` | claude-agent-acp | Claude Code API 线路 2 |
| `gemini` | gemini --yolo --acp | Gemini CLI (Google)，启动较慢（~25s），API 429 时静默无响应 |
| `codex` | codex-acp | Codex CLI (OpenAI) |

连通性测试：`cargo test -p acp-core --test real_agent_connectivity -- --ignored`

## 开发

```bash
cargo build                        # 构建
cargo test                         # 测试
cargo test -p acp-core             # 测试单个 crate
RUST_LOG=debug cargo run -- tui    # Debug 模式（日志输出到 stderr）
```

## Roadmap

详见 [docs/roadmap.md](docs/roadmap.md)

## License

MIT
