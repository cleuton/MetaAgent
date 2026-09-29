# Feature Specification: Metagente Core (MVP)

**Feature Branch**: `001-metagente-core`

**Created**: 2026-09-05

**Status**: Draft

**Input**: User description: "Create an interpreted programming language for building AI agents. It must use MCP and A2A and ship with some built in tools (file access, for example). It must be a 4GL, simple, easy to use even for a beginner. The interpreter must be written in Rust. It must be able to invoke other agents built with it through dynamic link. The MVP must be able to invoke tools and execute tasks."

## MVP Scope

The first release of Metagente is a Minimum Viable Product whose core, non negotiable capability is: **an agent can invoke tools and execute tasks through them**. Everything else in this spec (built in tools, MCP, A2A, dynamic link) exists to serve that one capability. A requirement that does not directly support "invoke a tool, execute a task" is out of scope for the MVP and moves to a later spec.

**Release staging**: the MVP release is User Stories 1 and 2 (tools, tasks, and beginner friendly errors). Version 1.0 of the language is complete only when all five user stories ship, because MCP and A2A interoperability is a core, non removable part of the language (constitution Principle III). Stories 3 to 5 are built in priority order but none is dropped.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run a first agent that uses tools (Priority: P1)

A person with no programming experience writes a short agent file that declares a goal and the tools it may use (a built in tool such as file access, and an external MCP tool), runs it, and the agent invokes the tools, executes the task, and answers correctly. Example: an agent that answers questions about the weather using the built in HTTP tool and an external MCP weather tool.

**Why this priority**: This is the MVP's reason to exist. Without tool invocation and task execution, nothing else has value.

**Independent Test**: Write one agent file declaring one built in tool and one external MCP tool, run it, and confirm both tools are invoked and the task result is returned, with no code beyond declaring and calling the tools.

**Acceptance Scenarios**:

1. **Given** a valid agent file that declares the file tool, **When** the agent asks to read a local file, **Then** the file's content is made available to the agent without the user writing any manual I/O code.
2. **Given** an agent that declares a connection to an external MCP server, **When** the agent invokes a tool exposed by that server, **Then** the interpreter discovers the tool, validates the parameters against the schema the server provides, executes the call, and returns the result to the agent.
3. **Given** a minimal agent (receives a message, calls a tool, executes a task, replies), **When** it is written, **Then** it fits in fewer than 10 lines.
4. **Given** an agent that declares the clock tool, **When** it asks for the current time and then waits a number of seconds, **Then** it receives the time and continues after the wait.
5. **Given** a language model provider chosen in the configuration file, **When** an agent reasons with it, **Then** the agent works without any provider being named in the agent file, and changing the configuration changes the provider with no edit to the agent.
6. **Given** a language model that keeps asking to use tools, **When** the step limit (default 10) is reached, **Then** the agent stops and the message says how many steps were used and how to raise the limit.

---

### User Story 2 - Understandable errors and a beginner friendly start (Priority: P1)

A beginner follows the getting started tutorial, writes their first agent, and runs it in under 15 minutes. When they make a mistake, the message is in plain language, points at the exact line, and suggests a fix.

**Why this priority**: Simplicity is the language's defining promise (constitution Principle I); a working agent that beginners cannot write or debug fails the product goal.

**Independent Test**: Give a first time user the tutorial and time them to a working agent; separately, introduce a syntax error and a runtime error and check the messages.

**Acceptance Scenarios**:

1. **Given** a beginner with no prior experience, **When** they follow the tutorial, **Then** they run a working agent in under 15 minutes without needing to understand Rust, network protocols, or asynchronous programming.
2. **Given** a syntax or runtime error in an agent file, **When** the interpreter hits the error, **Then** the message is in natural language, names the exact line, suggests a fix, and exposes no internal interpreter details.

---

### User Story 3 - Safe by default permissions (Priority: P2)

An agent can only use the built in tools (file, network, environment) it explicitly declared. Anything else is refused with a clear message.

**Why this priority**: Required for trust when running agents written by beginners, and a constitution constraint, but the core loop works without it.

**Independent Test**: Run an agent that does not declare the file tool and attempts to read a file; confirm it is refused with a clear message.

**Acceptance Scenarios**:

