use super::*;
use crate::domain::interaction::InteractionKind;

fn detect(text: &str) -> InteractionDetection {
    LayeredDetector.detect(&InteractionSignals { text })
}

fn pauses(text: &str) -> Option<InteractionKind> {
    let found = detect(text);
    should_pause(&found).then_some(found.kind).flatten()
}

#[test]
fn a_clarification_in_a_structured_block_is_read_as_one() {
    let found = detect(
        "I need one thing before I can go on.\n\n```atlas-interaction\n\
         {\"type\":\"clarification\",\"question\":\"Which API should I use?\",\
         \"options\":[\"/api/users\",{\"id\":\"c\",\"label\":\"/api/customers\"}]}\n```",
    );
    assert!(should_pause(&found));
    assert_eq!(found.kind, Some(InteractionKind::Clarification));
    assert_eq!(found.source, DetectionSource::Structured);
    assert_eq!(found.question, "Which API should I use?");
    let ids: Vec<&str> = found.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["/api/users", "c"]);
}

#[test]
fn an_agent_cannot_invent_the_buttons_of_an_approval() {
    let found = detect(
        "```atlas-interaction\n{\"type\":\"approval\",\"question\":\"Apply to main?\",\
         \"options\":[\"apply-to-main\"]}\n```",
    );
    let ids: Vec<&str> = found.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(ids, ["approve", "reject"]);
}

#[test]
fn a_block_with_an_unknown_type_or_no_question_is_ignored() {
    assert!(!should_pause(&detect(
        "```atlas-interaction\n{\"type\":\"sudo\",\"question\":\"Delete?\"}\n```"
    )));
    assert!(!should_pause(&detect(
        "```atlas-interaction\n{\"type\":\"approval\"}\n```"
    )));
    assert!(!should_pause(&detect(
        "```atlas-interaction\nnot json\n```"
    )));
}

#[test]
fn a_result_after_the_block_means_the_agent_finished() {
    let text = "```atlas-interaction\n{\"question\":\"Which API?\"}\n```\n\
                Never mind, found it.\n```atlas-result\n{\"outcome\":\"implemented\"}\n```";
    assert!(!should_pause(&detect(text)));
}

#[test]
fn a_result_alone_is_never_a_question_even_if_the_text_asks_one() {
    let text =
        "Done. Should I also add tests?\n```atlas-result\n{\"outcome\":\"implemented\"}\n```";
    assert!(!should_pause(&detect(text)));
}

#[test]
fn a_direct_question_to_the_person_is_a_clarification() {
    assert_eq!(
        pauses("I looked at both modules.\n\nWhich API should I use?"),
        Some(InteractionKind::Clarification)
    );
    assert_eq!(
        pauses("Qual endpoint devo utilizar?"),
        Some(InteractionKind::Clarification)
    );
}

#[test]
fn asking_for_a_go_ahead_is_an_approval() {
    assert_eq!(
        pauses("May I modify UserService?"),
        Some(InteractionKind::Approval)
    );
    assert_eq!(
        pauses("Found two approaches. Do you want to continue?"),
        Some(InteractionKind::Approval)
    );
}

#[test]
fn asking_to_be_allowed_is_a_permission() {
    assert_eq!(
        pauses("I need to edit this file. Do you allow me to modify it?"),
        Some(InteractionKind::Permission)
    );
    assert_eq!(
        pauses("Preciso modificar este arquivo. Deseja permitir?"),
        Some(InteractionKind::Permission)
    );
}

#[test]
fn a_runtime_prompt_is_a_runtime_confirmation() {
    assert_eq!(
        pauses("Apply these changes? [y/N]"),
        Some(InteractionKind::RuntimeConfirmation)
    );
    assert_eq!(
        pauses("Allow this operation?"),
        Some(InteractionKind::RuntimeConfirmation)
    );
}

#[test]
fn reporting_what_was_checked_is_not_a_question() {
    assert_eq!(pauses("I checked whether this approach would work."), None);
    assert_eq!(pauses("I checked whether we can use this API."), None);
}

