# Browser architecture research — 28 October 2026

## Decision supported by this investigation

Verb should have a browser runtime with its own session and permissions, presented beside the
work thread. The earlier generic iframe proposal is inadequate for the requested Codex/Claude-like
capabilities. This report supersedes it as the recommended direction. A separate local-only first implementation
now exists; see BROWSER_RUNTIME_CONTRACT.md. Only local preview 4873 restart and a local commit were subsequently approved; no public deployment is authorized.

We can use Codex's available browser tools to test Verb. They are tools provided by that application,
not a browser component that Verb can redistribute or depend on as its own runtime. Implement Verb's
integration against supported open-source browser APIs; do not copy proprietary implementation code.

## Evidence and limits

Installed app: `/Applications/ChatGPT.app`, bundle ID `com.openai.codex`, reported version
`26.1021.31345`, package name `openai-codex-electron`. No Claude Desktop app was found in the standard
system/user Applications directories. A Claude Code URL Handler is not Claude Desktop. Claude findings
below are documentation evidence, not local implementation evidence.

REA 6.8.0 was used without executing the target application, instrumenting it, installing integrations,
or opening private conversations, browser profiles, cookies or credentials. Temporary extracts are
under `/private/tmp/verb-browser-static-codex-20261028`; no proprietary source was placed in this repo.

1. REA `inspect-desktop-application` observed an Electron candidate in the app bundle. Its directory
   inventory was limited to 3000 entries, so this is partial installation coverage, not an exhaustive
   runtime audit. Evidence: `/private/tmp/verb-codex-browser-installation-20261028.json`.
2. Direct ASAR analysis stopped on a declared integrity mismatch in an unrelated unpacked native
   `permissions.node` component. The mismatch is retained, not suppressed. Static analysis of copied
   JavaScript is not a verification of that native component or the whole installation.
3. A selected-source REA run completed with partial coverage: two JavaScript modules were parsed;
   the approximately 5 MB main bundle exhausted the explicitly bounded 768 MiB worker heap during
   analysis and remains unexamined by REA's semantic graph. Semantic retention is bounded too.
   Evidence: `/private/tmp/verb-codex-browser-subset-analysis-fixed-cache-20261028.json`.
   The report's zero Electron constructor/IPC findings do not establish absence in the skipped bundle.
4. Separate bounded, read-only inspection of hashed shipped text found Electron guest-webview attach
   handlers, persistent browser route/session identifiers, partition selection, browser-sidebar
   ownership synchronization, browser-specific preload, permission handlers and debugger/capture APIs.
   These are static observations; they do not prove runtime reachability or all security properties.

Selected extracted file SHA-256 identities:

| File | SHA-256 |
| --- | --- |
| `main-0FVbxCQU.js` | `fad45aa7d81de6fe166704e8879e3b7cbfc85151e72df7946853336c7cbc5a3c` |
| `browser-background-switches-IQUsnzYP.js` | `e5b858ceb674070e0033b76ecec0d27a3379a770cc82da5bf579e35243b2c929` |
| `hidden-browser-use-webview-host-a49029186764.js` | `1c06e942858fe60b71a4af3451174a768d169df9ad391e81af046e3d94f453c2` |

Useful static locations in the decoded main bundle (character offsets, not byte offsets):
- 7156: persistent browser-app route and thread-browser-tabs identifiers.
- 2468000–2511500: guest attachment/session handling region; browser-specific lifecycle markers appear
  alongside other guest surfaces. Do not assume every handler in the region belongs to the browser.
- 2972380: browser-page preload; adjacent browser-sidebar manager and ownership synchronization.
- 2609486: WebContentsView usage; this isolated occurrence alone does not identify the main browser.

## Public documentation corroboration

