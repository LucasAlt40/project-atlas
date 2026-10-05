# 0023 — RTK (Rust Token Killer): evaluated, built, and removed

**Status:** removed. Kept as a record: what was found, so nobody has to find it again.

## Context

[RTK](https://github.com/rtk-ai/rtk) filters the output of development commands (`git`, tests, builds, `ls`, `grep`, `tsc`, `cargo`,
`npm`, `docker`…) before it reaches a model. The idea was for Atlas to suggest it, send the user to RTK's own documentation to install
it, and use it once installed. A first version was built and tested (detection through the guard, an install guide in Settings,
a process decorator below the guard that ran approved agent commands through `rtk`), including a test against the real RTK 0.51.
It was then removed, because it did nothing for Atlas.

## Why it was removed

**RTK filters a command's output, and no Atlas agent runs commands through Atlas.** The agents' commands would run inside each
runtime's own tool loop, and Atlas launches them with no shell (`claude -p --tools Read,Grep,Glob`, OpenCode's read-only `plan`
agent, headless Gemini and Antigravity that deny shell). The security layer has a context for commands an agent asks _Atlas_ to
run (`ProcessContext::AgentRequested`), but no feature issues such a command. The `git` Atlas itself runs (worktrees, changes) is
read by Atlas and never goes to a model, so it must not be filtered. Result: installing RTK saved nothing on Atlas's agents, and a
Settings card suggesting it would have been misleading even with a disclaimer. The ordinary reach of RTK (the user's own Claude
Code sessions) has nothing to do with Atlas.

## What was found (still true, useful if this comes back)

- `rtk rewrite` documents exit 0 for "rewritten" and 1 for "no equivalent"; RTK 0.51 exits **3** when it rewrites. Read the answer
  from the output (a single line starting `rtk `), not from the exit code.
- The rewrite is printed as plain text with **the quoting lost**: `rtk rewrite grep "foo bar" .` prints `rtk grep foo bar .`. It
  cannot be split back into arguments. Build the argument list from the original command: if RTK only put `rtk` in front, use the
  original arguments; if it mapped to another subcommand (`cat f` → `rtk read f`), split only when no argument has a space or shell
  syntax and every original argument is still at the end; otherwise run the command as asked.
- `cargo install rtk` from crates.io installs a different tool (Rust Type Kit). The check that it is the right one is
  `rtk gain` (`rtk gain --help` is a safe, read-only form).
- RTK writes the raw output of commands to `~/Library/Application Support/rtk/tee/` and keeps its own statistics database: raw
  output (possibly with secrets) lives outside the project, out of the guard's reach.
- RTK has optional telemetry, opt-in. Atlas-started `rtk` processes can set `RTK_TELEMETRY_DISABLED=1` without touching the user's choice.
- Not verified: whether a repository can ship an RTK "custom filter" that changes the output an agent sees, and whether RTK preserves
  the exit code of every wrapped command.
- Documented install methods (INSTALL.md): `brew install rtk`, `winget install rtk-ai.rtk`, the `install.sh` script,
  `cargo install --git https://github.com/rtk-ai/rtk`. `rtk init -g` edits the user's agent settings; Atlas never ran it.

## What would bring it back

Agents that run commands **through Atlas** (the `AgentRequested` path). That needs, in this order: an Atlas-hosted tool the CLI
runtime can call (an MCP server over stdio bound to one execution and its scope, not a listening port); an OS sandbox for the
processes it starts (the `SandboxProvider` seam exists, only `NoSandbox` is implemented); a small allow-list to begin with (Git
reads, test/lint/typecheck runners in the isolated worktree, no network, approval for everything else); and a measurement, before
RTK, of how much command output agents would actually produce and whether filtering it changes their results. Only then does
filtering the output earn its added binary and its blind spots.

The design used (a decorator below the guard, for `AgentRequested` commands only, never for Atlas's own) was sound and is recorded
in this ADR's history for reuse.
