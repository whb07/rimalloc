# AGENTS.md

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

If tasked to perform a large series of tasks, break the work down into an ordered linear sequence and complete them one by one until the full set is done.

When the user gives an explicit, numbered instruction to complete a specific set of tasks (e.g. “implement the first 50 tasks”), treat that instruction as the primary directive. Break the work into an ordered sequence and execute it task-by-task until complete, even if the total scope is large.

## 5. Mandatory Rust Verification

**Always run formatting, linting, and tests before declaring work complete.**

For Rust code changes, run:
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo test --workspace --all-targets --all-features`

If any command cannot be run or fails for reasons unrelated to your changes, report that explicitly in the final response. Do not silently skip these checks.

## 6. Avoid Audit Theater and Make-Work

**Hard results beat process metrics. Don't confuse motion with progress.**

Audit, compliance, and ratchet-style work must produce a real reduction in risk or complexity, not just larger ledgers. Before declaring any audit or refactoring task done, the code must be measurably better on a metric that matters — not just better documented.

- **Prefer fewer unsafe sites over more evidence rows.** Adding TSV rows, allowlist entries, or test files is not an achievement if the production unsafe count is flat.
- **Caps and limits are guardrails, not goals.** Splitting a module to stay under a line limit only counts if the split isolates an invariant or removes a hotspot. Fragmentation for its own sake is still churn.
- **Verification is a hard gate, not a footnote.** `cargo fmt`, `cargo clippy`, and `cargo test` must pass before claiming progress. A long commit history with no verification commits is a red flag, not a flex.
- **Commit the actual state.** A dirty working tree full of "TODO backlog" updates at the end of a run means the recorded state is stale. Land or revert it.
- **Ask whether a senior engineer would call it make-work.** If the main effect of a change is more files, more entries, or more metadata rather than a smaller trust surface, stop and do the real reduction first.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, clarifying questions come before implementation rather than after mistakes, and audit work shows up as reduced risk rather than expanded paperwork.
