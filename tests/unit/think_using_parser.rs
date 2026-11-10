// 0.1.5: new file. The grammar of `think ... using NAME` (contracts/think-using-grammar.md): what is accepted, what is
// refused and with which plain message. `using` stays an ordinary name everywhere else.
use metagente::lang::check::thinks_using;
use metagente::lang::parse_file;

const NICKNAME: &str = "After using I need the name of a model, written as in [llm.NAME].";

fn agent(body: &str) -> String {
    format!(
        "agent A\n  goal \"x\"\n  accepts go\n  on go\n{}\n",
        body.lines()
            .map(|l| format!("    {}", l))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn nicknames(body: &str) -> Vec<String> {
    let agents =
        parse_file("t.ag", None, &agent(body)).unwrap_or_else(|d| panic!("{}", d.render()));
    thinks_using(&agents[0])
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

fn refused(body: &str) -> metagente::diagnostics::Diagnostic {
    match parse_file("t.ag", None, &agent(body)) {
        Ok(_) => panic!("should have been refused: {}", body),
        Err(d) => d,
    }
}

#[test]
fn using_after_the_prompt_names_the_model() {
    assert_eq!(
        nicknames("x = think \"hi\" using fast\nreply x"),
        vec!["fast"]
    );
    assert_eq!(
        nicknames("reply think \"hi {x}\" using strong"),
        vec!["strong"]
    );
    assert_eq!(
        nicknames("x = think \"hi\" using fast # a comment\nreply x"),
        vec!["fast"]
    );
}

#[test]
fn a_nickname_with_a_dash_is_one_word() {
    assert_eq!(
        nicknames("reply think \"hi\" using fast-model"),
        vec!["fast-model"]
    );
}

#[test]
fn think_without_using_is_as_before() {
    assert!(nicknames("reply think \"hi\"").is_empty());
}

#[test]
fn using_is_an_ordinary_name_elsewhere() {
    assert!(nicknames("using = \"x\"\nreply using").is_empty());
    // the prompt is the variable `using`, and no model is chosen
    assert!(nicknames("using = \"hi\"\nx = think using\nreply x").is_empty());
    // the first value after think is the prompt, even a variable called using
    assert_eq!(
        nicknames("using = \"hi\"\nx = think using using fast\nreply x"),
        vec!["fast"]
    );
}

#[test]
fn every_think_of_an_agent_is_found_in_loops_and_conditions() {
    let found = nicknames(
        "for item in [\"a\", \"b\"]\n  x = think \"{item}\" using fast\nif yes\n  reply think \"q\" using strong\notherwise\n  reply \"no\"",
    );
    assert_eq!(found, vec!["fast", "strong"]);
}

#[test]
fn no_nickname_is_refused_at_the_line() {
    let d = refused("x = think \"hi\" using");
    assert_eq!(d.message, NICKNAME);
    assert_eq!(d.line, 5);
    assert_eq!(
        d.suggestion.as_deref(),
        Some("write it like: think \"...\" using fast")
    );
}

#[test]
fn a_quoted_text_a_parameter_a_number_a_list_or_a_path_is_not_a_nickname() {
    for line in [
        "x = think \"hi\" using \"fast\"",
        "x = think \"hi\" using @parameters.model",
        "x = think \"hi\" using 5",
        "x = think \"hi\" using [fast]",
        "x = think \"hi\" using fast.fast",
    ] {
        let d = refused(line);
        assert_eq!(d.message, NICKNAME, "{}", line);
        assert_eq!(d.line, 5, "{}", line);
    }
}

#[test]
fn nothing_may_follow_the_nickname() {
    for line in [
        "x = think \"hi\" using fast extra",
        "x = think \"hi\" using fast is \"yes\"",
        "x = think \"hi\" using fast using strong",
    ] {
        let d = refused(line);
        assert!(
            d.message.contains("the line should have ended"),
            "{}: {}",
            line,
            d.message
        );
    }
    let d = refused("if think \"q\" using fast is \"yes\"\n  reply \"y\"");
    assert!(
        d.message.contains("the line should have ended"),
        "{}",
        d.message
    );
}

#[test]
fn a_stray_word_after_a_prompt_that_is_a_variable_is_the_old_error() {
    let d = refused("x = think using fast");
    assert!(
        d.message.contains("the line should have ended"),
        "{}",
        d.message
    );
}
