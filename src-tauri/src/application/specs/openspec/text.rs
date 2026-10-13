//! What an agent is told about each `OpenSpec` document. Adapted from the `spec-driven` schema of
//! `OpenSpec` (MIT, Fission-AI), keeping its rules and dropping what only makes sense inside a
//! terminal (`openspec list`, slash commands): an Atlas agent cannot run them and is told what it
//! needs instead.

pub const PROPOSAL_INSTRUCTION: &str = "\
Write proposal.md: why this change is needed and what it changes. Keep it concise (one or two \
pages) and about the why, not the how (that belongs in design.md).

Sections:
- `## Why`: one or two sentences, at least 50 characters: the problem or the opportunity, and why now.
- `## What Changes`: a bullet list of specific changes: new behaviour, modified behaviour, removals. \
Mark breaking changes with **BREAKING**.
- `## Capabilities`: the contract with the next document.
  - `### New Capabilities`: one bullet per capability, written exactly as \
`- `capability-path`: what it covers`. Name each capability for a durable behaviour of the system \
(`user-auth`), not for the work of this change (`add-login-endpoint`). Use kebab-case for every path \
segment you introduce (`user-auth` or `identity/user-auth`) and follow how the project already \
organises its specs. Each one will get a file `specs/<capability-path>/spec.md`.
  - `### Modified Capabilities`: existing capabilities (the exact path under `openspec/specs/`) \
whose REQUIREMENTS change. Only when behaviour at the requirement level changes, not for \
implementation details. Leave the section empty if there are none.
  Every change declares at least one capability. Do not invent a requirement to have one.
- `## Impact`: the code, APIs, dependencies or systems affected.";

pub const PROPOSAL_TEMPLATE: &str = "\
# Proposal

## Why

<!-- the motivation: what problem this solves, and why now -->

## What Changes

<!-- specific changes: new capabilities, modifications, removals; mark **BREAKING** -->

## Capabilities

### New Capabilities
- `<capability-path>`: <what this capability covers>

### Modified Capabilities
- `<existing-capability-path>`: <what requirement is changing>

## Impact

<!-- affected code, APIs, dependencies, systems -->";

pub const SPECS_INSTRUCTION: &str = "\
Write the specification files that define WHAT the system should do: one file per capability listed \
in the proposal's Capabilities section, at `specs/<capability-path>/spec.md`, keeping the full path \
(`user-auth` or `identity/user-auth`). Use the exact path the proposal gives. For a modified \
capability use the exact existing path under `openspec/specs/`.

A spec is a behaviour contract, not an implementation plan.
- Good: observable behaviour people or other systems rely on; inputs, outputs and error conditions; \
external constraints (security, privacy, reliability, compatibility); scenarios that can be tested.
- Avoid: internal class or function names, library or framework choices, step-by-step implementation. \
Quick test: if the implementation can change without changing what can be observed, it does not belong \
in the spec.

