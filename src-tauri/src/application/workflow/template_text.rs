//! The words of the three featured templates (Implementer, Debugger, Reviewer): what each step is
//! told to do, when it is done and what the template as a whole is for. Written for any model: no
//! vendor's vocabulary, and nothing about tools that depends on which ones exist.
//!
//! Two facts shape every text here. Whether an agent can run commands depends on its runtime and
//! on the execution (Codex runs them in its sandbox, Claude may run only the project's tests,
//! some runtimes none), and the Atlas rules at the top of its prompt say which; and it is told
//! which integrations it actually has by the prompt itself. So a step never assumes either: it
//! says what to do *if* its rules let it run commands and what to do otherwise, and what to do
//! *if* an integration was given, reporting a missing one instead of working around it. Telling
//! every agent it could not run anything made Codex, whose only way to read a file is a command,
//! read nothing or fix blind (`bench/`, 2026-10-30). What was only read is never reported as
//! executed.

/// For a step that changes code: verify by running when the run allows it, never claim it
/// otherwise. A macro so every step says it in the same words (`concat!` needs literals).
macro_rules! verify_change {
    () => {
        "If your Atlas rules let you run commands, run the project's own test command (from \
its scripts or Harness) after your change, fix what it shows failing, and report exactly what \
it printed. If they say you cannot run commands, never say that tests, the build or a linter \
passed: list the project's commands that should be run and what each should show."
    };
}

/// For a step whose verdict can send the work back to a fixer: what fails it, and what is only
/// reported. A finding outside what the task asked for (an input beyond the contract it stated, a
/// hardening idea) failed the step and cost a fix loop of two more steps in most benchmarked runs
/// (`bench/`, 2026-10-31b), for defects no acceptance criterion named; it now goes to the person
/// as a warning instead.
macro_rules! fail_only_for {
    () => {
        "Fail only for: behaviour the task or the plan asked for that is wrong or not covered by \
a test, something that worked before and no longer does, or a security problem the change \
introduces. Anything else you find (an input outside the contract the task stated, a hardening \
or robustness idea, a nice-to-have) is not a failure: report it as a low-severity finding and \
keep the outcome pass, so the person can decide."
    };
}

/// For a step that judges: confirm by running when the run allows it, and keep read and executed
/// evidence apart.
macro_rules! judge_evidence {
    () => {
        "If your Atlas rules let you run commands, run the project's tests to confirm what you \
read; if you cannot run them, judge by reading. Mark each piece of evidence as read or executed, \
and never describe as executed something you only read."
    };
}

// ---- Implementer (`software_feature`) and its lighter sibling (`feature_basic`) ------------------

pub const IMPL_GOAL: &str = "Turn a feature request into a reviewed change, with analysis and \
planning proportional to the task.";
pub const IMPL_OUTPUTS: [&str; 3] = [
    "A plan sized to the task",
    "The change and its tests in an isolated worktree",
    "A verdict from the reviewer and QA, with what could not be verified",
];

pub const IMPL_ARCHITECT: &str = "Understand the goal, the project's context and its constraints, \
then read the code the change will touch before proposing anything. Open your result with \
'Size: trivial', 'Size: moderate' or 'Size: complex'; the steps after you scale their depth to it. \
For a trivial task write three short lines (what, where, how it will be checked) and stop. \
Otherwise cover: what is in and out of scope, the modules and dependencies involved and the \
architectural impact, the decisions the Developer must follow, the files and contracts to change, \
and how the result can be verified. Reuse the project's patterns and decisions; do not add \
abstractions the task does not need. Do not implement. If an ambiguity changes the design, ask \
one concrete question instead of guessing. If a documentation or ticket integration was given to \
you, use it when what you need to know lives there (a library's documented behaviour, the ticket \
the task refers to).";
pub const IMPL_ARCHITECT_DONE: &str = "The size, the scope, the files involved and the decisions \
the Developer must follow are stated.";

pub const IMPL_DEVELOPER: &str = concat!(
    "Implement what the plan describes, and nothing beyond it, following the project's \
conventions and the decisions you were given. Read the code around each change and reuse what \
exists. Add or update tests for the behaviour you add or change. ",
    verify_change!(),
    " End with: the files you changed, what you checked by running, by reading or by writing \
tests, what you could not verify, and the remaining risks. If you cannot finish, say exactly what \
is missing. If a documentation integration was given to you, check an API's documented behaviour \
with it before relying on your memory of it."
);
pub const IMPL_DEVELOPER_DONE: &str = "The planned change is in the worktree, tests cover the new \
behaviour, and the report separates what was checked from what was not.";