- [OpenAI Browser documentation](https://learn.chatgpt.com/docs/browser?surface=app) describes browser
  use from ChatGPT Work/Codex, site permissions, previewing a running development app, and visual
  comments alongside a diff. It does not expose an embeddable SDK for the desktop app's browser.
- [OpenAI computer/browser walkthrough](https://developers.openai.com/training/walkthroughs/using-your-computer-and-browser)
  distinguishes the built-in browser from the user's Chrome integration.
- [Claude Cowork built-in browser](https://support.claude.com/en/articles/16607400-use-the-built-in-browser-in-claude-cowork)
  describes its own browser in the desktop side panel, separate logins, manual sign-in/import choice,
  desktop availability for web/mobile use, site permissions and action checks. It does not establish
  Claude's internal implementation language or a reusable browser component.
- [Electron WebContentsView](https://www.electronjs.org/docs/latest/api/web-contents-view) is a native
  main-process surface for displaying web contents. [Electron security guidance](https://www.electronjs.org/docs/latest/tutorial/security)
  requires deliberate isolation and permission handling for remote content.
- [Playwright browser contexts](https://playwright.dev/docs/browser-contexts) provide separate cookies
  and storage. A context is a profile/session boundary, not by itself an OS sandbox or a network policy.
- [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/) provides browser control
  primitives; an endpoint is privileged and must not be exposed directly to page content or public clients.

## Implications for Verb's product

One workspace should keep the conversation, browser and review connected:
- Tabs with actual page title, current URL, history, loading/error state, stop/reload and viewport size.
- A clear location label: browser on this Mac, on a separate host, or unavailable. Localhost resolves
  on the browser runtime's machine; the UI must not confuse that with the device viewing Verb.
- A visible human/agent control handoff, with stop/take-control always reachable.
- Explicit site access, managed downloads and uploads, and a separate confirmation for consequential
  actions. Browser permission never approves instructions embedded in page content.
- Console/network errors and screenshots can become review evidence attached to a Spec, with URL,
  timestamp, viewport, runtime/session identity and source revision when known. A screenshot alone
  does not prove which commit was running.
- Browser screenshots and page text are opt-in content. They must follow Verb's privacy/revocation
  model. Normal browsing is not an always-on record of the user's activity.
- File, image, diff and browser tabs share consistent controls. A phone shows one working surface
  at a time and retains drafts while moving between them.

## Architecture candidates for a feasibility spike

| Candidate | Appropriate surface | Main unresolved work |
| --- | --- | --- |
| Native guest browser in an optional desktop shell | Mac/desktop browsing alongside Verb | Desktop packaging, lifecycle, permissions, IPC, updates and isolation |
| Dedicated Chromium runtime with a controlled API and a rendered remote surface | Existing web/phone UI | Auth/control lease, streaming/input, clipboard/files, lifecycle, network policy, resource use |
| Extension connected to the person's existing browser | Work on sites already signed in | Extension packaging, per-tab consent, pairing, compatibility; not a standalone built-in browser |
| Generic iframe in the existing web shell | Limited embeds only | Cannot supply reliable cross-site navigation, inspection, sign-in or agent control |

For Verb's existing web-first product, test a dedicated runtime behind an adapter before deciding
whether a desktop shell is necessary. Do not make Electron mandatory just because Codex ships it.
Keep browser control separate from PTY control and the existing boundary: no new Node-owned runtime,
registry, launcher or phone-server policy may be modified as a side effect. Start with disposable
fixtures and a bounded resource measurement on the Mac; phone-hosted Chromium feasibility is unknown.
The user subsequently requested implementation. The optional native-local worker now implements the
first spike behind owner access and explicit configuration, without changing CSP. Its separate runtime,
site-access, lifecycle and outstanding limitations are documented in BROWSER_RUNTIME_CONTRACT.md.

Acceptance for that spike: real redirects and history; an app that refuses iframe embedding; isolated
profiles; reconnect without lost ownership; denied stale commands; reliable teardown; no Verb-token
exposure; browser-process crash recovery; a phone-sized remote surface; measured latency/RAM/CPU.
Only after that should browser tools be available to agents through an explicit per-session contract.

## State of the repo

Research and the first optional browser runtime are local and uncommitted alongside the conversation
and preview work. The runtime is not enabled on the existing test or public service.
No target app modification, target runtime instrumentation, existing service restart, deployment,
Android edit or PocketFabric Node change. Private extracted app code and REA reports remain in /private/tmp.

## Final worker REA follow-up

REA 6.8.0 analyzed the final worker as inert JavaScript in a temporary copy, including the authenticated
egress proxy, request gates, control generation and lifecycle. One file parsed; zero parse failures;
2,129 AST nodes; 15 findings. The summary view is complete within its projection, but application
coverage is partial and semantic coverage unknown (600 explicit unknowns). Do not interpret absence
of a finding as proof a boundary is safe. The actual Chromium redirect test exposed the route-hook
gap; the proxy was implemented and the test rerun.

Evidence: `/private/tmp/verb-browser-worker-rea-final-20261028.json`; summary:
`/private/tmp/verb-browser-worker-rea-final-summary-20261028.json`. Parent evidence ID:
`ev_885b82b3996b0453ad9edfb7382ee8c865847e89e66a730cba2150a989e3d4e6`.
The temporary source directory digest is
`1b52f4710a6bc1ad261f2ba6496c62314d72b9f9de1c48dc1512bd4cce03a645`.
No proprietary app implementation was copied into Verb.

## UX worker follow-up

The page/background request classification changed after the earlier worker analysis. REA was rerun
on an inert copy: one parsed file, zero failures, 2,158 AST nodes, 15 findings and 610 semantic unknowns.
Evidence: `/private/tmp/verb-browser-worker-rea-ux-20261028.json`; summary:
`/private/tmp/verb-browser-worker-rea-ux-summary-20261028.json`. Parent evidence:
`ev_9c32c17aa063597474c5268a2739ec6f8a67a8286b14808f86c479b86ea7bc70`.
Directory digest: `fa164773056333fb3f94967689e2a4bf9077d59f708eec02e6d642ff4324986a`.
Runtime tests still verify unapproved redirects are blocked before connecting.

## 30 October: renewed REA study and companion decision

At the user's request, REA 6.8.0 reanalyzed the two previously copied Codex browser modules
`hidden-browser-use-webview-host-a49029186764.js` and
`browser-background-switches-IQUsnzYP.js`. Neither app, browser profiles, cookies nor conversations
were opened or instrumented. Evidence: `/private/tmp/verb-browser-rea-recheck-20261030.json`;
summary: `/private/tmp/verb-browser-rea-recheck-summary-20261030.json`.

Two JavaScript files parsed, zero parse failures, 27,717 AST nodes visited and 63 findings. Coverage
is partial: the main bundle was deliberately not reanalyzed; 829 semantic unknowns remain. Static
observations of browser tab/conversation identities and presented versus hidden mounts support
separating runtime ownership from its visible surface. They do not establish all reachable behavior
or provide a redistributable Codex browser component. No proprietary implementation was copied into
Verb. Claude Desktop was not installed in the original inventory; its current public browser
[documentation](https://support.claude.com/en/articles/16607400-use-the-built-in-browser-in-claude-cowork)
confirms that its desktop runtime must remain online for browser use through web/mobile.

The user selected a Mac companion connected to public Verb. Verb's independent implementation uses
an outbound, browser-only relay, one-use owner pairing and the existing isolated Chromium worker.
It does not expose CDP, import a personal profile, or run Chromium on the phone. See the companion
contract for credentials, expiry, disconnects and remaining compatibility limits.

REA also inspected a pre-format snapshot of Verb's four browser modules (worker, URL policy,
companion runner and Studio controller) as inert source. Four files parsed, zero parse failures,
5,857 AST nodes visited and 41 findings; semantic coverage remains unknown with 1,539 unknowns.
Evidence: `/private/tmp/verb-browser-rea-companion-final-evidence-20261030.json`; summary:
`/private/tmp/verb-browser-rea-companion-final-summary-20261030.json`. These observations guided
manual lifecycle review; real Chromium/Rust tests establish the behavioral acceptance. REA is not
a security certification. Final formatting and pipe-error handling were reviewed separately.
