# Studio code editor

29 October 2026. Release candidate for the browser workstation. Android remains a
separate app; this change adds no PocketFabric runtime, policy or service dependency.

## User-facing behavior

Edit file opens Code mode by default. It provides line numbers, syntax highlighting,
bracket matching/closing, folding, per-file undo/redo, and local completion for the
bundled language packages. HTML, CSS, JavaScript/JSX, TypeScript/TSX, JSON and Python
have syntax support; other UTF-8 files remain editable as plain text. JSON additionally
shows local parse diagnostics. These are syntax/completion helpers, not a connected
project language server, compiler, formatter, debugger or AI agent.

Code and Plain text share the existing in-tab draft and explicit Save file operation.
Find/replace, replacement undo, go to line, copy, disk reload and recoverable draft
restore work with the selected mode. Code mode remembers cursor/history per retained
file. Disk reload resets code undo; the previous unsaved draft is still explicitly
recoverable. Clean draft eviction also evicts its editor state. History is transient,
not a persisted backup or evidence of successful execution.

Change file opens the existing Files control and preserves drafts. File-picker chrome
is hidden during editing. Phone editing scrolls to the editor controls, uses16px text,
wraps lines by default and keeps44px control targets. Wrap lines is explicit and
retains the current preference while switching files. Tab moves focus out of the
editor; Ctrl+Space opens completion. Save/find/go-to-line retain existing shortcuts.
If editor initialization fails, the plain-text fallback stays available.

## Save and execution authority

The existing workspace ID and content-revision checks, visible-file rules,200KiB limit,
unknown-save handling, stream/privacy policy and monitored-boundary transport remain.
Typing, highlighting, completion, folding and JSON diagnostics execute no project code,
start no provider or language server, and request no additional file content. Saved
single-HTML previews still require explicit authorization and their original sandbox.
Saving is distinct from committing, uploading, reviewing or deploying.

CodeMirror6 is used as a browser editor component within Studio. It does not replace
the complete-IDE workspace/authority work or establish code-server/Theia execution
access. The existing local VS Code-compatible adapter remains separately scoped.

## Dependencies and distribution

Direct package versions are exact in package.json; all resolutions/integrities are
locked. Only official npm packages were installed. Existing package versions were
unchanged. The24 added dependency packages declare MIT licenses; their license texts
and copyright notices are included in studio-code-notices.js and retained in the
bundled application. The editor uses no CDN, remote telemetry or new CSP exception.

The bundle grew about0.62MB uncompressed to about1.35MB. It includes the six language
families and license notices. This is transfer-size evidence, not a sustained-memory
or physical-phone performance claim. The build transforms template literals so that
completion snippet whitespace is preserved without whitespace-only source lines in
the generated bundle; editing upstream dependency sources is unnecessary.

## Acceptance

New real-Rust-host browser cases cover explicit save→sandboxed preview, separate file
histories, shared Code/Plain drafts, find/replacement undo, Unicode go-to-line, disk
recovery, JSON diagnostics, outside-writer conflicts, phone layout/focus escape,
typing during an in-flight save and actual HTML completion. The older plain-text
journeys remain as fallback coverage; Files explicitly reopens the picker during edit.
The diff-preview fixture now creates its own pending change instead of depending on
an earlier test. Physical-device IME/screen-reader and sustained-use evidence remain
separate acceptance work. The complete workstation is not claimed finished.

Rollout requires clean-source CI/artifact identity, existing idle/backup/rollback guards,
then public owner-browser acceptance. No Node-owned file, service or configuration
change is part of this feature. Detailed deployment receipts stay private.

Primary references: https://codemirror.net/examples/basic/ and
https://codemirror.net/docs/guide/ . Installed package source/type declarations were
also checked. Package notices describe the exact selected dependencies.
