# Vendor record — codex-peer-review

Third-party skill vendored into the machine's addon kit (`mine/`). Opt-in only;
not registered with Claude Code until slotted in (see `mine/README.md`) or
activated into a project via the `/assemble` opt-in step.

## Provenance
- Source: https://github.com/leegonzales/AISkills (path `CodexPeerReview/codex-peer-review`)
- Upstream commit: `d34b567c996449bfe488663325c91eba4def6c1a`
- License: MIT (see `LICENSE.txt`, retained verbatim with copyright notice)

## What it does
Shells out to OpenAI's **Codex CLI** (`codex exec`) for a cross-model second
opinion on architecture, security, design trade-offs, and alternatives, then
synthesizes both perspectives. Complementary to `/personas` (intra-Claude panel),
not a duplicate: this is a different model family, not a different reviewer lens.

## Prerequisite (heavy, external — why this is opt-in)
- OpenAI Codex CLI: `npm i -g @openai/codex` (or `brew install openai/codex/codex`)
- OpenAI auth: `codex auth login` (ChatGPT Plus/Pro) or `codex auth api-key <key>`

## Local deviations from upstream
- **Emoji stripped** from all `.md` files to satisfy the machine's no-emoji
  standard for active instruction docs (`.claude/rules/coding-standards.md`).
  Content is otherwise verbatim.
- `dist/` binary bundle and top-level repo docs (`README.md`, `TESTING.md`,
  `VALIDATED_FLAGS.md`) were not vendored — only the skill folder.

## Updating
Re-pull the skill folder from upstream at a newer commit, re-run the emoji strip,
bump the commit hash above. The upstream "Codex vs Gemini" section and
`Integration Points` references mention sibling skills (Gemini peer review,
concept-forge, prose-polish, claimify) the machine does not ship; they are
harmless and left intact for fidelity.
