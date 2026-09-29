# Feature Specification: Metagente, an interpreted 4GL language for AI agents

**Feature Branch**: `001-metagente-core`
**Created**: 2026-09-05
**Status**: Draft
**Input**: "Create an interpreted programming language for building AI agents. It must use MCP and A2A and ship with some built in tools (file access, for example). It must be a 4GL, simple, easy to use even for a beginner. The interpreter must be written in Rust. It must be able to invoke other agents built with it through dynamic link. The MVP must be able to invoke tools and execute tasks."

## MVP Scope

The first release of Metagente is scoped as a Minimum Viable Product whose core, non negotiable capability is: an agent can invoke tools and execute tasks through them. Everything else in this spec (built in tools, MCP, A2A, dynamic link) exists to serve that one capability. If a requirement below does not directly support "invoke a tool, execute a task," it is out of scope for the MVP and should move to a later spec.

## User Scenarios & Testing

### Primary User Story

A person with no prior programming experience wants to build an agent that answers questions about the weather. They write a single `.ag` file in a few lines, declare that the agent may use the built in HTTP tool and an external MCP weather tool, run the file with the Metagente interpreter, and the agent invokes the tool, executes the task, and answers correctly. Later, they create a second agent that, on a more complex question, invokes the first agent through dynamic link to get the weather data before replying.

### Acceptance Scenarios

1. **Given** a valid `.ag` file with an agent that declares `tool file`, **when** the agent is run and asks to read a local file, **then** the file's content is read and made available to the agent, without the user writing manual I/O code.
2. **Given** an agent that declares a connection to an external MCP server, **when** the agent invokes a tool exposed by that server, **then** the interpreter discovers the tool, validates the parameters against the schema the server provides, executes the call, and returns the result to the agent, this whole chain counting as "invoking a tool and executing a task" for the MVP.
3. **Given** two Metagente agents in the same project, **when** Agent A references Agent B by name through dynamic link, **then** the interpreter locates, loads, and executes Agent B on demand, returning the result to Agent A, without Agent A knowing Agent B's internals.
4. **Given** an agent that publishes its Agent Card through A2A, **when** an external A2A compatible agent discovers that card and sends it a task, **then** the Metagente agent receives, processes, and responds to the task following the A2A protocol.
5. **Given** a beginner with no prior experience, **when** they follow the getting started tutorial and write their first agent, **then** they can run a working agent in under 15 minutes without needing to understand Rust, network protocols, or asynchronous programming.
6. **Given** a syntax or runtime error in an `.ag` file, **when** the interpreter hits the error, **then** the message shown is in natural language, points at the exact line, and suggests a fix, without exposing internal Rust interpreter details.

### Edge Cases

* What happens when an agent attempts a dynamic link call to another agent that does not exist or has moved?
* What happens when an external MCP server becomes unavailable in the middle of a tool call or a task execution?
* What happens when a task executed through a tool runs longer than expected, times out, or fails partway through?
* What happens when two agents call each other through dynamic link in a cycle (Agent A calls B, which calls A)?
* What happens when an agent tries to access a tool (file, network) it did not declare in its permissions?
* How does the interpreter behave when a message received through A2A does not match any task the agent knows how to handle?

## Requirements

### Functional Requirements