Format (OpenSpec's delta format):
- Sections by operation, with `##` headers: `## ADDED Requirements` (new behaviour), \
`## MODIFIED Requirements` (changed behaviour: include the FULL updated requirement, copying the existing \
block from the main spec and editing it, with the header text unchanged), `## REMOVED Requirements` \
(include **Reason** and **Migration**), `## RENAMED Requirements` (`FROM:` and `TO:` lines).
- A NEW capability's file starts with `## Purpose`: one or two sentences, at least 50 characters, on \
what the capability is for. Do not add a Purpose to the delta of an existing capability.
- Each requirement: `### Requirement: <name>` followed by a statement that uses SHALL or MUST (not \
should or may), in at most 500 characters. One behaviour per requirement: move examples and edge cases \
into scenarios.
- Every requirement has at least one scenario, with EXACTLY four hashes: `#### Scenario: <name>`, then \
`- **WHEN** <condition>` and `- **THEN** <expected outcome>` (and `- **AND** …` if needed). Three hashes \
or other bullets do not count as a scenario.
- Cover the error and boundary cases the request implies as scenarios of their own.
- Never invent a business rule the person did not give. If one is missing, ask (see ASKING THE PERSON).";

pub const SPECS_TEMPLATE: &str = "\
# Spec Delta

## Purpose
<!-- new capabilities only: one or two sentences (50+ characters) on what it is for -->

## ADDED Requirements

### Requirement: <requirement name>
The system SHALL <observable behaviour>.

#### Scenario: <scenario name>
- **WHEN** <condition>
- **THEN** <expected outcome>";

pub const DESIGN_INSTRUCTION: &str = "\
Write design.md: HOW the change will be implemented. It explains the technical approach; the proposal \
covers why and what, and the approved specs cover the requirements, so point to them instead of \
restating them.

Sections:
- `## Context`: only the current state and constraints needed to explain the approach.
- `## Goals / Non-Goals`: what this design achieves and what it leaves out, as design-level boundaries.
- `## Decisions`: the key technical choices with the reason for each and the alternatives considered \
(why X over Y).
- `## Risks / Trade-offs`: known limitations and what could go wrong, as `[Risk] → Mitigation`.
- `## Migration Plan`: how to roll it out and back, when that applies.
- `## Open Questions`: only unknowns that can safely be answered later without changing the specs, the \
approach or the task breakdown. If an unknown would change any of them, ask the person now.

Focus on architecture and approach, not line-by-line code. Base what you say about the project on what \
the project's context shows; mark anything else as an assumption.";

pub const DESIGN_TEMPLATE: &str = "\
# Design

## Context

<!-- current state and constraints; see proposal.md for the motivation -->

## Goals / Non-Goals

**Goals:**
<!-- what this design aims to achieve -->

**Non-Goals:**
<!-- what is explicitly out of scope -->

## Decisions

<!-- key decisions with the reason and the alternatives considered -->

## Risks / Trade-offs

<!-- known risks and trade-offs -->";

pub const TASKS_INSTRUCTION: &str = "\
Write tasks.md: the checklist that breaks the implementation into trackable work, in the order it \
should be done.

The apply stage reads checkboxes to track progress, so follow this format exactly:
- Group related tasks under numbered `##` headings (`## 1. Setup`).
- Every tracked task is a checkbox with its number: `- [ ] 1.1 Description`. A line that is not a \
checkbox is not tracked.
- Tasks are small enough to finish in one session, ordered by dependency. Say a dependency with \
`depends on 1.2`.
- Every task says how to verify it is done (a test, a command, an observable behaviour or a delivered \
artifact), inside the task's own line.
- When a task implements a requirement of the specs, end the line with `(Requirement: <exact \
requirement name>)`. Atlas uses this to show which requirements the tasks cover. Do not cite a \
requirement that is not in the specs.
- Each group lands the tests and documentation its own work calls for; do not collect them into a last \
group. A final group is only for integration checks.
- If the design leaves an open question that would change what gets built, ask the person before you \
write the list.";

pub const TASKS_TEMPLATE: &str = "\
# Tasks

## 1. <task group name>

- [ ] 1.1 <task, and how to verify it> (Requirement: <requirement name>)
- [ ] 1.2 <task, and how to verify it>

## 2. <task group name>

- [ ] 2.1 <task, and how to verify it>";

pub const REVIEW_INSTRUCTION: &str = "\
Review the approved specification documents for gaps a person would regret finding during \
implementation. You only read and report: you change nothing.

Look for, where they apply:
- ambiguity: a requirement that two people could read differently;
- contradiction: two requirements, or a requirement and the design, that cannot both hold;
- missing_scenario: a requirement with no scenario that can be tested, or an error or boundary case \
the request implies and no scenario covers;
- business_rule: a rule the documents depend on and nobody stated;
- risk: an architectural or operational risk the design ignores;
- dependency: something the work needs (a service, a migration, a permission) that is not planned;
- coverage: a requirement no task implements, or a task that implements nothing asked;
- decision: a technical choice that still needs the person's confirmation.

Be honest about what you know: quote the document for what it says (`evidence`), and say plainly in \
`rationale` when something is your inference. Do not report style preferences. Report only what you \
can point to. If a finding needs a decision only the person can make, put the question in `question`.";

/// The shape the reviewing agent answers in.
pub const REVIEW_FORMAT: &str = "\
End your answer with ONE block like this and nothing after it (`artifact` is one of proposal, specs, \
design, tasks; `severity` is high, medium or low; `category` is one of the names above):

```atlas-review
{\"summary\":\"one or two sentences\",\"findings\":[{\"category\":\"ambiguity\",\"severity\":\"medium\",\"artifact\":\"specs\",\"reference\":\"Requirement: User can switch the theme\",\"title\":\"short title\",\"description\":\"what is wrong\",\"evidence\":\"a quote from the document\",\"rationale\":\"why it matters, and what is inference\",\"question\":\"only if the person must decide\",\"suggestion\":\"how it could be resolved\"}]}
```

If you find nothing worth reporting, send an empty `findings` list. Do not invent findings to have \
something to say.";