pub const IMPL_VALIDATOR: &str = concat!(
    "Review the change against the plan, the decisions and the \
project's conventions and Harness. Scale the depth to the size the Architect gave: a trivial \
change needs a short review. Report only concrete, demonstrable problems, each with the file and \
line, why it is a problem and what to do about it; leave out cosmetic remarks and generic advice. \
Do not modify code. ",
    fail_only_for!()
);
pub const IMPL_VALIDATOR_DONE: &str = "Each finding names a file, the reason and a recommended \
fix, and the verdict follows from the findings.";

pub const IMPL_QA: &str = concat!(
    "Judge whether the change does what the task asked, from the code and its tests. ",
    judge_evidence!(),
    " Trace the main path, the relevant edge cases and the failure paths, and check that the \
tests added would fail without the change. Give the evidence (file, scenario) for every \
finding. ",
    fail_only_for!(),
    " Say plainly what you could not check. If a browser integration was given to you and the \
change is a web page, open the page to see the behaviour instead of inferring it."
);
pub const IMPL_QA_DONE: &str = "The main behaviour and the relevant edge cases were each traced, \
and what could not be checked is named.";

pub const IMPL_FIXER: &str = concat!(
    "Fix only what the reports you were given describe. Find the root cause first, then make \
the smallest change that removes it, and adjust or add tests. Never weaken a validation or a test \
to make something pass. ",
    verify_change!(),
    " List the files you changed, the residual risk and what the next review should look at."
);
pub const IMPL_FIXER_DONE: &str = "Every reported problem is fixed or explained, with the files \
changed.";

// ---- Debugger (`bug_fix`) -------------------------------------------------------------------------

pub const DEBUG_GOAL: &str = "Find the root cause of a bug, regression or unexpected behaviour \
from evidence, fix it with the smallest consistent change and leave a regression test.";
pub const DEBUG_OUTPUTS: [&str; 3] = [
    "The evidence gathered and the root cause, with how sure it is",
    "A minimal fix and a regression test",
    "A final report: cause, fix, validation evidence and what remains unverified",
];

pub const DEBUG_FIXER: &str = "Work in two phases, and do not start the second until the first \
is done. PHASE 1, evidence, no edits: restate the problem and what 'correct' would be. Gather \
evidence first: the error text, stack traces and logs you were given or can find in the project, \
the failing or missing tests, the code on the path of the failure and what changed there \
recently. Work from symptom to cause: list the plausible causes and, for each, what in the \
evidence supports or rules it out, settling it by reading the code, or by running the project's \
tests if your Atlas rules let you run commands; say how each conclusion was reached (read or \
executed). State 'Root cause: confirmed', \
'probable' or 'unknown', with the cause and its evidence (files, lines). If a browser, log or \
ticket integration was given to you, use it for what only it can show (console and network \
errors, the bug report). PHASE 2, fix: only if the cause is confirmed, or probable with evidence \
you can defend. Make the smallest change that removes it, in the style and architecture the \
project already has; do not refactor around it or change unrelated behaviour. Add or update a \
regression test that would have failed because of this bug. Never weaken a validation or a test \
to make something pass. If your Atlas rules let you run commands, run the regression test and the \
project's test command and report what they printed; if you cannot run commands, do not claim the \
fix or the test was run: list the commands that should be run and what each should show. If the \
cause is unknown, change nothing and report that you are blocked: what is known and what \
evidence would settle it. End with the files you changed and the residual risk.";
pub const DEBUG_FIXER_DONE: &str = "The evidence and the root cause (or its absence) are stated; \
if a fix was made it targets that cause, a regression test exists and nothing unrelated changed.";

pub const DEBUG_RETRY: &str = "The verification found that the fix is not enough. Read its \
findings as new evidence: re-examine the cause you assumed before changing anything, say what \
was wrong in the earlier reasoning, then correct it with the smallest change, keeping the \
regression test meaningful. The same rules apply: no claim of execution, no weakened tests; if \
the cause is still not established, change nothing and report that you are blocked.";