#[test]
fn a_question_that_is_not_the_end_of_the_answer_does_not_pause() {
    assert_eq!(
        pauses("Question? Anyway, I completed the implementation."),
        None
    );
    assert_eq!(
        pauses("Which API should I use? I went with /api/users and implemented it."),
        None
    );
}

#[test]
fn a_rhetorical_question_without_an_addressee_does_not_pause() {
    assert_eq!(
        pauses("Why does this fail? The mutex was never released."),
        None
    );
    assert_eq!(pauses("The cache is stale, so what then?"), None);
}

#[test]
fn an_offer_of_more_work_after_the_job_is_not_a_block() {
    assert_eq!(
        pauses("The endpoint is implemented and tested.\n\nWould you like me to also add docs?"),
        None
    );
}

#[test]
fn a_question_inside_code_is_not_a_question() {
    assert_eq!(
        pauses("Here is the query:\n```sql\nselect 1 where a ? b\n```"),
        None
    );
}

#[test]
fn a_blocking_question_keeps_what_came_before_as_context() {
    let found =
        detect("Two modules expose users.\nThe old one is deprecated.\n\nWhich one should I use?");
    assert!(should_pause(&found));
    assert!(found.context.contains("deprecated"));
    assert_eq!(found.question, "Which one should I use?");
}

#[test]
fn a_question_followed_by_a_sign_off_is_still_the_end_of_the_message() {
    // A real run: the plan, the question, a readiness line and a stray signature.
    let text = "## Plan\n\n1. Add the column\n2. Fix the webhook\n\n---\n\n\
                Would you like me to proceed with implementation? I have a clear plan and am \
                ready to code.\n\nROBERT!";
    let found = detect(text);
    assert!(should_pause(&found));
    assert_eq!(found.kind, Some(InteractionKind::Approval));
    assert_eq!(
        found.question,
        "Would you like me to proceed with implementation?"
    );
    // The whole message, plan included, travels with the question to be read in Atlas.
    assert!(found.document.contains("Fix the webhook"));
}

#[test]
fn a_sign_off_does_not_turn_a_statement_into_a_question() {
    assert_eq!(pauses("The fix is in place.\n\nROBERT!"), None);
    assert_eq!(pauses("All done. I am ready to code more if needed."), None);
}

#[test]
fn an_approval_asked_in_portuguese_is_recognised() {
    assert_eq!(
        pauses("Aprova o plano para eu iniciar a implementação?"),
        Some(InteractionKind::Approval)
    );
    assert_eq!(
        pauses("Podemos seguir com a opção A?"),
        Some(InteractionKind::Approval)
    );
    assert_eq!(
        pauses("Is this plan approved, shall we go ahead?"),
        Some(InteractionKind::Approval)
    );
}

#[test]
fn an_unfinished_message_ending_on_any_question_is_a_question_for_the_person() {
    let found =
        detect_unfinished_question("Resumo do plano acima.\n\nFaz sentido para você assim?")
            .expect("it ends on a question");
    assert!(
        found.confidence < MIN_CONFIDENCE,
        "but the strict detector would not pause"
    );
    assert!(detect_unfinished_question("Tudo implementado.").is_none());
}

#[test]
fn a_question_followed_by_a_summary_is_found_in_an_unfinished_message() {
    // A real run: the question sits mid-message, with two statements after it.
    let text = "## Plan\n\n1. Add the column\n\n**Ready to implement?** This plan addresses all \
                findings. The key behavioral change: each charge creates its own record.";
    assert!(
        !should_pause(&detect(text)),
        "a finished-looking message is not paused for"
    );

    let found = detect_unfinished_question(text).expect("there is a question in the tail");
    assert!(found.question.contains("Ready to implement"));
    assert_eq!(found.kind, Some(InteractionKind::Approval));
    assert!(found.document.contains("Add the column"));
}
