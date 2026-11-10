// 0.1.5: new file. The words of the language that cannot be a model nickname (D1): one list, kept honest by tests.
use metagente::lang::RESERVED_WORDS;
use metagente::lang::parser::{AGENT_WORDS, BUILTIN_TOOLS};

#[test]
fn every_word_the_parser_treats_specially_is_reserved() {
    for w in AGENT_WORDS.iter().chain(BUILTIN_TOOLS) {
        assert!(RESERVED_WORDS.contains(w), "{} is not in RESERVED_WORDS", w);
    }
    for w in [
        "agent",
        "reply",
        "fail",
        "if",
        "otherwise",
        "for",
        "in",
        "think",
        "yes",
        "no",
        "nothing",
        "or",
        "and",
        "not",
        "is",
        "more",
        "less",
        "than",
        "contains",
        "within",
        "second",
        "seconds",
        "from",
        "at",
        "mcp",
        "start",
    ] {
        assert!(
            RESERVED_WORDS.contains(&w),
            "{} is not in RESERVED_WORDS",
            w
        );
    }
}

#[test]
fn using_is_reserved_as_a_nickname() {
    assert!(RESERVED_WORDS.contains(&"using"));
}

#[test]
fn the_list_is_sorted_and_has_no_duplicates() {
    let mut sorted = RESERVED_WORDS.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted, RESERVED_WORDS.to_vec());
}
