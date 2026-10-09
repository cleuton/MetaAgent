// 0.1.3: new file. Which proxy is used, which hosts skip it, and that no secret can be shown.
use metagente::runtime::config::ProxyConfig;
use metagente::runtime::net::{EffectiveProxy, ProxySource, no_proxy_matches, redact};
use reqwest::Url;
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |name: &str| map.get(name).cloned()
}

fn url(text: &str) -> Url {
    Url::parse(text).unwrap()
}

#[test]
fn without_any_setting_there_is_no_proxy() {
    let p = EffectiveProxy::resolve(&ProxyConfig::default(), "metagente.toml", &env(&[])).unwrap();
    assert_eq!(p.source, ProxySource::None);
    assert!(p.for_destination(&url("https://example.com")).is_none());
}

#[test]
fn the_environment_is_used_when_the_toml_has_no_url() {
    let e = env(&[
        ("HTTPS_PROXY", "http://secure.proxy:3128"),
        ("http_proxy", "http://plain.proxy:8080"),
    ]);
    let p = EffectiveProxy::resolve(&ProxyConfig::default(), "metagente.toml", &e).unwrap();
    assert_eq!(p.source, ProxySource::Environment);
    assert_eq!(
        p.for_destination(&url("https://a.example"))
            .unwrap()
            .display(),
        "http://secure.proxy:3128"
    );
    assert_eq!(
        p.for_destination(&url("http://a.example"))
            .unwrap()
            .display(),
        "http://plain.proxy:8080"
    );
}

#[test]
fn the_toml_url_wins_and_the_environment_is_ignored_completely() {
    let cfg = ProxyConfig {
        url: Some("http://toml.proxy:1".into()),
        ..ProxyConfig::default()
    };
    let e = env(&[
        ("HTTPS_PROXY", "http://env.proxy:2"),
        ("NO_PROXY", "a.example"),
    ]);
    let p = EffectiveProxy::resolve(&cfg, "metagente.toml", &e).unwrap();
    assert_eq!(p.source, ProxySource::Toml);
    assert!(p.environment_ignored);
    assert_eq!(
        p.for_destination(&url("https://x.example"))
            .unwrap()
            .display(),
        "http://toml.proxy:1"
    );
    // the environment NO_PROXY is ignored too
    assert!(p.for_destination(&url("https://a.example")).is_some());
    assert!(p.describe().contains("ignored"), "{}", p.describe());
}

#[test]
fn a_toml_no_proxy_list_replaces_the_environment_one() {
    let cfg = ProxyConfig {
        no_proxy: vec!["only.example".into()],
        ..ProxyConfig::default()
    };
    let e = env(&[
        ("HTTPS_PROXY", "http://env.proxy:2"),
        ("NO_PROXY", "other.example"),
    ]);
    let p = EffectiveProxy::resolve(&cfg, "metagente.toml", &e).unwrap();
    assert_eq!(p.source, ProxySource::Environment);
    assert!(p.for_destination(&url("https://only.example")).is_none());
    assert!(p.for_destination(&url("https://other.example")).is_some());
}

