// 0.1.4: a token never shows in any printed form (FR-035).
use metagente::runtime::auth::{AuthPolicy, Fingerprint, RequestAuth, parse_token};
use metagente::runtime::task::CallContext;
use std::sync::Arc;

const SECRET: &str = "super-secret-token-value";

fn policy() -> Arc<AuthPolicy> {
    Arc::new(AuthPolicy {
        validator: Some("v".into()),
        protect: vec![],
        forward: vec![],
        cache_seconds: 0,
        timeout_seconds: 5,
        base_dir: ".".into(),
        file: "metagente.toml".into(),
        forward_line: 0,
        cache: Default::default(),
    })
}

#[test]
fn nothing_printed_contains_the_token() {
    let token = parse_token(SECRET).unwrap();
    let auth = RequestAuth::network(policy(), Some(token.clone()), true);
    let mut ctx = CallContext::new("t-1");
    ctx.auth = Some(auth.clone());
    let printed = [
        format!("{:?}", token),
        format!("{}", token),
        format!("{:?}", auth),
        format!("{:?}", ctx),
        format!("{:?}", ctx.entering("A")),
        format!("{:?}", Fingerprint::of(&token)),
    ];
    for text in printed {
        assert!(!text.contains(SECRET), "{}", text);
        assert!(!text.contains("secret"), "{}", text);
    }
}

#[test]
fn the_fingerprint_tells_tokens_apart_and_is_not_the_token() {
    let a = Fingerprint::of(&parse_token("aaa").unwrap());
    let b = Fingerprint::of(&parse_token("bbb").unwrap());
    assert_ne!(a, b);
    assert_eq!(a, Fingerprint::of(&parse_token("aaa").unwrap()));
}
