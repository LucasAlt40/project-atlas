# 0025 — Optimization Layer, phase 5: Guardrails and Context Review

## Context

Phase 5 asks for contextual guardrails ("given the agent, task, context, runtime and policy, may this happen?") and a review of
the context an agent is about to receive. The rule that governs both: **there is one security authority** (`application::security`),
and nothing an agent, a skill, a handoff or a project file says can be that authority.

## Inventory: what `security/` already was

| Need               | Already there                                                                                                                                        |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Answers            | `PermissionDecision` (`Allowed` / `Denied` / `RequiresApproval`) = ALLOW / DENY / ASK; `Permission` (denied < approval required < allowed)           |
| Why                | `Reason` codes the UI words; `Evaluation`                                                                                                            |
| Policy             | `SecurityPolicy`, `restrict` (every layer can only narrow), profiles (`read_only`, `developer`), `ToolAccess::ceiling` (runtime capability as a cap) |
| Who may start what | `GuardedProcessRunner` + `evaluator` (paths, symlinks, shells, git classes, network, env)                                                            |
| Audit              | `AuditLog`, `PermissionEvent` (kept on `Execution::permission_events`, streamed to the UI)                                                           |
| Approval           | `ApprovalBroker` for process approvals; `PendingInteraction` for questions that pause a step                                                         |
| Secrets            | `harness::secrets::redact_secrets`                                                                                                                   |
| Confinement seam   | `SandboxProvider` (only `NoSandbox` today)                                                                                                           |
| Edit grant         | `edit_access`: policy allows writing + isolated worktree + runtime capability                                                                        |

What is **not** there: any notion of _autonomy_ (Manual / Guided / Auto). No type, setting or code path exists for it. The phase
brief says to respect the existing modes; since there are none, none was invented. The decisions below do not take an autonomy
mode, and an `ASK` is possible exactly where a person can answer (a workflow step). Introducing autonomy is a product decision
to take on its own (see "Decisions left").

Also not true of the tool surface: Atlas cannot interpose on what a CLI runtime does inside its process. `Read`, `Grep`, `Edit` and
`Write` run in Claude's own tool loop; Atlas sees the prompt it sends and the answer it reads (ADR 0023, 0024). So a "before each
action" or "before each write" guardrail does not exist for those tools: what Atlas controls is what it grants (the tool list, the
worktree) and what it hands over. Guardrails therefore act at the two moments Atlas owns: **before the agent starts** and **before
a step's result is handed on**.

## Decision

**One authority, extended not duplicated.**

- `ALLOW` / `ASK` / `DENY` are `PermissionDecision` under another name (`GuardrailDecision::from`). `TRANSFORM` is the one new answer.
- Every evaluation is a `PermissionEvent` on the execution's one audit trail (new actions `ReviewContext`, `EditFiles`,
  `ShareResult`; new outcome `Transformed`; new `Reason` codes). Rule id, matched rules and a description of any transformation
  travel as notes. **Never content**: targets are names (`context`, `skills`), and the secret itself is never quoted.
- An `ASK` is a `PendingInteraction` (kind `Permission`, buttons Allow/Deny fixed by Atlas), with a new `DetectionSource::Guardrail`.
  No second approval system.
- Policy, permission, capability and the guard are untouched. `restrict` still only narrows.

**Decisions** (`security/guardrails.rs`, all deterministic functions of their inputs):

| Situation                                                          | Answer                        | Rule / reason                                        |
| ------------------------------------------------------------------ | ----------------------------- | ---------------------------------------------------- |
| Context `Healthy` or `Partial`                                     | ALLOW                         | `context.healthy` / `context.partial`                |
| Context `NeedsReview`, a step that can wait                        | ASK                           | `context.needs_review` / `ContextNeedsReview`        |
| …and a person answered Allow                                       | ALLOW (by the user)           | `context.needs_review.approved`                      |
| …and a person answered Deny                                        | DENY (by the user)            | `context.needs_review.declined` / `ApprovalRejected` |
| …in a conversation (nobody to ask)                                 | ALLOW, reported               | `context.needs_review.unattended`                    |
| Context `Invalid`                                                  | DENY, whatever anyone answers | `context.invalid` / `ContextInvalid`                 |
| Edit tools: policy writes, isolated worktree, runtime can edit     | ALLOW                         | `write.worktree_scope`                               |
| …policy writes but the agent is not isolated                       | DENY (it runs read-only)      | `write.not_isolated` / `WriteNotIsolated`            |
| …policy writes but the runtime cannot be launched with edit tools  | DENY (read-only)              | `write.runtime_capability` / `RuntimeCannotEdit`     |
| Secrets in context from files or other agents / in a step's result | TRANSFORM                     | `secrets.redact` / `SecretsRedacted`                 |