#[test]
fn no_proxy_knows_names_suffixes_addresses_and_localhost() {
    let list: Vec<String> = [
        "localhost",
        "127.0.0.1",
        ".internal.company.com",
        "exact.example",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for host in [
        "localhost",
        "127.0.0.1",
        "api.internal.company.com",
        "internal.company.com",
        "exact.example",
        "EXACT.example",
    ] {
        assert!(no_proxy_matches(host, &list), "{}", host);
    }
    for host in ["example.com", "xexact.example", "company.com", "10.0.0.1"] {
        assert!(!no_proxy_matches(host, &list), "{}", host);
    }
    assert!(no_proxy_matches("anything", &["*".to_string()]));
}

#[test]
fn credentials_in_an_environment_address_are_used_and_never_shown() {
    let e = env(&[("HTTPS_PROXY", "http://bob:s%40cret%3A@proxy.example:3128")]);
    let p = EffectiveProxy::resolve(&ProxyConfig::default(), "metagente.toml", &e).unwrap();
    let target = p.for_destination(&url("https://a.example")).unwrap();
    assert!(target.login.is_some());
    let shown = format!("{} | {}", target.display(), p.describe());
    for secret in ["bob", "s%40cret", "s@cret"] {
        assert!(!shown.contains(secret), "{}", shown);
    }
    assert!(shown.contains("***") || !shown.contains('@'), "{}", shown);
}

#[test]
fn the_login_comes_from_named_variables_even_with_special_characters() {
    let cfg = ProxyConfig {
        url: Some("http://proxy.example:3128".into()),
        username_env: Some("PROXY_USER".into()),
        password_env: Some("PROXY_PASSWORD".into()),
        ..ProxyConfig::default()
    };
    let e = env(&[("PROXY_USER", "ann"), ("PROXY_PASSWORD", "p@ss:/%word")]);
    let p = EffectiveProxy::resolve(&cfg, "metagente.toml", &e).unwrap();
    let t = p.for_destination(&url("https://a.example")).unwrap();
    let (user, password) = t.login.as_ref().unwrap();
    assert_eq!(user.expose(), "ann");
    assert_eq!(password.expose(), "p@ss:/%word");
}

#[test]
fn a_missing_or_empty_login_variable_is_named_but_its_value_never_is() {
    let cfg = ProxyConfig {
        url: Some("http://proxy.example:3128".into()),
        username_env: Some("PROXY_USER".into()),
        password_env: Some("PROXY_PASSWORD".into()),
        password_env_line: 7,
        ..ProxyConfig::default()
    };
    let e = env(&[("PROXY_USER", "ann"), ("PROXY_PASSWORD", "")]);
    let err = EffectiveProxy::resolve(&cfg, "metagente.toml", &e)
        .err()
        .unwrap()
        .render();
    assert!(err.contains("`PROXY_PASSWORD`"), "{}", err);
    assert!(err.contains("line 7"), "{}", err);
    assert!(!err.contains("ann"), "{}", err);
}

#[test]
fn redact_hides_anything_before_the_at_sign() {
    assert_eq!(
        redact("http://bob:secret@proxy.example:3128/x"),
        "http://***@proxy.example:3128/x"
    );
    assert_eq!(redact("https://example.com/a"), "https://example.com/a");
}

#[test]
fn what_a_tool_program_inherits_includes_both_cases_and_the_login() {
    let cfg = ProxyConfig {
        url: Some("http://proxy.example:3128".into()),
        username_env: Some("U".into()),
        password_env: Some("P".into()),
        no_proxy: vec!["localhost".into()],
        ..ProxyConfig::default()
    };
    let p = EffectiveProxy::resolve(&cfg, "metagente.toml", &env(&[("U", "ann"), ("P", "pw")]))
        .unwrap();
    let vars = p.child_environment();
    for name in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        assert!(vars.iter().any(|(n, _)| n == name), "{}", name);
    }
    let value = &vars.iter().find(|(n, _)| n == "HTTPS_PROXY").unwrap().1;
    assert_eq!(value, "http://ann:pw@proxy.example:3128");
}

// 0.1.3: NO_PROXY ranges such as 10.0.0.0/8 are common in company environments
#[test]
fn no_proxy_knows_address_ranges() {
    let list: Vec<String> = ["10.0.0.0/8", "192.168.1.0/24", "fd00::/8"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for host in [
        "10.1.2.3",
        "10.255.255.255",
        "192.168.1.77",
        "fd00::1",
        "[fd00::2]",
    ] {
        assert!(no_proxy_matches(host, &list), "{}", host);
    }
    for host in ["11.0.0.1", "192.168.2.1", "example.com", "fe80::1"] {
        assert!(!no_proxy_matches(host, &list), "{}", host);
    }
    assert!(!no_proxy_matches(
        "10.0.0.1",
        &["10.0.0.0/notanumber".to_string()]
    ));
}

#[test]
fn check_says_nothing_when_there_is_no_proxy() {
    let (notes, problems) = metagente::runtime::net::check_lines(
        &metagente::runtime::config::NetworkConfig::default(),
        &|_| None,
    );
    assert!(notes.is_empty() && problems.is_empty());
}
