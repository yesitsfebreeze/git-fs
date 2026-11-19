# Stacked pull requests on one session branch

An architecture decision record may plan a change as a stack of pull requests:
layer 1 is reviewed and merged before layer 2 is reviewed, and each layer has
its own branch. Git-FS does not have a branch per pull request. It has one
linear session branch and two guarded publication tools. This note states how a
stack maps onto them and where the mapping stops.

## The constraint

- One MCP session writes one session branch, `refs/gitfs/<session>`. Its commits
  are linear and private to that session.
- `integrate` previews the session overlay against the checked-out trunk and
  commits it there. The reviewed commit is the parent of the next layer's
  integration.
- A push may publish only a commit that this session integrated. A different
  session cannot finish an ordinary push that this session did not authorize
  with `phase: "handoff"`.

So the stack is not stored as branches. It is stored as an ordered series of
trunk commits, one per layer, all produced by the same session, and the pull
request branches are published afterwards from those commits.

## The order

Work the whole stack in one session, layer by layer. Do not start layer N+1
until layer N is integrated: the next integration would otherwise put both
layers into one commit.

For each layer N:

1. Make the layer's edits with the normal tools and read the diff back with
   `gitfs_diff`.
2. Preview the integration:
   `integrate {phase: "preview", message: "PR N: <what changes>"}`.
3. Commit exactly that preview:
   `integrate {phase: "commit", expected_revision: <preview.revision>,
   message: "PR N: <what changes>"}`.
   The result names the integrated commit, `cN`.
4. Preview the branch for that layer:
   `push {phase: "preview", remote: "origin", ref: "refs/heads/pr-N",
   commit: "cN"}`. `expected_remote_head` may be supplied; the preview returns
   the binding and its own revision.
5. Push it:
   `push {phase: "commit", remote: "origin", ref: "refs/heads/pr-N",
   commit: "cN", expected_revision: <preview.revision>,
   operation_id: "stack-pr-N"}`.
   Keep the same `operation_id` for a retry or `push {phase: "reconcile",
   operation_id: "stack-pr-N"}`.

Layer N+1's integration now has `cN` as its parent, so `pr-(N+1)` descends from
`pr-N`. Open each pull request against the branch below it, or retarget it after
the lower pull request merges. Retargeting is a hosting-provider operation;
Git-FS neither performs nor records it.

## Why the order matters

- A session branch is linear. Editing layer 2 before integrating layer 1 puts
  both layers in the first integrated commit, and no later split can recover the
  boundary without rewriting history.
- The trunk commit is the review unit. Each `integrate` call is one reviewed
  commit; a stack of three pull requests therefore needs three `integrate`
  calls, each on top of the previous one.
- A push is bound to one commit. Publishing `pr-N` does not publish `pr-(N+1)`;
  both calls are explicit.

## Failure modes

- **The session ends before the stack is published.** A fresh session may not
  push the earlier integrated commits under the ordinary guard. Either keep the
  session that integrated them, or materialize the worktree and publish the
  trunk commits with native Git; record which path was used. A restarted
  session cannot inherit the earlier session's identity by naming it again.
- **A layer's preview goes stale.** Trunk, the session overlay or the review
  policy moved. Preview again and commit only the new `expected_revision`;
  never reuse an old one.
- **A push outcome is unknown.** Do not create a new `operation_id`. Use
  `push {phase: "reconcile", operation_id: ...}` until the exact remote commit
  is confirmed or reported unresolved.
- **The lower pull request merges while the stack is open.** The remote branch
  and the trunk commit are different facts. Retarget or rebase the upper branch
  with the hosting provider, then push the exact integrated commit again only
  when its content changed; Git-FS does not rewrite integrated history.

## What this does not provide

Git-FS does not create one session or one branch per layer, does not open or
retarget pull requests, and does not prove that a hosted pull request merged.
Those remain the client's or the hosting provider's operations. The stack works
when the ordering above is obeyed; it is not a substitute for a stacked-branch
feature.
