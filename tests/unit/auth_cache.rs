// 0.1.4: the accepted-token cache (D10).
use metagente::runtime::auth::{AcceptedCache, CACHE_LIMIT, Fingerprint, parse_token};
use std::time::{Duration, Instant};

fn fp(n: usize) -> Fingerprint {
    Fingerprint::of(&parse_token(&format!("token-{}", n)).unwrap())
}

#[test]
fn an_entry_lives_for_the_lifetime_from_acceptance_and_is_not_extended() {
    let cache = AcceptedCache::new();
    let t0 = Instant::now();
    let life = Duration::from_secs(30);
    cache.record(fp(1), life, t0);
    assert!(cache.is_fresh(&fp(1), life, t0 + Duration::from_secs(29)));
    // asking again at 29 seconds did not move the end
    assert!(!cache.is_fresh(&fp(1), life, t0 + Duration::from_secs(30)));
    assert!(!cache.is_fresh(&fp(2), life, t0));
}

#[test]
fn a_zero_lifetime_keeps_nothing() {
    let cache = AcceptedCache::new();
    cache.record(fp(1), Duration::ZERO, Instant::now());
    assert!(cache.is_empty());
    assert!(!cache.is_fresh(&fp(1), Duration::ZERO, Instant::now()));
}

#[test]
fn at_most_1000_entries_and_the_oldest_go_first() {
    let cache = AcceptedCache::new();
    let t0 = Instant::now();
    let life = Duration::from_secs(300);
    for n in 0..(CACHE_LIMIT + 5) {
        cache.record(fp(n), life, t0 + Duration::from_millis(n as u64));
    }
    assert_eq!(cache.len(), CACHE_LIMIT);
    let now = t0 + Duration::from_secs(1);
    assert!(!cache.is_fresh(&fp(0), life, now));
    assert!(cache.is_fresh(&fp(CACHE_LIMIT + 4), life, now));
}
