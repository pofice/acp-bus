# acp-bus Roadmap

## Phase 1：稳定性与可靠性 ✅

- [x] ACP 协议握手 + 会话管理
- [x] MCP Server 内置化（解决 binary 发现问题）
- [x] Agent 间通信（bus_send_message / bus_list_agents）
- [x] 多行任务解析
- [x] 消息自动滚动（考虑 word wrap）
- [x] Scheduler 串行化 Main Agent 消息（防止消息风暴）
- [x] 原子写入、UTF-8 安全、进程管理修复
- [x] 强制终止：Ctrl+C 立即 SIGKILL 所有 Agent 进程组，不再卡死
- [x] 任务中断：Ctrl+Q 取消选中 Agent 的当前 prompt
- [x] 自发消息防护：禁止 Agent 通过 bus 给自己发消息（防死循环）
- [x] 用户直接对话：@worker 时 worker 直接回复，不再绕道 @main
- [x] Sidebar 工具调用实时展示（工具名 + 计时器）
- [x] 消息过滤：Agent tab 显示 @mention 该 Agent 的广播消息

## Phase 2：通信增强

- [ ] **消息投递确认**：Worker 收到消息后的 ACK 机制，确认对方真的收到并处理了
- [ ] **消息历史查询**：Agent 可以查询与特定 Agent 的历史对话
- [ ] **广播消息**：`bus_broadcast` 一对多通知，用于全局协调
- [ ] **消息优先级**：紧急消息插队，避免被队列阻塞
- [ ] **断线重连**：Agent 崩溃后自动重启并恢复会话上下文

## Phase 3：智能调度

- [ ] **动态负载均衡**：根据 Agent 忙闲自动分配任务
- [ ] **任务依赖图**：声明式定义任务间的依赖关系（A 完成后才启动 B）
- [ ] **超时与重试**：任务级别的超时监控和自动重试
- [ ] **Agent 池化**：预创建 Agent 池，避免每次创建的握手开销
- [ ] **成本追踪**：记录每个 Agent 的 token 消耗和 API 调用量

## Phase 4：可观测性

- [ ] **实时 Dashboard**：Agent 状态、消息流量、任务进度的可视化
- [ ] **通信拓扑图**：实时展示 Agent 间的消息流向
- [ ] **会话回放**：从快照回放历史会话，复盘协作过程
- [ ] **结构化日志**：OpenTelemetry 集成，分布式追踪

## Phase 5：生态集成

- [ ] **IDE 插件**：Neovim / VS Code 内嵌 Agent 面板
- [ ] **API 模式**：HTTP/WebSocket API，支持外部系统调用
- [ ] **Agent 模板市场**：预定义角色模板（代码审查员、测试工程师、文档撰写等）
- [ ] **多模型混合调度**：同一任务链中混用 Claude / Gemini / Codex，按特长分配
- [ ] **持久化频道**：频道持久化 + Agent 热加载，支持长期运行的项目频道

## Phase 6：高级协作模式

- [ ] **层级团队**：Team Lead 可以创建 Sub-Team Lead，形成树状管理结构
- [ ] **投票决策**：多个 Agent 对方案投票，少数服从多数
- [ ] **知识共享**：Agent 间共享发现的知识和中间结果，避免重复工作
- [ ] **代码冲突检测**：多个 Agent 同时修改代码时自动检测和解决冲突
- [ ] **Swarm 模式**：去中心化协作，无 Main Agent，Agent 自组织
