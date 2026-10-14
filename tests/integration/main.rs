mod common;

mod a2a_client;
mod a2a_interop;
mod a2a_unsupported;
mod builtin_tools;
mod dynamic_link;
mod env_scope;
mod examples_check;
mod http_state_tools;
mod https_chain; // 0.1.3: the sample multi-agent-samples/https-chain end to end
mod internal_error;
mod link_cycle;
mod link_interface;
mod link_parameters; // 0.1.2: linked agents use the loader's parameters
mod llm_provider;
mod matrix_regression; // 0.1.3: one test per cell of the baseline matrix that already worked
mod mcp_client;
mod mcp_https; // 0.1.3: tool servers over https
mod mcp_server;
mod multiline_run; // 0.1.2: multi-line prompts reach the model as typed
mod net_config; // 0.1.3: the [network] section
mod net_model; // 0.1.3: the model has its own switch
mod net_pass_to_tools; // 0.1.3: proxy settings for tool programs
mod net_proxy; // 0.1.3: proxies and their login
mod net_regression; // 0.1.3: what worked before still works
mod net_selfsigned; // 0.1.3: development certificates
mod net_tls; // 0.1.3: https, bad certificates, downgrade
mod parameters; // 0.1.2: @parameters.name
mod perf;
mod permissions;
mod python_interop; // 0.1.3: Python and Metagente agents call each other
mod sample_city_briefing;
mod secure_agent; // 0.1.3: a secure agent speaks https in every direction
mod secure_check; // 0.1.3: `metagente check` and the secure list
mod secure_concurrency; // 0.1.3: https must not serialize requests
mod secure_link; // 0.1.3: a secure loader cannot escape the rule by delegating
mod secure_llm; // 0.1.3: the model over https only
mod secure_remote; // 0.1.3: remote agents that must use https
mod secure_selfsigned; // 0.1.3: self-signed only when the toml says so
mod secure_serve; // 0.1.3: `metagente serve` over https
mod secure_serve_refusals; // 0.1.3: what `metagente serve` refuses over https
mod secure_tools; // 0.1.3: tools that must use https
mod serve_bind;
mod serve_concurrency;
mod think;
mod timeout;
