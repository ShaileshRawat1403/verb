# Open questions for owner approval

1. Approve the proposed plain-text CLI commands `verb shared spec <spec-id>` and `verb shared full` before implementing slice 3.
2. Confirm proposed work item paths and whether the standard document sections in the PRD are mandatory.
3. Define the signals that put a work item in the needs-owner group and which persisted timestamps determine the most recently changed order.
4. Decide what the publisher should do if required content alone exceeds 8192 bytes after all permitted omissions. This includes oversized required brief fields or any per-spec content that must be retained.
5. Choose the stable criterion reference format inside the legacy Traceability section. SDLC keys evidence by index plus exact criterion text and hashes all criterion text into intent. Slice 2 must preserve both index and text.
6. Verify native instruction-file behavior by supported CLI version before slice 3. The source lists Claude, Codex, Gemini, Antigravity, OpenCode and Dsh, plus shell and custom commands, with separately configured external runtimes. Verb writes both files and supplies a shared-read bootstrap for Claude and Codex. Those facts do not verify which files the CLIs actually read. Record the observed matrix before relying on automatic discovery.
