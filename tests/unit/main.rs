mod auth_cache; // 0.1.4: accepted-token cache
mod auth_header; // 0.1.4: the Authorization header rules
mod change_markers; // 0.1.2: markers and version
mod diagnostics_golden;
mod example_size;
mod lexer_multiline; // 0.1.2: multi-line strings
mod models_safe_names; // 0.1.5: what a message may print from metagente.toml
mod net_proxy_rules; // 0.1.3: proxy choice, no_proxy and secrets
mod parser_tests;
mod redaction; // 0.1.4: a token never shows in a printed value
mod reserved_words; // 0.1.5: the words that cannot be a model nickname
mod secure_rules; // 0.1.3: the rule that refuses http for secure names
mod think_using_parser; // 0.1.5: think ... using NAME
mod version; // 0.1.4: [version] minimum
