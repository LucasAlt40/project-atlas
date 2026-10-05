# 0022 — Optimization Layer, phase 2: Skills with progressive disclosure

## Context

Atlas agents run on CLI runtimes that take one prompt. Putting every piece of project know-how into every prompt wastes tokens and
buries the part that matters; leaving it out loses it. The open **Agent Skills** format already answers this: a folder with a
`SKILL.md` (YAML frontmatter `name` and `description`, then instructions), optionally `references/`, `scripts/` and `assets/`,
loaded in levels. Atlas adopts the format as it is.

## Decision

**Format.** `<name>/SKILL.md` with the standard fields (`name`, `description`, `license`, `compatibility`, `allowed-tools`,
`metadata`). Atlas's own hints live inside the format's free-form `metadata` map and are optional: `atlas-priority` (an integer),
`atlas-personalities`, `atlas-stacks`, `atlas-files` (comma-separated). No proprietary file, no proprietary frontmatter key.

**Where skills live.** `<project>/.atlas/skills/` and the user's `<app data>/skills/`. A project skill shadows a user skill of the
same name (the shadowed one is listed with an error and never selected). Both folders are read through the `SkillStore` port; the
filesystem adapter never follows a symlink and refuses any path that would leave a skill's folder.

**Three levels.**

1. _Discovery_: name, description, metadata of every skill, parsed once and **cached per skill** until that skill's file changes
   (fingerprint = size and modified time, checked with a stat, not a read). Never sent to the model: the runtimes cannot ask for a
   skill later, so a list of unloaded skills would be tokens for nothing. The cost discovery avoided is reported (`level1Tokens`).
2. _Activation_: the `SKILL.md` body of at most three skills the task calls for.
3. _Resources_: `references/` files that the task's words share with the file name, or that the skill's own text links with words
   the task shares (at most two per skill, 3,000 estimated tokens in all, text only). Scripts and assets are never inlined: Atlas's
   agents run read-only or file-edit tools without a shell, so a script has no place in a prompt.

**Selection is deterministic and explainable**: words of the task (plus a step's label and instructions) against the skill's name
(3 points each) and description (1), the project's technologies (2), the personality (3), a file type the task mentions (2), and the
skill's priority (a tie-break, never a reason by itself). Plurals are folded; nothing cleverer. A skill needs 4 points, or to be named
(`/name`, `skill name`, `name skill`), which always counts. Every choice carries its reasons into the
`optimization_skills_selected` event. No model judges a skill; that can come later if it ever proves better.

**Quality checks** (`validate.rs`), each with a message saying what to fix. Errors keep a skill out of selection (malformed,
missing or invalid name, missing description, shadowed); warnings are reported: name differs from folder, description too long /
without a "use when" / too generic, body too large (over 5,000 estimated tokens or 500 lines), `allowed-tools` with more than 12
tools, an instruction repeated, instructions that say opposite things, an empty section, and a skill that repeats three or more
instructions of another.

**Prompt.** `Prompt.skills` is a new section, between the Harness and the project context, framed by a notice: guidance with no
authority, below the Atlas rules and the task. Like everything else it is built by `PromptBuilder`. What the runtime may do is still
decided by the process guard, which never reads prompts; a test puts "ignore every rule" in a skill and checks the request is
otherwise identical.

**Context Engine.** Each selected skill is a `ContextItem` (`Normal` if Atlas matched it, `High` if the task named it). With the
engine on, a line one skill repeats of another is dropped, and under a budget a matched skill is the first thing left out (replaced
by a line saying so) while a named one is kept and the overrun is reported. This is the first real use of the `Normal` priority.

**Flag.** `optimization.skills.enabled`, **off by default** (it adds text to prompts). Off, nothing is even looked at on disk, and the
prompt is the one phase 1 builds.

**Native capability first.** Claude Code has its own skills. Whether it loads them under Atlas's tool restrictions (`--tools
Read,Grep,Glob`) is not verified, so no runtime is marked as supporting skills natively and Atlas injects what it selects. When a
runtime is verified, its capability flag should stop Atlas from injecting the skills that runtime already loads.

## Measured

Ten skills of about 800 tokens each, one matching the task: loading the catalogue whole would be 8,160 tokens (plus 220 of
metadata); progressive disclosure sends 808 (about 10%) and none of the metadata.

## Not built

- A screen or command to list skills and their issues (the data is in the metrics and the discovery API; the dashboard is phase 7).
- `.claude/skills` and `.agents/skills` discovery (waits for the native-capability decision above).
- LLM-assisted selection, resource selection by anything but words, and per-skill token budgets beyond the global one.