* **FR-001**: The system MUST provide an interpreter that runs Metagente source files (suggested extension `.ag`) directly, with no separate compilation step visible to the user.
* **FR-002**: The language MUST provide a first class construct for declaring an agent, including a declared goal (`goal`), a set of enabled tools, and message or task handlers.
* **FR-003**: For the MVP, an agent MUST be able to invoke at least one tool and execute at least one task end to end, covering both a built in tool (for example file access) and an external MCP tool, as the core proof of the language's reason to exist.
* **FR-004**: The system MUST ship, with no additional installation, at least the following built in tools: file access (read and write), HTTP requests, environment variable reads, and a simple per agent state store.
* **FR-005**: The system MUST act as an MCP client, able to connect to external MCP servers, discover the tools they expose, and invoke them, including tools that represent longer running task execution, not just simple function calls.
* **FR-006**: The system MUST be able to act as an MCP server, exposing a Metagente agent's tools and capabilities so other MCP clients can discover and invoke them.
* **FR-007**: The system MUST implement enough of the A2A protocol to publish an Agent Card, receive tasks from external A2A compatible agents, and send tasks to external A2A agents.
* **FR-008**: The language MUST provide a dynamic link mechanism letting an agent invoke another Metagente agent by name or path at runtime, with no need to relink or recompile the calling agent when the target agent changes.
* **FR-009**: The system MUST validate, before executing a dynamic link call, that the target agent accepts the message or task being sent, returning a natural language error on mismatch.
* **FR-010**: The language's syntax MUST be learnable by a beginner with no prior programming experience, which includes: no manual memory management, no mandatory type syntax for simple cases, and natural language error messages.
* **FR-011**: The system MUST prevent an agent from accessing a built in tool (file, network) it has not explicitly declared using.
* **FR-012**: The system MUST detect and clearly report cycles in agent to agent dynamic link calls, avoiding silent infinite loops.
* **FR-013**: The interpreter MUST be distributed as a single binary per platform (Linux, macOS, Windows), built from a common Rust codebase.

### Key Entities

* **Agent**: the language's central unit of execution. Has a declared goal, a list of enabled tools, a set of message or task handlers, and optionally an Agent Card for A2A exposure.
* **Tool**: a capability an agent can invoke, either built into the interpreter (file, HTTP, environment, state) or exposed by an external MCP server.
* **Task**: a unit of work executed through a tool call, whether a quick synchronous function or a longer running operation, that produces a result the agent can use.
* **MCP Server**: an external process exposing a set of tools or tasks discoverable through the MCP protocol, consumed by a Metagente agent as a client.
* **Agent Card**: the public description of a Metagente agent under the A2A protocol, used by other agents to discover its capabilities before sending it a task.
* **Dynamic Link**: a runtime reference from one agent to another Metagente agent, resolved and loaded on demand by the interpreter.

## Success Criteria

* **SC-001**: A person with no prior programming experience can write and successfully run their first working agent within 15 minutes, following the getting started tutorial.
* **SC-002**: A minimal working agent (receives a message, calls a tool, executes a task, replies) can be written in 10 lines of code or fewer.
* **SC-003**: The MVP's core loop, invoking a tool and executing a task through it, works end to end for both a built in tool and an external MCP tool, with no additional code beyond declaring the tool and calling it.
* **SC-004**: 100% of the built in tools described in FR-004 work with no dependency or installation beyond the interpreter binary.
* **SC-005**: Two distinct Metagente agents can communicate through dynamic link with neither needing to be recompiled or restarted when the other changes.
* **SC-006**: A Metagente agent published with an Agent Card is discovered by, and successfully receives a task from, an external A2A compatible agent outside the Metagente ecosystem.

## Clarifications Needed

* **[NEEDS CLARIFICATION]**: the exact source file format expected (`.ag`, `.metagente`, other), and whether a single file can hold multiple agents or must hold exactly one.
* **[NEEDS CLARIFICATION]**: the concurrency model exposed to the language user (do agents run in parallel by default, or only when declared), since this directly affects the simplicity principle.
* **[NEEDS CLARIFICATION]**: how dynamic link resolves the target agent (relative path, a central registry of installed agents, or both).
* **[NEEDS CLARIFICATION]**: which LLM provider or model the interpreter assumes by default for an agent's "reasoning," and whether this is configurable per agent or fixed at the runtime level.
* **[NEEDS CLARIFICATION]**: for the MVP specifically, whether "execute a task" must support long running or asynchronous tasks (polling for a result later), or whether synchronous tool calls are sufficient for the first release.

## Review & Acceptance Checklist

* [ ] All functional requirements are testable and unambiguous
* [ ] Success criteria are measurable and not tied to a specific implementation technology
* [ ] Acceptance scenarios cover the happy paths for tool invocation, task execution, MCP, A2A, and dynamic link
* [ ] Edge cases cover network failures, cycles, and permissions
* [ ] No functional requirement describes how to implement something in Rust, only the expected behavior
* [ ] The MVP Scope section is respected: every requirement traces back to invoking a tool and executing a task
