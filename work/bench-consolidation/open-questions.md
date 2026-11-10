# Open questions for owner approval

1. **Work item documents.** Approve the proposed project paths and choose work item ID generation, size metadata syntax, required standard sections and how the small PRD's one-page limit is measured.
2. **Traceability edge cases.** Decide whether a criterion can cite multiple requirements, how to handle retired references, and how to preserve evidence identity when adding the required suffix to an existing criterion.
3. **Board rollup.** Define grouping, ordering, counts, when a work item is complete, and whether items can be archived. Define which spec stages count as in progress for shared context.
4. **Decisions.** Choose the canonical decision file or section and which decisions are included in shared context.
5. **Overflow.** Choose the omission priority under the fixed 8 KB cap and the exact KB-to-byte convention. Define full-context and per-spec CLI command names, output, redaction rules and launch-prompt size handling.
6. **External edits and publish.** Choose the refresh trigger for CLI and Git edits, lock scope, recovery for a partial two-file publish, and malformed or duplicate marker handling.
7. **CLI coverage.** Confirm the supported launch matrix and how shared and spec-specific context reach each agent in isolated and separate-account environments. Confirm what unlinked legacy spec launches receive.
8. **Legacy association.** Approve the unlinked lane, who can attach existing specs, and whether tasks should ever receive explicit associations. Confirm the project-wide legacy task queue in Settings.
9. **Navigation and rollback.** Choose the compatibility switch and retirement timing, Overview counts on Board, and whether Ask, Observer and the palette stay global.
10. **Gate boundaries.** Confirm which existing checks count as warning-only stage gates, including Ship checks. Independent Git and terminal safeguards remain in scope for their own operations.
11. **Handoff wording and build approval.** Approve the exact D1/D2 wording before server-side landing. Phase 2 remains separately gated.
