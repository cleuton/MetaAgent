// 0.1.5: new file. The rule of D10: a message prints nothing from metagente.toml or from the `using` word of an `.ag`
// file except a SAFE NAME, because a person may paste a key where a name belongs.
use metagente::runtime::models::{shown, valid_nickname, valid_variable_name, variable_phrase};

#[test]
fn a_safe_name_is_one_to_24_ascii_letters_digits_underscores_or_dashes() {
    assert_eq!(shown("fast"), Some("fast"));
    assert_eq!(shown("fast-model_2"), Some("fast-model_2"));
    assert_eq!(shown("A"), Some("A"));
    assert_eq!(
        shown("a23456789012345678901234"),
        Some("a23456789012345678901234")
    ); // 24
    assert_eq!(shown("a234567890123456789012345"), None); // 25
    for unsafe_text in [
        "", "a b", "a\"b", "a.b", "a=b", "a/b", "fást", "x\ny", "$(x)", "a;b", "[x]", " fast",
        "fast ",
    ] {
        assert_eq!(shown(unsafe_text), None, "{:?}", unsafe_text);
    }
    // a pasted key is long, so it is never safe
    assert_eq!(
        shown("sk-ant-api03-0123456789abcdefghijklmnopqrstuvwxyz"),
        None
    );
}

#[test]
fn a_variable_name_is_one_to_64_ascii_letters_digits_or_underscores_not_starting_with_a_digit() {
    for ok in ["FAST_KEY", "a", "_x", "X1", &"A".repeat(64)] {
        assert!(valid_variable_name(ok), "{}", ok);
    }
    for bad in [
        "",
        "1KEY",
        "FAST-KEY",
        "FAST.KEY",
        "FAST KEY",
        "sk-ant-api03-x",
        "KEY=1",
        "ключ",
        &"A".repeat(65),
    ] {
        assert!(!valid_variable_name(bad), "{}", bad);
    }
}

#[test]
fn a_nickname_starts_with_a_letter_and_has_at_most_24_safe_characters() {
    for ok in [
        "fast",
        "strong",
        "fast-model",
        "gpt_4",
        "A",
        "a23456789012345678901234",
    ] {
        assert!(valid_nickname(ok), "{}", ok);
    }
    for bad in [
        "",
        "1fast",
        "_fast",
        "-fast",
        "fast model",
        "a234567890123456789012345",
        "fást",
    ] {
        assert!(!valid_nickname(bad), "{}", bad);
    }
}

#[test]
fn a_variable_is_named_in_a_message_only_when_its_name_is_safe() {
    assert_eq!(variable_phrase("FAST_KEY", 3), "the variable FAST_KEY");
    let long = "A_VERY_LONG_VARIABLE_NAME_OF_MORE_THAN_24";
    assert!(valid_variable_name(long));
    assert_eq!(variable_phrase(long, 7), "the variable named on line 7");
}
