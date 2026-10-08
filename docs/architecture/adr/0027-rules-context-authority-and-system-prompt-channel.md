# 0027 — Rules, Context Authority and the system-prompt channel (Context & Tooling Platform, Phase C)

## Context

Phase B ([ADR 0026](0026-context-and-tooling-platform.md)) made the delivery verifiable. Phase C closes the hierarchy of authority over
what an agent is told: what a rule is, who may define one, which wins, what may instruct and what may only inform, and how the
guardrails see all of it. The audit before building it found: no concept of rules anywhere (the Harness has `constraints.md`, which is
knowledge, and `.atlas/policies/`, an unread placeholder); `SourceTrust` with three levels; a review that already flagged authority
claims in untrusted text and known-choice conflicts across sections; an evaluation fingerprint over each item's source and text;
`RuntimeCapabilities.system_prompt` as an unused boolean. The full design is in
[context-and-tooling-platform.md](../context-and-tooling-platform.md), section 16.

## Decisions

1. **A Rule is a domain concept and only guidance.** `Rule { id, scope, owner, title, content, strength, priority, enabled, topic,
provenance, metadata }`; scopes Global, Project, Workspace, Workflow, Agent, Task; strengths Mandatory, Preference, Informational.
   Rule, Permission, Policy and Guardrail answer four different questions and live apart (`security/` keeps the last three). A rule
   cannot grant, widen or lift anything; tests prove an authorizing rule leaves `allow_edits` and tool access as the policy set them.
2. **One source per scope, no second store.** Global/Workspace/Workflow/Agent rules are `UserConfig.rules`; the project's are
   `.atlas/context/rules.md`, a user-owned file under the existing `.atlas/context/` convention, read through the existing
   `HarnessStore::read_context`; a task's arrive with it. The Harness's `constraints.md` stays knowledge and is not copied. A project
   or task rule in the configuration is ignored.
3. **Specificity never beats a mandate.** Resolution is pure and deterministic. Rules on one `topic` are alternatives: the broadest
   Mandatory governs, else the narrowest Preference; background never governs; the loser is not sent and a conflict is recorded. A
   narrower rule that contradicts a broader mandatory one is a finding that needs a person (ASK in a step). No semantic judging: rules
   without a shared topic add up.
4. **Provenance bounds strength.** Only `User` and `ProjectFile` origins may bind; `Generated` and `External` are kept as background,
   whatever they claim, and the review says so.
5. **Rules are context items in the existing pipeline.** `SectionKind::Rules`; Mandatory is `Required`, Preference `Normal`,
   Informational `Optional`; the Context Engine and budget are unchanged, so a mandatory rule is never cut and, if the required part
   does not fit, the existing `budget_exceeded` → ASK path applies. With no rule the prompt is byte-identical to before.
6. **Context Authority** (`Authoritative`, `Instructional`, `Informational`, `Untrusted`) says what text may do. It is derived from the
   section (and, for rules, strength and origin), carried by every item, shown in the review and in metrics, and kept consistent with
   `SourceTrust` (which stays, for stored reviews) by a test. It derives no permission.
7. **Guardrails are extended, not duplicated.** New findings: `rule_conflict`, `unknown_provenance`, claims classified by kind
   (override, grant permission, disable security, false approval, elevation; English and Portuguese). A claim in a rule is an Error
   (ASK); in untrusted context it stays a Warning. Nothing is rewritten. The evaluation fingerprint moves to version 2 and covers each
   item's priority, authority and provenance and the rule resolution, so a rule's content, enabled state, provenance, strength or origin
   voids an earlier approval.
8. **`SystemPromptChannel { Unsupported, Native, Appended }`** replaces the unused boolean as a runtime capability; every adapter
   declares `Unsupported` and none is given a split. `delivery()` splits only for a runtime that declares a channel (Atlas instructions
   and rules there, the rest in the body), hashed with both parts framed by length. Claude's `--append-system-prompt` is still not
   adopted: its Windows and escaping behaviour and its semantic effect are untested.