1. **Given** an agent that did not declare a built in tool, **When** it tries to use that tool, **Then** the attempt is refused and the message names the missing declaration and how to add it.
2. **Given** an agent that declared only some environment variables by name, **When** it reads a variable it did not declare, or the variable that holds the language model key, **Then** the read is refused with a clear message.

---

### User Story 4 - One agent invokes another (dynamic link) (Priority: P2)

A second agent, on a more complex question, invokes the first agent by name or path at runtime to get its result before replying.

**Why this priority**: It is how agents compose, and it serves task execution, but a single agent already delivers the MVP.

**Independent Test**: Create two agent files; have Agent A call Agent B by name; change Agent B and confirm Agent A picks up the change without being modified.

**Acceptance Scenarios**:

1. **Given** two agents in the same project, **When** Agent A references Agent B by name, **Then** the interpreter locates, loads, and executes Agent B on demand and returns the result to Agent A, without Agent A knowing Agent B's internals.
2. **Given** Agent B does not accept the message Agent A sends, **When** the call is attempted, **Then** it is rejected before execution with a natural language message describing the mismatch.
3. **Given** Agent A calls B and B calls A, **When** the cycle is detected, **Then** the interpreter stops and reports the cycle clearly instead of looping.

---

### User Story 5 - Interoperate with the outside world (MCP server and A2A) (Priority: P3)

A Metagente agent can expose its own tools to other MCP clients, publish an Agent Card, receive tasks from external A2A agents, and send tasks to external A2A agents.

**Why this priority**: Core to the language identity (constitution Principle III), but the MVP's proof point is tool invocation from the agent's own side.

**Independent Test**: Point an external A2A compatible agent at a published Agent Card and send it a task; connect an external MCP client to an agent and list its tools.

**Acceptance Scenarios**:

1. **Given** an agent that publishes its Agent Card, **When** an external A2A compatible agent discovers the card and sends a task, **Then** the Metagente agent receives, processes, and responds following the A2A protocol.
2. **Given** an agent that exposes tools, **When** an external MCP client connects, **Then** it can discover and invoke those tools.
3. **Given** a message arrives through A2A that matches no task the agent handles, **When** it is received, **Then** the sender gets a clear "not supported" response.
4. **Given** 50 simultaneous requests sent to a served agent, **When** they are processed, **Then** all of them complete correctly, and the agent author wrote nothing to enable this.
5. **Given** agents served without the public option, **When** another machine tries to connect, **Then** the connection is refused; **and when** the public option is given, **Then** connections are accepted and a warning states that there is no authentication.

---

### Edge Cases

- A dynamic link call targets an agent that does not exist or has moved: a clear message names the missing agent and where it was looked for.
- An external MCP server becomes unavailable during a tool call or task: the agent receives a clear failure result and the interpreter does not crash.
- A task runs longer than expected, times out, or fails partway: the agent gets a clear timeout or failure result it can react to.
- Two agents call each other in a cycle: detected and reported (see User Story 4).
- An agent uses a tool it did not declare: refused (see User Story 3).
- An A2A message matches no known task: clear "not supported" response (see User Story 5).

## Glossary

