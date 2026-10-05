//! The format (parsing) and the quality checks, on their own.

use super::model::{parse, ParseError};
use super::validate::{valid_name, validate, IssueCode, Severity};

fn codes(text: &str) -> Vec<IssueCode> {
    validate(&parse("my-skill", text).unwrap())
        .into_iter()
        .map(|i| i.code)
        .collect()
}

const GOOD: &str = "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in the finance module\n---\n# My skill\n\nRound every invoice total to two decimals.\nApply the tax table of the invoice country.\n";

#[test]
fn a_well_formed_skill_parses_and_has_no_issues() {
    let skill = parse("my-skill", GOOD).unwrap();

    assert_eq!(skill.name.as_deref(), Some("my-skill"));
    assert!(skill
        .description
        .as_deref()
        .unwrap()
        .starts_with("Use when"));
    assert!(skill.body.starts_with("# My skill"));
    assert_eq!(skill.body_lines, 4);
    assert!(validate(&skill).is_empty(), "{:?}", validate(&skill));
}

#[test]
fn the_optional_fields_of_the_format_are_read() {
    let skill = parse(
        "x",
        "---\nname: x\ndescription: d\nlicense: MIT\ncompatibility: needs git\nallowed-tools: Read Grep Bash(git:*)\nmetadata:\n  atlas-priority: 2\n  author: me\n  version: 1.5\n---\nbody",
    )
    .unwrap();

    assert_eq!(skill.license.as_deref(), Some("MIT"));
    assert_eq!(skill.compatibility.as_deref(), Some("needs git"));
    assert_eq!(skill.allowed_tools, ["Read", "Grep", "Bash(git:*)"]);
    assert_eq!(skill.priority(), 2);
    assert_eq!(skill.metadata["version"], "1.5");
    assert_eq!(skill.metadata["author"], "me");
}

#[test]
fn text_that_is_not_a_skill_is_a_parse_error_with_a_reason() {
    assert_eq!(parse("x", "no frontmatter"), Err(ParseError::NoFrontmatter));
    assert_eq!(
        parse("x", "---\nname: x\nno closing"),
        Err(ParseError::UnclosedFrontmatter)
    );
    assert!(matches!(
        parse("x", "---\nname: [unclosed\n---\nbody"),
        Err(ParseError::InvalidYaml(_))
    ));
    assert_eq!(
        parse("x", "---\n- a\n- b\n---\nbody"),
        Err(ParseError::NotAMapping)
    );
}

#[test]
fn a_windows_file_with_a_byte_order_mark_parses() {
    let skill = parse(
        "x",
        "\u{feff}---\r\nname: x\r\ndescription: Use when something happens in the system\r\n---\r\nbody\r\n",
    )
    .unwrap();

    assert_eq!(skill.name.as_deref(), Some("x"));
    assert!(skill.body.starts_with("body"));
}

#[test]
fn names_follow_the_format() {
    for ok in ["a", "billing-rules", "v2-api", "a1"] {
        assert!(valid_name(ok), "{ok}");
    }
    for bad in [
        "",
        "Billing",
        "-a",
        "a-",
        "a--b",
        "a_b",
        "a b",
        &"a".repeat(65),
    ] {
        assert!(!valid_name(bad), "{bad}");
    }
}

#[test]
fn missing_and_wrong_names_and_descriptions_block_the_skill() {
    let no_name = validate(
        &parse(
            "x",
            "---\ndescription: Use when a thing happens in the app\n---\nbody",
        )
        .unwrap(),
    );
    assert!(no_name
        .iter()
        .any(|i| i.code == IssueCode::MissingName && i.severity == Severity::Error));

    let bad_name =
        codes("---\nname: Bad_Name\ndescription: Use when a thing happens in the app\n---\nbody");
    assert!(bad_name.contains(&IssueCode::InvalidName));

    let no_description = validate(&parse("x", "---\nname: x\n---\nbody").unwrap());
    assert!(no_description
        .iter()
        .any(|i| i.code == IssueCode::MissingDescription && i.severity == Severity::Error));
}

#[test]
fn a_name_that_differs_from_its_folder_is_a_warning() {
    let issues = validate(
        &parse(
            "folder",
            "---\nname: other\ndescription: Use when a thing happens in the app\n---\nbody",
        )
        .unwrap(),
    );

    let mismatch = issues
        .iter()
        .find(|i| i.code == IssueCode::NameMismatch)
        .unwrap();
    assert_eq!(mismatch.severity, Severity::Warning);
}

#[test]
fn a_description_must_say_when_and_be_specific() {
    let no_triggers = codes("---\nname: my-skill\ndescription: Helps with invoices and billing totals in finance\n---\nbody");
    assert!(no_triggers.contains(&IssueCode::MissingTriggers));

    let generic = codes("---\nname: my-skill\ndescription: Use when needed\n---\nbody");
    assert!(generic.contains(&IssueCode::OverlyGeneric));

    let long = codes(&format!(
        "---\nname: my-skill\ndescription: Use when {}\n---\nbody",
        "something ".repeat(200)
    ));
    assert!(long.contains(&IssueCode::DescriptionTooLong));
}

#[test]
fn an_enormous_skill_is_too_large_and_a_focused_one_is_not() {
    let lines = (0..600)
        .map(|i| format!("Instruction number {i} about something specific"))
        .collect::<Vec<_>>()
        .join("\n");
    let big = codes(&format!(
        "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in finance\n---\n{lines}"
    ));
    assert!(big.contains(&IssueCode::TooLarge));
    assert!(!codes(GOOD).contains(&IssueCode::TooLarge));
}

#[test]
fn a_skill_that_asks_for_every_tool_is_flagged() {
    let tools = (0..15)
        .map(|i| format!("Tool{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let issues = codes(&format!(
        "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in finance\nallowed-tools: {tools}\n---\nbody"
    ));

    assert!(issues.contains(&IssueCode::ToolExplosion));
}

#[test]
fn the_same_instruction_twice_is_flagged_outside_code() {
    let repeated = codes(
        "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in finance\n---\nAlways format the code before committing it.\nSomething else entirely different here.\nAlways format the code before committing it!\n",
    );
    assert!(repeated.contains(&IssueCode::DuplicatedInstructions));

    let in_code = codes(
        "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in finance\n---\n```\nrun the formatter before committing it\nrun the formatter before committing it\n```\nDone.\n",
    );
    assert!(!in_code.contains(&IssueCode::DuplicatedInstructions));
}

#[test]
fn instructions_that_say_opposite_things_are_flagged() {
    let conflicting = codes(
        "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in finance\n---\nAlways round the invoice total to two decimals.\nNever round the invoice total to two decimals.\n",
    );
    assert!(conflicting.contains(&IssueCode::ConflictingInstructions));

    let fine = codes(
        "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in finance\n---\nAlways round the invoice total to two decimals.\nNever commit generated files into the repository.\n",
    );
    assert!(!fine.contains(&IssueCode::ConflictingInstructions));
}

#[test]
fn a_section_with_nothing_in_it_is_flagged() {
    let empty = codes(
        "---\nname: my-skill\ndescription: Use when changing invoices, taxes or billing totals in finance\n---\n# Rules\n\nRound totals to two decimals always.\n\n## Examples\n\n## Notes\n\nSome real note about the module here.\n",
    );

    assert!(empty.contains(&IssueCode::UnusedSections));
    assert!(!codes(GOOD).contains(&IssueCode::UnusedSections));
}
