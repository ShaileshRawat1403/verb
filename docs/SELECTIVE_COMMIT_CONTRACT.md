# Selected-file commit contract

Implemented locally in the conversation-flow checkout. Not deployed or enabled on the phone boundary.

## Person's workflow

Open Commit, inspect the fresh file list and choose files. Select all and Clear selection are keyboard
and touch controls. Each file has a View diff action and the dialog announces the selected count.
With no selection, Commit is disabled. The optional Spec audit checkbox explicitly includes the Spec
file in addition to the chosen files. The selected Spec is frozen when the dialog opens.

The commit records a local snapshot; it does not upload, merge, release or deploy anything. Unselected
working files are left untouched. A file edited after review requires reopening the review. Failed or
interrupted submissions are never replayed automatically; inspect history before retrying an uncertain
result. The dialog keeps its message and asks for a fresh review after an error.

## Server guarantees and limits

- GET `/api/git/commit-review` returns a version bound to the branch, HEAD, changed paths, content
  hashes and executable modes, plus per-file unsupported reasons and any global blocking condition.
- POST `/api/git/commit` requires `selection: {version, paths}`. Missing selection, duplicates, unknown
  paths, empty selections and stale versions fail closed. There is no fallback to `git add --all`.
- The transaction owns Git's real index lock, copies the existing index into a separately reserved
  private index, and stages only selected paths there. Pre-existing staged content or intent-to-add
  entries are refused. Unselected index flags are preserved, rather than rebuilding the entire index.
- Staged blobs and modes must equal the reviewed bytes and modes, not merely the working files at
  some later check. Concurrent file edits therefore cannot replace the reviewed commit content;
  newer edits can remain in the working tree. Git pathspec magic is disabled and paths follow `--`.
- The tree is committed against the reviewed parent. Updating that specific branch uses Git's expected
  old-OID compare-and-swap, preventing a concurrent commit from being overwritten. Only afterward is
  the prepared index installed. If that last step fails, the error identifies the already-created
  commit and asks for terminal recovery; it does not claim nothing happened.
- Spec audit text is included in the committed tree. On a pre-commit failure, Verb restores only an
  unchanged copy of its own attempted audit edit; it does not overwrite a newer external edit.
- Guided commits use Git plumbing. Configured commit signing, custom hooks paths, active commit or
  reference-transaction hooks, conflicts and in-progress Git operations require the terminal so their
  policy is not silently skipped. There is no automatic hook or signing bypass.
- Symbolic links, submodules, non-regular files and files above 8 MB are individually unsupported and
  unselected. Non-UTF-8 names and more than 200 changed files require the terminal. Renames appear as
  independent old-path deletion and new-path addition; select both to include the complete move.
- Commands have bounded execution/output. No networking is needed. Remote execution boundaries are
  deliberately unsupported until a reviewed boundary implementation exists; no Node policy is changed.

Opening a review reads changed-file content as part of normal Git review. It is not an agent-content
stream or a background activity recorder. Git's own ignored-file policy determines the changed list.

## Validation

Rust coverage includes actual selected commits, literal/Unicode paths, unselected experiments,
stale content and invalid paths, byte-identical pre-existing staging, deletion, Spec inclusion and
failure cleanup, first commits, intent-to-add, lock ownership and unselected index flags. Browser
acceptance covers desktop/phone selection, actual Git history, diff inspection, stale-review refusal,
the selected Spec's audit and unavailable-review recovery. Test run results belong in the local handoff.

## Remaining production work

Boundary parity is required for guided selective commits on the phone server. Until then, it uses the
terminal. This slice does not add hunk staging or conflict resolution. The native ref/index interruption gap
now has explicit receipt-based repair, described in GIT_RECOVERY_CONTRACT.md; unknown early locks,
interrupted repairs and missing evidence still need terminal inspection. Keep uncertain results
visible and inspect history/status before further writes. Real-provider conversation acceptance and measured
phone resource/reconnect acceptance also remain separate release requirements.


Validation for this local slice: 438 Rust all-target tests passed, five ignored; all 83 web unit tests
passed; all 39 affected Studio/workbench browser acceptance cases passed. Formatting, strict clippy,
web/native builds and diff checks passed. The phone dialog screenshot was inspected; controls are
reachable at 390 px with no document overflow and a 44 px Commit target. Logs: `/private/tmp/verb-select-all-rust.log`,
`/private/tmp/verb-select-unit.log`, `/private/tmp/verb-select-acceptance.log`, `/private/tmp/verb-select-clippy.log`.
These are disposable native-local tests, not public-service or external phone-boundary acceptance.
