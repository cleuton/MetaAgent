# Feature Specification: Multi-Agent Sample (City Briefing)

**Feature Branch**: `002-multiagent-sample`

**Created**: 2026-09-29

**Status**: Draft

**Input**: User description: "Create a multi-agent sample using Metagente. At least 2 agents: one coordinates the response and the other uses tools to get information. The demo must use A2A and MCP and Anthropic Claude Sonnet 5.5. The project must have a samples directory with instructions on how to configure and run the demo, including metagente.toml. No version upgrade."

## Scope and Constraints

This feature adds documentation and example files only. It changes nothing in the language, the interpreter, or the grammar.

- **No version change**: the constitution stays at 1.0.0 and spec 001 (Metagente Core) is not amended. Every construct used by the sample must already exist in spec 001 and the programming guide. If the sample cannot be written with existing constructs, that is a defect to report against spec 001, not a reason to extend the language here.
- **Dependencies on spec 001**: the sample needs User Story 1 (tools, MCP client, model configuration), User Story 4 (agent to agent calls) and User Story 5 (A2A serving and calling, local only by default). The sample cannot be completed before those stories ship.
- **Constitution alignment**: Principle I (each agent is short and readable aloud), Principle II (the agent is the unit), Principle III (both MCP and A2A are exercised), Principle IV (a built in tool is used with one line), and the secure by default constraint (each agent declares only what it needs).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run the demo from the samples folder (Priority: P1)

A person clones the project, opens `samples/city-briefing/`, follows the README, sets one environment variable, starts the researcher agent in one terminal and the concierge agent in another, asks about a city, and gets a short briefing.

**Why this priority**: The demo exists to show the multi-agent flow working end to end with copy and paste steps.

**Independent Test**: On a clean machine with only the interpreter, Node or Python tooling for the MCP server, and an Anthropic API key, follow the README and obtain a briefing for one city.

**Acceptance Scenarios**:

1. **Given** the README and a valid `ANTHROPIC_API_KEY`, **When** the person follows the steps in order, **Then** a briefing for the requested city is printed.
2. **Given** `metagente.toml` in the sample folder, **When** the agents run, **Then** both reason with Claude Sonnet 5.5 and no agent file names a provider or model (FR-017 of spec 001).
3. **Given** the person does not set the API key, **When** they run an agent, **Then** the message says in plain language which variable is missing and how to set it.

---

### User Story 2 - Coordinator delegates over A2A (Priority: P1)

The coordinator agent receives a question, delegates the fact finding to the researcher agent through A2A, and writes the final answer from what it gets back.

**Why this priority**: It proves agent to agent composition across process boundaries using the standard protocol.

**Independent Test**: Start only the researcher, send it an A2A task with an external A2A client, and confirm it answers. Then start the coordinator and confirm it reaches the researcher.

**Acceptance Scenarios**:

1. **Given** the researcher is served locally, **When** the coordinator receives a question, **Then** it sends an A2A task to the researcher and uses the reply to compose its answer.
2. **Given** the researcher is not running, **When** the coordinator tries to reach it, **Then** the message names the researcher, the address tried, and how to start it.
3. **Given** no public option is given, **When** another machine tries to reach the researcher, **Then** the connection is refused (FR-019 of spec 001).

---

### User Story 3 - Researcher gets facts through MCP (Priority: P1)

The researcher agent uses an external MCP server (the reference `fetch` server) to read a public page, uses the built in clock to stamp the answer, and summarizes the facts with the model.

**Why this priority**: It proves the tool invocation core loop with both an external MCP tool and a built in tool.

**Independent Test**: Run the researcher alone with a city and confirm the fetch tool is called and the reply carries a time stamp.

**Acceptance Scenarios**:

1. **Given** the MCP server is declared in `metagente.toml`, **When** the researcher asks for a page, **Then** the interpreter discovers the tool, validates the parameters, and returns the page text.
2. **Given** the MCP server cannot start, **When** the researcher runs, **Then** the message names the server, shows the command that failed, and suggests installing the prerequisite.
3. **Given** the researcher does not declare a tool, **When** it tries to use it, **Then** the attempt is refused (FR-011 of spec 001).

---

### Edge Cases

- The city is missing: the researcher stops with "I need a city" and the line.
- The page does not exist for the city: the model is told the fetch failed and the reply says no facts were found.
- The model keeps asking for tools: the step limit message appears (FR-020 of spec 001).
- Port already in use: the message names the port and how to change it.

## Requirements *(mandatory)*

### Functional Requirements

- **SAM-001**: The project MUST contain a `samples/` folder with a top level `README.md` listing the samples and a `city-briefing/` folder for this demo.
- **SAM-002**: `samples/city-briefing/` MUST contain `README.md`, `metagente.toml`, `concierge.ag` and `researcher.ag`.
- **SAM-003**: The README MUST cover prerequisites, configuration, running each agent, a sample question with expected output shape, an external A2A check, and troubleshooting.
- **SAM-004**: `metagente.toml` MUST select Anthropic as provider and Claude Sonnet 5.5 (`claude-sonnet-5-5`) as model, read the key from an environment variable, and declare the MCP server and the researcher's local address.
- **SAM-005**: `metagente.toml` MUST NOT contain the API key itself.
- **SAM-006**: The concierge MUST have no tools other than what it needs to reach the researcher and reason with the model.
- **SAM-007**: The researcher MUST use one external MCP tool and the built in clock, and declare nothing else.
- **SAM-008**: Each agent file MUST be readable aloud by a beginner, use only constructs from the programming guide, and contain no comments that explain the interpreter internals.
- **SAM-009**: The sample MUST work with the local only default for served agents and MUST NOT require the public option.
- **SAM-010**: No file in this feature may require a change to the interpreter, the grammar, or the constitution.

### Key Entities

- **Concierge**: the coordinating agent. Accepts a question, delegates, composes the final answer.
- **Researcher**: the tool using agent. Accepts a research request, fetches facts through MCP, stamps the time, summarizes.
- **metagente.toml**: the configuration file holding model, MCP server and remote agent address.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person following the README reaches a working briefing in under 10 minutes on a machine with prerequisites installed.
- **SC-002**: Each agent file is under 20 lines; the concierge is under 10 lines.
- **SC-003**: Stopping the researcher and asking a question produces a plain language error naming the researcher and how to start it.
- **SC-004**: An external A2A client can discover the researcher's Agent Card and send it a task.
- **SC-005**: Changing only the model name in `metagente.toml` changes the model used by both agents, with no agent file edited.

## Assumptions

- The reference MCP `fetch` server is started with `uvx mcp-server-fetch`, so Python tooling with `uv` is a prerequisite. Node users may swap the server in the toml.
- The exact spelling of MCP server, remote agent, serve and run settings follows the getting started tutorial and programming guide of spec 001. The plan and task list for this feature must take them from those documents and must not invent syntax.
- The sample files (agents, `metagente.toml`, README) are produced by the Speckit plan, task list and implementation steps for this feature, not by this specification.
- The sample runs on one machine with two terminals.
- The tutorial and README are in English.
- Cost and rate limits of the model provider are the user's responsibility.
