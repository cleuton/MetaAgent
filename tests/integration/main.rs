mod common;

mod a2a_client;
mod a2a_interop;
mod a2a_unsupported;
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_cache; // 0.1.4: the cache of accepted tokens
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_chain_sample; // 0.1.4: the sample multi-agent-samples/auth-chain end to end
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_concurrency; // 0.1.4: 50 at once, 8 validators at most
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_fail_closed; // 0.1.4: every mistake refuses
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_forward; // 0.1.4: passing the token on
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_no_leak; // 0.1.4: no token in any output
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_regression; // 0.1.4: nothing changes without the new sections
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_side_doors; // 0.1.4: link and GetTask
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod auth_validator; // 0.1.4: protecting a message and the validator contract
mod builtin_tools;
mod dynamic_link;
mod env_scope;
mod examples_check;
mod examples_unchanged; // 0.1.5: examples, samples and 0.1.4 doc blocks behave as on 0.1.4 (FR-022)
mod http_state_tools;
mod https_chain; // 0.1.3: the sample multi-agent-samples/https-chain end to end
mod internal_error;
mod link_cycle;
mod link_interface;
mod link_parameters; // 0.1.2: linked agents use the loader's parameters
mod llm_characterization; // 0.1.5: what 0.1.4 sends for [llm], proved before and after the work
mod llm_provider;
mod matrix_regression; // 0.1.3: one test per cell of the baseline matrix that already worked
mod mcp_client;
mod mcp_https; // 0.1.3: tool servers over https
mod mcp_server;
mod models_backward; // 0.1.5: what named models must not change
mod models_binary; // 0.1.5: the real binary with two named models
mod models_check_warnings; // 0.1.5: check warnings and unknown nicknames
mod models_config; // 0.1.5: every mistake of [llm.NAME] stops with a plain message
mod models_fake_servers; // 0.1.5: named models against fake Anthropic and OpenAI at their official addresses
mod models_new; // 0.1.5: metagente new shows how to name a model
mod models_no_key_in_output; // 0.1.5: no key in any output (SC-004)
mod models_security; // 0.1.5: each named model keeps the security rules of the single model
mod models_serve; // 0.1.5: serve reads the models at start and keeps the last good agent
mod multiline_run; // 0.1.2: multi-line prompts reach the model as typed
mod net_config; // 0.1.3: the [network] section
mod net_failure_kinds; // 0.1.5: the kind of a failed model request, with no text
mod net_model; // 0.1.3: the model has its own switch
mod net_pass_to_tools; // 0.1.3: proxy settings for tool programs
mod net_proxy; // 0.1.3: proxies and their login
mod net_regression; // 0.1.3: what worked before still works
mod net_selfsigned; // 0.1.3: development certificates
mod net_tls; // 0.1.3: https, bad certificates, downgrade
mod openai_reasoning_effort; // 0.1.5: one retry with reasoning_effort none after that refusal
mod parameters; // 0.1.2: @parameters.name
mod perf;
mod permissions;
mod python_interop; // 0.1.3: Python and Metagente agents call each other
#[cfg(unix)] // 0.1.4: these tests use shell script validators; Windows is not tested yet
mod python_interop_auth; // 0.1.4: bearer tokens between Metagente and the Python SDK
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
mod two_models_sample; // 0.1.5: the sample multi-agent-samples/two-models

mod think_using; // 0.1.5: think ... using NAME chooses the model
mod timeout;
mod using_as_a_name; // 0.1.5: `using` stays an ordinary name
mod version_gate; // 0.1.4: [version] minimum