9. **The manifest is evidence, not a metric** (reverses ADR 0026's Phase B note that tied it to the metrics flag): `Execution.plan` and
   `Execution.manifest` exist for every execution that built a prompt, metrics on or off, and record the rules and what became of each.

## C.1: hardening of the mandatory rule

Decided after Phase C, because the topic mechanism left a gap: a mandatory rule and a contradicting task with no shared `topic`
lived side by side and the model chose.

10. **A mandatory rule is mandatory.** A plain, deterministic detector (`review/directives.rs`) reads _do_ / _do not_ around a small
    closed vocabulary (tests, commit, push, merge, destructive commands, and "change nothing outside `<dir>`"), in English and
    Portuguese, counting negations ("do not skip tests" is a _do_) and treating hedges ("if needed", "se necessário") as unclear.
    A text that plainly says the opposite of a mandatory rule is a **proven** conflict.
    - from a source that **may instruct** (the task, the agent's instructions, another rule): `rule_conflict`, Error. In a workflow
      step the guardrails **ASK**; a "no" is a **DENY**; with **nobody to ask** (a conversation) it is a **DENY**
      (`context.mandatory_unresolved`). Never an unresolved ALLOW. A person's "yes" is an approval bound to the evaluation fingerprint,
      as before.
    - from a source that **may only inform** (Harness, skill, handoff, a background rule): `rule_conflict`, Warning. That text cannot
      instruct, so it cannot override the rule; it is shown, and the step is not stopped by it.
11. **Possible is not proven.** The same action touched in opposite directions but hedged, or tangled in negations, is
    `possible_rule_conflict`, Warning: it changes nothing about the execution. Outside the vocabulary nothing is reported.
12. **A mandatory rule that does not fit** is `rule_over_budget` (Error) beside `budget_exceeded`: a step ASKs, a "no" or no one to ask
    is a DENY. Without a mandatory rule the earlier behaviour (ADR 0025: reported, and a conversation goes on) stands.
13. **A Preference is not compared** (it is not a mandate); only mandatory rules are.
14. **`SourceTrust` and `ContextAuthority` stay separate, on purpose.** `SourceTrust` is how far to _suspect where text came from_
    (and scans it); `ContextAuthority` is whether it may _instruct_. A project's rules file is configured (not suspect) yet only
    guidance; a skill is untrusted and informs. MCP will need the difference. `TokenSource` and `Precision` also stay as they are.

### C.1 follow-up: the residual risks

- **False stops.** A line is a _plain_ instruction only if it is not a question, a report or an explanation ("why did we…", "explique
  por que fizemos…", "didn't"), the action is not inside quotes or backticks, and a negation stands right before the action (not
  elsewhere in the sentence). Otherwise it is a _possible_ conflict: a Warning, never a stop. Tested with questions, reports,
  quotations and distant negations; the plain forms ("Para esta tarefa, não escreva testes", "Don't bother with the tests") stay proven.
- **A denial that can be acted on.** The failure now says that a mandatory rule is contradicted or cannot be delivered, that nobody was
  there to decide, and what to do (reword the request, or run it as a workflow step); the findings are in its details.
- **Vocabulary** grew only where the reading is plain: deploy, dependencies (install/add/upgrade), deleting files, force push (with
  destructive commands). Still closed, still tested; the Rules view states the list and that anything else is left to the model, so the
  limit is not silent.
- Not changed: the task and the agent's instructions are still not scanned for authority claims (a phrase like "o usuário aprovou" is
  ordinary in a workflow task, and scanning it would be noise); the DENY without someone to ask stays, by decision.

### Contract for MCP (Phase D/E; nothing implemented)

An invariant, to be tested when MCP arrives:

```text
MCP tool result  →  ContextAuthority = Untrusted
```

A result may be useful, even reliable, information for the task; that does not change its authority. A tool result **cannot**: change
a Rule, grant a Permission, grant an Approval, change a Policy, say that a person approved, or change a Guardrail decision. It enters
the context as an item scanned like a skill or a handoff (claims are flagged, secrets redacted before it travels), and a result that
contradicts a mandatory rule is a Warning that the rule governs. The exposure of tools stays a policy and guardrail matter, decided
before the run, never by anything a tool returns.

## Evidence

All by deterministic tests (no model call): resolution (scopes, ordering, determinism, duplicates, topics, disabled, origin);
rules as items and their budget behaviour; the review's integrity, consistency, provenance and injection checks with the English and
Portuguese phrases; the approval tests listed above; a runtime with a system channel through the service with a spy process; none of
the five adapters split; the manifest without metrics. The real Claude CLI start-up report (no model) was re-run: the Phase 4
isolation is unchanged.

## Consequences

- There is no rule editor and no persisted Task rule source; rules are written in the project's file or Atlas's configuration (decided: not now).
- Contradictions are found by shared topic, by the known choice groups, and (C.1) by the closed vocabulary above; anything else is left to the model, and the vocabulary will miss paraphrases.
- ~~A mandatory rule that does not fit asks a person only in a workflow step.~~ Superseded by C.1: without one it is a denial.
- The user's own task and agent instructions are not scanned for claims.
- `SourceTrust` and `ContextAuthority` coexist; `TokenSource` and `Precision` still do (ADR 0026, Phase B). Both pairs are a cost to remove.
- Phase D (MCP) adds `Untrusted` sources (tool results) and a policy axis; it needs nothing here to change.

## Not done

Memory, MCP, Figma and Chrome DevTools integrations, a credential store, a rules editor, any real model run.