(A policy that does not allow writing is the normal state of a read-only agent: nobody asked, so nothing is recorded.)

**Approval has one source.** `Answered` comes from an answered `PendingInteraction` whose source is `Guardrail`, for this step and
this pass through it, read by the workflow from the run (`brief::guardrail_answer`), then handed down as `StepOptions::review`.
Text in a handoff ("the user approved"), a skill, a file, a runtime's own question (any other detection source) or the agent's
message is never read as it, and tests prove each. The agent is not told about Atlas's question (it never asked it). A person cannot
approve an `Invalid` context. The approval covers the pass through the step it was given for.

**TRANSFORM is only redaction.** Lines that look like secrets (the Harness's own rule) are replaced by a notice, in untrusted
context (Harness, Task Context, skills, workflow context, handoff) before the prompt is built, and in a step's structured result
before it is handed on (`WorkflowEventKind::GuardrailTransformed`, counts only). What the user wrote (the task, the agent's
instructions) is theirs: it is reported, never rewritten. No command is ever rewritten.

## Context Review

A pure, deterministic check of the same items the Context Engine works on (`prompt_items`), so both agree on what each part is. It
reports; the guardrail decides. It does not judge code and uses no model.

- **Integrity** (blocking): the Atlas rules are in the system text, there is a task, a step has its result protocol, and the engine
  did not change required text (the engine's promise is checked, not trusted).
- **Consistency** (error): two sources assert different choices in a small set of known exclusive groups (database, front-end
  framework, package manager, test runner) as instructions. Narrow on purpose: same source twice, comparisons, migrations,
  prohibitions, code blocks and lines naming two choices are not conflicts. Anything that cannot be shown is not reported.
- **Freshness** (warning): Harness items the project changed under (`ContextRecord::stale_items`, and the "may be outdated" block
  in the text). A signal, not a verdict.
- **Budget** (error / warning / info): required parts that do not fit; items left out; repeated lines removed (information only).
- **Content** (warning): secrets in context that is about to be sent, and untrusted text that claims authority ("ignore previous",
  "administrator access", "the user approved", `sudo`…, English and Portuguese). The latter changes nothing: it is shown, because the
  defence is that no text is ever a permission.
- **Provenance and trust**: every item has a `SectionKind` and a `SourceTrust` (`Atlas`, `Configured`, `Untrusted`).

Health: `Healthy` (information only) < `Partial` (warnings) < `NeedsReview` (an error) < `Invalid` (a blocking issue). Duplication
alone is `Healthy`. The result, with each issue's two sides, is kept with the execution (`OptimizationMetrics.contextReview`).

## Where it runs

`ExecutionService::run_step`, after the prompt is built (and after the Context Engine, if on) and before the runtime starts:
review → redact → decide → record → build again from the inputs if anything was redacted → start, ask or stop. A denied or paused
execution never starts its runtime. The worktree it was given is kept, as for any failed execution. `Apply` is not touched and
stays a human decision; nothing here commits, pushes or merges.

Flag: `optimization.guardrails.enabled`, **on by default** (a safety check; it changes a prompt only to remove what looked like a
secret). Off, none of this runs.

## Measured

- With the real stack and a stand-in CLI: a step whose own instructions say MongoDB while its skill says PostgreSQL pauses before
  any process starts, asks; a yes runs it to completion without asking again; a no fails the step with the reason and nothing after
  it runs.
- With the **real Claude CLI and a model that does not exist** (it starts, reports, fails at its first request, spends nothing): the
  same pause, no process before the answer, and after the yes the real CLI is started (and fails for its model, not for Atlas).

## Hardening (phase 5.1)

Three gaps found after phase 5 and closed without adding a feature.

### An approval is for one evaluation

An `ASK` is a `PendingInteraction`; it now carries `evaluation`, the fingerprint of the evaluation it was asked about (the same field
on `InteractionDetection`, so nothing new exists beside the interaction). The fingerprint is a digest of what the evaluation looked
at: the stage, each context item's id, source and content, the review's findings, the agent, runtime, model and capabilities, the
agent's **resolved** policies (not the files they came from), the worktree and the edit access granted. It excludes the execution id,
because the step is run again (a new execution) after the answer.

The answer reaches the guardrail as `ReviewAnswer { answered, evaluation }`. `brief::guardrail_answer` returns it only for a question
that was asked in this workflow run, by this step, in this pass, **by the attempt being run again** (an older attempt's question is
not this one's) and that carries an evaluation (a record from before this change approves nothing). The guardrail then drops it unless
`evaluation` equals the fingerprint it has just computed; a dropped answer means the question is asked again, and the event says the
earlier approval no longer applies (`staleApproval`). A declined answer is bound the same way. An invalid context is still denied
whatever was answered.

Tests: the same context on its answer goes (A); a changed task text (B), a changed policy (C) and a changed handoff (D) ask again; an
answer of another agent is not this agent's; a question of step A is nothing for step B (E); of one pass nothing for another (F); of
an earlier attempt, of another run, or without an evaluation nothing (G).

Limit: the fingerprint is a digest of data Atlas assembled, not a signature. It prevents a stale approval; it does not defend against
someone who can already rewrite Atlas's run files.

### The ChangeSet is reviewed before Apply

`security/changeset.rs` reviews what a run would put in the project, every file, deterministically:

| Finding                                                                                                                 | Health        | Answer  |
| ----------------------------------------------------------------------------------------------------------------------- | ------------- | ------- |
| `.git` as any component (also a renamed file's old path), any case or separator                                         | `Invalid`     | `DENY`  |
| absolute path, drive letter, `../` that leaves the project                                                              | `Invalid`     | `DENY`  |
| a file whose real location (following links, also a linked folder above it) is outside                                  | `Invalid`     | `DENY`  |
| `.atlas/` at the project root                                                                                           | `NeedsReview` | `ASK`   |
| `.env`, `.env.*` (not `.example`/`.sample`/`.template`/`.dist`), credentials, private keys, `.ssh/`, `.aws/credentials` | `NeedsReview` | `ASK`   |
| an added line that looks like a secret (the Harness scanner, `redact_secrets`; added lines only)                        | `NeedsReview` | `ASK`   |
| a file that could not be read or a link that does not resolve inside                                                    | `NeedsReview` | `ASK`   |
| nothing                                                                                                                 | `Healthy`     | `ALLOW` |

Reused, not reinvented: `security/paths.rs` (`NormalizedPath`, `is_inside_physical`) for escapes, `harness/secrets.rs` for content.
There was no per-path policy to defer to, so the rule for `.atlas/` is fixed here; none was added. The review names a path and a
code, never content.

It runs in `IntegrationService::apply`, through the blocking mechanism that already exists. `DENY` → the run is `Blocked` with the new
`BlockReason::ProtectedPaths`, Apply is not offered and trying again changes nothing (Keep isolated and Discard still work).
`ASK` → the first Apply is answered with `Blocked`/`NeedsReview` and the findings; the person's next Apply on **the same review
fingerprint** (revisions, files, findings) is their decision. Changed code is a different review and is held again. `ALLOW` → Apply
goes on as before. In every case Apply is a person's act, **never commits**, and the other checks (conflicts, dirty checkout, policy)
are the existing ones: a healthy review does not override a conflict, and a ChangeSet with no files is not applied (the run says
there is nothing to apply). The review is stored on the run (`integration.review`). If the changes cannot be read to be reviewed,
Apply is held (`Undetermined`, may be retried): the review fails closed.

What could not be exercised end to end: Git does not report `.git/…` paths or absolute / `../` paths in a real change set (it refuses
to add them), so those are proven by the review's own tests only. Symlinks (including one to `/etc`), `.env`, `.atlas/`, secret
lines, several files at once, conflicts and the empty change set are proven against real Git worktrees.

The audit of this decision is the run's own record (`integration.review`, the `IntegrationChanged` event); it is not on the
per-execution `AuditLog`, which is keyed by an execution and a `PermissionAction` and has none for it.

### `--restricted` for Claude: investigated, not adopted

Claude CLI 2.1.285. Its help text: `--restricted` removes the built-in tools that run commands or code (Bash, PowerShell, REPL…) and
`WebFetch` unless `--tools` names them, and ignores user, project and local settings files (managed settings and `--settings` still
apply); it also confines file tools to the working directories and protects writes to settings, Git and tool-config files, and
refuses `bypassPermissions` (those three are the CLI's own help text; **not observed here**, see below).

Experiment, no model and no tokens: the real CLI started in an empty directory with `--model not-a-real-model-xyz` and `< /dev/null`,
which prints its `system/init` and fails its first request with usage 0 and cost 0. Each Atlas configuration (read-only tools, edit
tools with `--permission-mode acceptEdits`, no tools) with and without `--restricted`, plus the flag alone and with Atlas's isolation
flags and no `--tools`:

| Observed in the CLI's own `init`                                                | Atlas now                                         | Atlas + `--restricted`                                                                                                                  |
| ------------------------------------------------------------------------------- | ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| tools, read-only / edit / none                                                  | 3 / 5 / 0                                         | 3 / 5 / 0 (identical)                                                                                                                   |
| MCP servers, slash commands, skills                                             | 0, 0, 0                                           | 0, 0, 0                                                                                                                                 |
| permission mode                                                                 | same                                              | same                                                                                                                                    |
| plugins                                                                         | 4: the user's `atlassian`, `figma` and 2 built-in | 2 built-in only                                                                                                                         |
| auto-memory path (the user's `~/.claude/projects/…/memory/`)                    | reported                                          | absent                                                                                                                                  |
| error                                                                           | unrecognized model, exit 1                        | the same                                                                                                                                |
| without `--tools`: `Bash`, `CronCreate`, `Monitor`, `RemoteTrigger`, `WebFetch` | exposed                                           | removed                                                                                                                                 |
| the flag alone, without Atlas's other flags                                     | —                                                 | 32 tools, 6 claude.ai MCP servers, 55 slash commands, 19 skills: it does not replace `--strict-mcp-config` / `--disable-slash-commands` |

Findings: with the explicit `--tools` allow-list Atlas already passes, `--restricted` changes no tool, no MCP server and no slash
command; it is the allow-list, `--strict-mcp-config` and `--disable-slash-commands` that do that work, and `--restricted` does not
replace them. What it does change is outside the tool list: the user's plugins are not loaded and the user's settings and memory are
not read. That is a real isolation gain (a plugin or a settings file of the user's no longer reaches a run Atlas starts) that was
**observed only as the CLI's declared start-up state**.

Not observed, so not claimed: that file tools are confined to the working directory, that writes to Git or settings files are
refused, how `--restricted` behaves in a run that really calls tools, whether OAuth/keychain sign-in works under it (the failing
request was the model check, before authentication was exercised), and what the user's own hooks or `permissions` rules in the
ignored settings files were doing for them (for example a hook they depend on would stop applying). Each of those needs a model call.

**Decision: C, investigate later.** Not adopted. The evidence says it may add isolation from the user's plugins, settings and memory
and nothing else, and the part that would matter most (confinement of the file tools in an edit run, and sign-in) cannot be shown
without spending tokens. The next step is one small real run, with the user's go-ahead, in an edit-enabled step in a throwaway
worktree: ask it to write outside the folder and to touch `.git`, with and without the flag, and check sign-in.

## Limitations

- No autonomy modes exist; none was added.
- No "before each action / write" guardrail for a CLI runtime's own tools (see above). Write confinement for those is what Atlas
  grants (tool list, isolated worktree) plus the CLI's own rules; `--restricted` was investigated in phase 5.1 and not adopted.
- The ChangeSet review is path- and pattern-based: a secret that does not look like one, or a harmful change in an ordinary file,
  is not detected. It stops Git's files, escapes and the obvious sensitive files; it does not judge whether the change is good.
- An `ASK` exists only in workflow steps. In a conversation a context that needs review is reported and the run goes on.
- The conflict detector knows four groups; most real contradictions are not detectable by rule and are not reported.
- Authority-claim and secret detection are pattern lists: they will miss paraphrases. They are signals; safety does not rest on them.
- An approval is bound to the evaluation it was asked about (phase 5.1), and to one attempt of one pass of one step.
- Metrics and review are kept per execution; there is no dashboard (phase 7).

## Decisions left

1. Whether to introduce autonomy (Manual / Guided / Auto) and what each means for `ASK`.
2. `--restricted` for Claude: decision C, a small real run is needed (see above).
3. Extending the conflict lexicon from real projects.
