mod common;

mod a2a_client;
mod a2a_interop;
mod a2a_unsupported;
mod builtin_tools;
mod dynamic_link;
mod env_scope;
mod examples_check;
mod http_state_tools;
mod internal_error;
mod link_cycle;
mod link_interface;
mod link_parameters; // 0.1.2: linked agents use the loader's parameters
mod llm_provider;
mod mcp_client;
mod mcp_server;
mod multiline_run; // 0.1.2: multi-line prompts reach the model as typed
mod parameters; // 0.1.2: @parameters.name
mod perf;
mod permissions;
mod sample_city_briefing;
mod serve_bind;
mod serve_concurrency;
mod think;
mod timeout;
