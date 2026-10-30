# Verb console and preview workspace

Local implementation, 27 October 2026. Public deployment is not authorized.

The supplied reference has three steady anchors: project navigation on the left,
a work conversation in the middle, and tabbed output beside it. Verb adopts that
arrangement while continuing to use its existing Spec/session/Git records.

The centre shows the selected work, its independent execution/verification/delivery
facts, real audit activity, and the existing explicit change composer. Terminal,
files, board and Git actions stay directly available. It is a work thread backed by
real records; full freeform agent chat is not represented as implemented.

The right pane supports:

- Up to eight actual file tabs, arrow/Home/End navigation, and guarded closing.
- Escaped document/source previews, explicit edits and recoverable drafts.
- Saved single-file HTML app previews in the existing opaque-origin sandbox.
- PNG/JPEG/GIF/WebP viewing with fit, natural-size zoom and download.
- Actual changed-file lists and textual Git diffs, without committing anything.
- Pointer/keyboard resizing, expansion, and Escape to restore the split.
- Explicit Canvas/Work switching on phones instead of a compressed desktop layout.

The host never opens external-project images directly. The existing execution
client runs a trusted embedded helper as the registered project user. Its source
is a fixed-size stdin prefix, followed by bounded JSON. Image signatures, sizes,
paths and hashes are verified; ignored files, links and active image formats are
refused. Saving and displaying a preview are separate operations.

REA 6.8.0 was used for static analysis of copies of Verb's two existing Studio
modules. It parsed both files without executing them (3,351 AST nodes; no parse
failures). Evidence: ev_3c5ca5110fd3abcdb83f529e51e306d9cce2a0ab686149d09d1a8962521193bb.
The source snapshot hash is 8b7abe747092b8a20b1b9b2bbf1b01472088d3e78573468cdb864945d6a6b18e.
This supplied an inventory for splitting preview presentation from editing state;
it is static evidence, not runtime compatibility proof. The reference screenshots
supply the observable layout behavior. REA is a research tool, not a Verb runtime
dependency. No proprietary application source was needed for this implementation.

Public remains a separate release. Do not restart or deploy while iterating here.
PocketFabric Node source, services and execution policy are outside this work.
