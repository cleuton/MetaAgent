// 0.1.4: the Authorization header rules (D7).
use axum::http::{HeaderMap, HeaderValue, header::AUTHORIZATION};
use metagente::runtime::auth::{Malformed, parse_authorization};

fn one(value: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.append(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
    h
}

fn token_of(value: &str) -> Result<Option<String>, Malformed> {
    parse_authorization(&one(value)).map(|t| t.map(|t| t.expose().to_string()))
}

#[test]
fn no_header_is_no_token() {
    assert!(matches!(parse_authorization(&HeaderMap::new()), Ok(None)));
}

#[test]
fn a_valid_bearer_token_in_any_letter_case() {
    assert_eq!(
        token_of("Bearer abc.DEF-123_x~y+z/w==").unwrap().unwrap(),
        "abc.DEF-123_x~y+z/w=="
    );
    assert_eq!(token_of("bearer abc").unwrap().unwrap(), "abc");
    assert_eq!(token_of("BEARER abc").unwrap().unwrap(), "abc");
}

#[test]
fn everything_else_is_malformed() {
    for bad in [
        "Bearer",
        "Bearer ",
        "Bearer  two-spaces",
        "Bearer a b",
        "Bearer a;b",
        "Bearer a\"b",
        "Bearer 'a'",
        "Bearer $(x)",
        "Bearer a=b",
        "Bearer =",
        "Bearer ====",
        "Basic dXNlcjpwYXNz",
        "Token abc",
        "abc",
        "Bearerabc",
    ] {
        assert!(token_of(bad).is_err(), "{:?} should be malformed", bad);
    }
}

#[test]
fn two_headers_are_malformed() {
    let mut h = one("Bearer a");
    h.append(AUTHORIZATION, HeaderValue::from_static("Bearer a"));
    assert!(parse_authorization(&h).is_err());
}

#[test]
fn the_longest_token_is_16384_characters() {
    assert!(token_of(&format!("Bearer {}", "a".repeat(16384))).is_ok());
    assert!(token_of(&format!("Bearer {}", "a".repeat(16385))).is_err());
}
