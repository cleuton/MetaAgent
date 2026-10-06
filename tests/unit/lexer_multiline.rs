// 0.1.2: new file. Multi-line strings between `"""` and `"""`, kept exactly as typed.
use metagente::lang::{Expr, Params, Stmt, TextPart, parse_file, parse_file_with};

// 0.1.2: the text of `a = think <text>` in the only handler of the only agent
fn think_text(source: &str) -> Vec<TextPart> {
    let agents = parse_file("m.ag", None, source).unwrap_or_else(|d| panic!("{}", d.render()));
    match &agents[0].handlers[0].body[0] {
        Stmt::Assign {
            value: Expr::Think { prompt, .. },
            ..
        } => match prompt.as_ref() {
            Expr::Text(parts, _) => parts.clone(),
            other => panic!("not a text: {:?}", other),
        },
        other => panic!("not a think: {:?}", other),
    }
}

fn literal(parts: &[TextPart]) -> String {
    assert_eq!(parts.len(), 1, "expected one literal part: {:?}", parts);
    match &parts[0] {
        TextPart::Lit(s) => s.clone(),
        other => panic!("not a literal: {:?}", other),
    }
}

const HEAD: &str = "agent A\n  goal \"x\"\n  accepts go\n  on go\n";

// 0.1.2: scenario 2 of the spec, with the space after `isso.` and the indented closing quotes
#[test]
fn spaces_and_line_breaks_stay_exactly_as_typed() {
    let source = format!(
        "{HEAD}    a = think \"\"\"\nAqui vale fazer isso. \n         E isso também\n  \"\"\"\n"
    );
    assert_eq!(
        literal(&think_text(&source)),
        "\nAqui vale fazer isso. \n         E isso também\n  "
    );
}

#[test]
fn text_right_after_the_opening_quotes_is_kept() {
    let source = format!("{HEAD}    a = think \"\"\"You are a recruiter.\nRead it.\"\"\"\n");
    assert_eq!(
        literal(&think_text(&source)),
        "You are a recruiter.\nRead it."
    );
}

#[test]
fn an_empty_multi_line_string_is_an_empty_text() {
    let source = format!("{HEAD}    a = think \"\"\"\"\"\"\n");
    assert_eq!(literal(&think_text(&source)), "");
}

#[test]
fn a_hash_inside_is_text_and_not_a_comment() {
    let source = format!("{HEAD}    a = think \"\"\"\n# not a comment\n  also # this\n\"\"\"\n");
    assert_eq!(
        literal(&think_text(&source)),
        "\n# not a comment\n  also # this\n"
    );
}

#[test]
fn braces_still_fill_in_names() {
    let source = format!("{HEAD}    a = think \"\"\"Resume:\n{{resume}}\"\"\"\n");
    let parts = think_text(&source);
    assert_eq!(
        parts,
        vec![
            TextPart::Lit("Resume:\n".to_string()),
            TextPart::Var(vec!["resume".to_string()])
        ]
    );
}

#[test]
fn a_backslash_is_not_an_escape_inside() {
    let source = format!("{HEAD}    a = think \"\"\"a\\nb\"\"\"\n");
    assert_eq!(literal(&think_text(&source)), "a\\nb");
}

#[test]
fn windows_line_endings_inside_are_kept() {
    let source = format!("{HEAD}    a = think \"\"\"x\r\ny\"\"\"\r\n");
    assert_eq!(literal(&think_text(&source)), "x\r\ny");
}

#[test]
fn an_empty_one_line_text_is_still_an_empty_text() {
    let source = format!("{HEAD}    a = think \"\"\n");
    assert_eq!(literal(&think_text(&source)), "");
}

#[test]
fn lines_after_a_multi_line_string_keep_their_own_statements() {
    let source = format!("{HEAD}    a = think \"\"\"\none\ntwo\n\"\"\"\n    reply a\n");
    let agents = parse_file("m.ag", None, &source).unwrap();
    assert_eq!(agents[0].handlers[0].body.len(), 2);
}

#[test]
fn the_error_after_a_multi_line_string_names_the_right_line() {
    // line 5 opens the string, it closes on line 8, the broken word is on line 9
    let source = format!("{HEAD}    a = think \"\"\"\none\ntwo\n\"\"\"\n    reply nme nme\n");
    let text = parse_file("m.ag", None, &source).unwrap_err().render();
    assert!(text.starts_with("Problem on line 9"), "{}", text);
}

#[test]
fn text_after_the_closing_quotes_continues_the_line() {
    let source = format!("{HEAD}    reply \"\"\"a\nb\"\"\" extra\n");
    let text = parse_file("m.ag", None, &source).unwrap_err().render();
    // the logical line is reported where it starts, which is where the string opened
    assert!(text.starts_with("Problem on line 5"), "{}", text);
    assert!(text.contains("I did not expect `extra`"), "{}", text);
}

// 0.1.2: scenario 3, the text is never closed
#[test]
fn a_string_that_is_never_closed_points_at_the_line_it_opened() {
    let source = format!("{HEAD}    a = think \"\"\"\nno end here\n");
    let text = parse_file("m.ag", None, &source).unwrap_err().render();
    assert!(text.starts_with("Problem on line 5"), "{}", text);
    assert!(
        text.contains("the text that starts on line 5 was never closed"),
        "{}",
        text
    );
    assert!(text.contains("Fix: end it with three quotes"), "{}", text);
}

#[test]
fn a_parameter_value_is_not_needed_to_parse_a_file_without_parameters() {
    let source = format!("{HEAD}    reply \"hi\"\n");
    assert!(parse_file_with("m.ag", None, &source, &Params::default()).is_ok());
}
