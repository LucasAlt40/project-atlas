# ADR 0030 — Workflow templates: purpose, tools and saved copies

Status: accepted. Builds on [ADR 0013](0013-workflow-engine-and-orchestrator.md), [ADR 0015](0015-result-contracts-outcomes-and-worktree-lock.md) and
[ADR 0028](0028-mcp-foundation.md) / [0029](0029-mcp-for-every-runtime.md); changes none of their invariants.

## Context (audited in the code, not assumed)

- A template was eight pieces of code that each built a graph with one- or two-sentence instructions. No goal, no "done when", no tools, no outputs,
  nothing to read before starting, and no template of the person's own: a customised workflow could not be kept as a starting point.
- **No Atlas agent runs commands**: the editing prompt says "You cannot run shell commands" and Claude is launched with `Read,Grep,Glob,Edit,Write`.
  The old instructions ("run or review the tests") invited an agent to report a test run that never happened.
- MCP was complete in the core (connections, grants by agent / workflow / step, a plan per runtime) and absent from workflows: the node inspector had
  one checkbox, the grants were reachable only from the hub, and the prompt named servers but not their tools.
- A node could not narrow its agent's permissions, so a "read-only" template depended on the agent's profile being read-only.

## Decisions

1. **The three featured templates reuse the existing ids** (`software_feature` → Implementer, `bug_fix` → Debugger, `code_review` → Reviewer), so stored
   workflows and Automatic mode keep working. The other five stay as variations. Their instructions live in `workflow/template_text.rs`.
2. **A step says when it is done** (`AgentNode.completion_criteria`, told to the agent after its instructions with "say what is missing instead of
   reporting it as done") and **may be held to reading** (`ExecutionPolicy.read_only`). Both default to the old behaviour. `read_only` only narrows:
   it makes the execution take the same path as a policy that denies writes (no edit tools reach the runtime, the prompt says read-only) and lets a
   step share the worktree with others. It does **not** limit MCP tools: those are what grants are for (the UI says so).
3. **Honest instructions.** Every step that could be tempted says it cannot run commands, asks for the commands someone should run, and to separate
   what was read from what was executed. Tools are mentioned only conditionally ("if an integration was given to you") and a missing one is to be
   reported, not worked around. Proportionality is in the text (the Architect opens with `Size: trivial|moderate|complex`, later steps scale to it) and
   in Automatic mode, which proposes the quick `feature_basic` for a short task that calls itself small.
4. **The Debugger is one fixer step with two phases** (evidence and root cause with a stated confidence, then the smallest fix with a regression test),
   a read-only verification that writes the final report, and a way out: a fixer that declares `blocked` ends the run at an **Inconclusive** end node
   (`EndOutcome::Failed`) instead of guessing. The route exists only when the fixer's contract declares `fixed`/`blocked` (otherwise the step goes on to
   the verification), because a node nothing reaches is an invalid workflow. A separate "investigate" step was rejected: the agent's contract is its
   own, and an investigation cannot honestly end with `fixed`.
5. **The Reviewer** is one read-only step; the template cannot make a file change whatever the agent's profile. A follow-up fix is a separate run the
   person starts.
6. **Tools are suggestions until the person applies them.** A template names a _kind_ of tool per step (`ToolCapability`: browser, issues, docs, repo),
   matched against the workspace's connections by name (a guess, so it only ever suggests). `template_tools::resolve` says, per suggestion, which
   connection would serve it with which tools, or why none can (no connection, switched off, missing secret, could not connect, tools never listed, no
   reading tools). `create_workflow_from_template(applyTools: true)` turns the usable ones into ordinary grants for that agent, workflow and step.
   A step that only _consults_ gets only the connection's tools whose first word is a reading verb, and only when the server was discovered: without
   a discovery the writing tools cannot be held back, so nothing is granted and the preview says why. A browser step gets the whole server (its tools act
   on a page). Nothing is granted without the person's option, and only connections they already switched on.
7. **The agent is told the truth about its tools.** The integration note lists the tools a grant covers (`jira (tools: getJiraIssue)`) and names what
   was granted and could not be given (`context7 (its credential is missing)`), with a request to say so in the result instead of guessing.
8. **Saved templates are copies.** `CustomTemplate` (config field `workflow_templates`, default empty) keeps a workflow's steps, routes and the tools
   granted to its steps (by connection name and tool selection). Built-in templates are code: they cannot be edited or deleted
   (`workflow_template_builtin`). Deleting a workflow now also removes the grants made to its steps.
9. **Three layers, said in the UI:** global (agents, MCP connections), template (steps, instructions, suggested tools), run (the task, and edits to
   the workflow). The preview lists steps (who, read-only, bounded loop, done-when, instructions), tools with their state in this workspace and the
   outputs before anything is created. The inspector edits the criteria and the read-only flag and manages the step's grants through the existing grant commands.

## Not done / limits

- The suggestion match is by connection name; a connection named otherwise is granted by hand in the inspector.
- `read_only` does not govern MCP tools; the reading-verb filter is a guess from names (Atlas does not know tool descriptions).
- Named-tool grants are refused where the runtime cannot hold tools back (OpenCode today): the server is then not given and the agent is told.
- No real model was called: behaviour is covered by the real services over fake runtimes, and by the stand-in CLI stack. How well each instruction
  steers a real model was not measured.
- `LIVE_NARRATION` still says to quote paths "in shell commands", which no agent can run. It is product text and was left alone.