- **Tool call**: one use of a tool by an agent, for example `file.read path: "a.txt"`.
- **Task**: a tool call together with its result. The A2A protocol has its own "task" (a unit of work sent between agents); this spec writes "A2A task" for it.
- **Task list**: the implementation to-do list in `tasks.md`; always called the "task list" in this document.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST run agent source files (extension `.ag`) directly, with no separate compilation step visible to the user.
- **FR-002**: The language MUST provide a first class construct for declaring an agent, including a goal, a set of enabled tools, and message or task handlers.
- **FR-003**: An agent MUST be able to invoke at least one tool and execute at least one task end to end, covering both a built in tool and an external MCP tool.
- **FR-004**: The system MUST ship, with no additional installation, these built in tools: file access (read and write), HTTP requests, environment variable reads, a simple per agent state store, and a basic clock (current time and waiting).
- **FR-005**: The system MUST act as an MCP client: connect to external MCP servers, discover their tools, and invoke them, including tools that take a while to finish, as long as they finish within the time limit of FR-016.
- **FR-006**: The system MUST be able to expose an agent's tools to other MCP clients (MCP server role).
- **FR-007**: The system MUST publish an Agent Card, receive tasks from external A2A compatible agents, and send tasks to external A2A agents.
- **FR-008**: The language MUST let an agent invoke another Metagente agent by name or path at runtime, with no need to modify the calling agent when the target changes.
- **FR-009**: Before a dynamic link call executes, the system MUST validate that the target agent accepts the message or task, and return a natural language error on mismatch.
- **FR-010**: The syntax MUST be learnable by a beginner: no manual memory management, no mandatory type syntax for simple cases, and natural language error messages (see SC-007).
- **FR-011**: The system MUST prevent an agent from using a built in tool (file, network, environment) it has not explicitly declared. Environment access is granted per named variable, and the variable holding the language model key is never readable by agents.
- **FR-012**: The system MUST detect and clearly report cycles in dynamic link calls.
- **FR-013**: The system MUST be distributed as a single executable per platform for Linux, macOS, and Windows.
- **FR-014**: A single `.ag` file MUST be able to define one or more agents.
- **FR-015**: Dynamic link MUST resolve a target agent in this order: an agent defined in the same file, a file named after the agent next to the calling file, then the project's `agents/` folder. A quoted path is used as written.
- **FR-016**: The system MUST execute tasks synchronously in the MVP: the agent waits for the tool's result, and a task that exceeds its time limit returns a clear timeout result. Long running tasks with later result checks are out of scope for the MVP.
- **FR-017**: The system MUST reason with a language model whose provider is chosen through configuration only (never in agent source code), so agent files stay simple and portable across providers.
- **FR-018**: The system MUST handle concurrency invisibly: incoming A2A and MCP requests are served concurrently without the user writing anything extra to enable it.
- **FR-019**: When serving agents to other programs (A2A or MCP over the network), the system MUST accept connections from the local machine only by default. Exposing agents beyond the local machine MUST require an explicit choice by the user. Authentication is out of scope for the MVP.
- **FR-020**: When an agent reasons with the language model and the model asks to use tools, the system MUST stop after a maximum number of steps (default 10, configurable) and report this in plain language.

### Key Entities

- **Agent**: the central unit of execution; has a goal, enabled tools, handlers, a public interface, and optionally an Agent Card.
- **Tool**: a capability an agent can invoke, either built in (file, HTTP, environment, state, clock) or exposed by an external MCP server.
- **Task**: a unit of work executed through a tool call, always waited for (see FR-016), producing a result the agent can use.
- **MCP Server**: an external process exposing tools or tasks discoverable through MCP, consumed by an agent as a client.
- **Agent Card**: the public description of an agent under A2A, used by other agents to discover its capabilities.
- **Dynamic Link**: a runtime reference from one agent to another Metagente agent, resolved and loaded on demand.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person with no programming experience writes and runs their first working agent within 15 minutes, following the tutorial.
- **SC-002**: A minimal working agent (receives a message, calls a tool, executes a task, replies) is fewer than 10 lines.
- **SC-003**: The core loop (invoke a tool and execute a task) works end to end for both a built in tool and an external MCP tool, with no code beyond declaring the tool and calling it.
- **SC-004**: 100% of the built in tools in FR-004 work with nothing installed beyond the interpreter.
- **SC-005**: Two distinct agents communicate through dynamic link, and changing one requires no edit to or restart of the other.
- **SC-006**: An agent with a published Agent Card is discovered by, and receives a task from, an external A2A compatible agent outside the Metagente ecosystem.
- **SC-007**: 100% of syntax and runtime errors raised by Metagente itself are shown in natural language with a line reference and a suggested fix. Errors that come from outside Metagente (for example an external MCP server failing) are relayed in plain words and name their source, with a fix suggested when one exists.

## Assumptions

- Target users are non programmers; the tutorial and error messages are in English for the MVP.
- The interpreter is implemented in Rust as required by the project constitution; this is a project constraint, not a user facing behavior, so it is not part of the requirements.
- Agents run on the user's own machine with normal network access; hosted or multi-tenant execution is out of scope.
- Dynamic link is local (same host); remote composition is done through A2A.
- Only standard MCP and A2A behavior is supported; no proprietary extensions.
- Specialized tools beyond FR-004 are delivered as external MCP packages, not built in.
- Graphical editors, debuggers, and package registries are out of scope for the MVP.
- Authentication and authorization for served agents are out of scope for the MVP; served agents listen on the local machine only unless the user chooses otherwise (FR-019).