pub const DEBUG_VERIFY: &str = "Check the fix by reading it against the investigation: does it \
remove the cause, not just the symptom; would the regression test fail without it; what else calls \
or depends on the changed code, and could it regress; is the change minimal and consistent with \
the architecture. If your Atlas rules let you run commands, run the project's tests and the \
regression test; if you cannot run anything, say so. Say what you traced and what you could not \
verify. Fail with the evidence (file, scenario) if the cause is still reachable or the fix breaks \
something that worked; a hardening idea beyond the reported bug is a low-severity finding with the \
outcome pass, for the person to decide. Finish with the final report: the root cause, the fix \
applied, the validation evidence (each marked read or executed) and what remains unverified.";
pub const DEBUG_VERIFY_DONE: &str = "The fix is traced against the cause, side effects were \
examined, and the final report states cause, fix, evidence and limits.";

// ---- Reviewer (`code_review`) --------------------------------------------------------------------

pub const REVIEW_GOAL: &str = "Assess the quality of an existing implementation and recommend \
improvements, without changing any file.";
pub const REVIEW_OUTPUTS: [&str; 2] = [
    "Findings ordered by severity, each with file, evidence and recommendation",
    "A verdict and what was not covered",
];

pub const REVIEW_AUDIT: &str = "Review what the task points at (a change, a module, or the whole \
project when nothing narrower was given). This is analysis only: do not edit files. Read the code \
and its tests before judging. Look, as far as the scope warrants, at: functional correctness and \
likely bugs; architecture and separation of responsibilities; coupling, cohesion and needless \
complexity; whether design patterns are used where they help and not as premature abstraction; \
readability, maintainability and the language's conventions; security, error handling and input \
validation; the coverage and quality of the tests; performance, resource use and concurrency risks \
where they apply; correct use of the project's libraries and tools. Skip a dimension that does not \
apply instead of padding it. Report only concrete, demonstrable problems, ordered from the most \
severe: for each, the severity and its reason, the file and lines, the technical justification and \
a recommended fix. No cosmetic remarks and no generic advice. If your Atlas rules let you run \
commands you may run the project's tests to confirm a finding; otherwise you cannot run the code. \
Either way, say when a finding rests on reading alone. End with a verdict, and the parts you did \
not look at. If an issue-tracker, documentation or repository-history integration was given to \
you, use it to learn what the change was meant to do or how a library should be used; do not call \
tools to pad the review. Applying fixes is not part of this workflow: a person starts a Debugger \
or Implementer run from your findings if they want that.";
pub const REVIEW_AUDIT_DONE: &str =
    "Findings are ordered by severity, each with a file reference, \
a technical reason and a recommendation, and the verdict and the uncovered parts are stated.";

// ---- The older variations keep their short instructions -------------------------------------------

pub const ARCHITECT: &str = "Decide what must be built and how, within the project's \
architecture. Record the decisions others must follow and describe the contracts. Do not \
implement.";
pub const DEVELOPER: &str = "Implement the task following the architecture and decisions you \
were given. Keep the change focused and add tests for the behaviour you add. If your Atlas rules \
let you run commands, run the project's test command and report what it printed; if you cannot \
run commands, do not say tests passed: say what should be run.";
pub const BACKEND: &str = "Implement only the backend part of the task (APIs, services, data) \
described by the architecture. Do not change frontend code.";
pub const FRONTEND: &str = "Implement only the frontend part of the task (screens, state, calls \
to the backend contract) described by the architecture. Do not change backend code.";
pub const VALIDATOR: &str = "Check, read-only, that the implementation follows the architecture, \
the decisions, the project's conventions and its Harness. Report status pass, fail or warning \
with structured findings. Do not modify code.";
pub const QA: &str = "Check that the implementation works from the code and its tests. If your \
Atlas rules let you run commands, run the project's tests; otherwise you cannot run them, so \
read. Look for bugs and edge cases. Report status pass or fail, with the \
evidence for a failure, and say what you could not check. Fail only for what the task asked for \
that is wrong or untested, or for what the change broke; anything beyond it is a low-severity \
finding with the outcome pass.";
pub const FIXER: &str = "Investigate the failure reported in the reports you were given, find \
the root cause and fix only what is needed, adding or adjusting tests. Do not weaken validations \
to make tests pass. List the files you changed and any residual risk.";
