# Continue Verb work on a phone

Status: same-network live control implemented in the desktop web workbench, desktop CLI, and
Verb Mobile. The broader cross-device work handoff below remains a product proposal.
The click-through desktop/phone concept is `docs/mockups/desktop-phone-continuation.html`.
The desktop-local process contract is `docs/DESKTOP_MOBILE_BRIDGE_PROTOCOL.md`.

## The promise

A person can leave the desktop, open Verb on a phone, recognize the project and task, and continue
with a truthful account of where the agent is running. They should not have to understand process
IDs, transcript stores, or how an agent CLI resumes.

Verb has three different kinds of continuity:

| User need | What continues | Truth Verb must show |
| --- | --- | --- |
| Return to an agent on the same device | Its native conversation, if the adapter verifies its identity and store | **Resume this conversation** |
| Use the phone while the desktop agent is running | The *same desktop process* through a remote view and input connection | **Control on desktop** · desktop must be reachable |
| Take the work onto the phone | The task, decisions, project context, and a deliberate code checkpoint in a **new** phone session | **Continue on this phone** · new agent session |

The current `.vcont` import is a fourth, narrower operation: **View history from another device**.
It is read-only structural evidence. It must never be labelled Continue or Resume.

## First release to design for

Start with **control a live desktop session from the phone**. This fulfills the strongest meaning
of “same session”: the CLI, working tree, conversation store, and credentials stay on the desktop.
Verb Mobile is a small remote window and controller. It does not duplicate the agent or move a
transcript. The desktop must stay running and reachable. Scope the first acceptance run to a paired
phone and desktop on the same network; choose a broader transport only after this interaction is
proven on a physical phone. The current receiver displays a plain-text terminal screen and sends
explicit terminal input. It does not create a second agent process.

The next release is **continue as a new phone session** for times when the desktop is unavailable
or the user deliberately wants local execution. This requires a separate work handoff contract:
project identity, task and memory revisions, explicit next step, and a Git commit/branch or reviewed
patch. Neither `.vcont` nor the Android Working World archive serves this purpose. A task handoff
does not automatically carry CLI auth, a transcript, uncommitted files, or a live process.

## Desktop interaction

The Workbench keeps sessions and priority tasks together. A session row gains a contextual action
only when there is a real continuation path. It does not add a permanent navigation item.

```text
 VERB  /  website-redesign                                  2 agents running

 SESSIONS                          WORK THAT NEEDS ATTENTION
 ● Claude · Homepage copy          Review landing page copy     Needs review
   This Mac · running              Owner: Claude · 12 min ago
   [Open] [Continue on phone]      [Open task]

 ● OpenCode · Asset audit          Audit image licences         In progress
   This Mac · running              Owner: OpenCode
   [Open] [Continue on phone]      [Open task]

 MEMORY  Brand voice · Approved image sources · Current release goal
```

Selecting **Continue on phone** opens one focused sheet:

```text
 Continue Claude on your phone

 Claude and the project will keep running on this Mac.
 Your phone will control this exact terminal after pairing.

 [ Pair phone ]     [ Cancel ]
```

After pairing, show the device label, connection state, and one explicit control owner. The
desktop can always take control back. Closing the phone view does not end the agent. Ending the
agent requires the usual explicit session action.

If the desktop cannot host the connection, explain the missing prerequisite in plain language:
“This Mac is offline. You can view the last shared task update on your phone; live control is
unavailable.” Do not turn an imported `.vcont` record into a playable session.

## Phone interaction and future refinements

The first screen answers *what can I continue?* rather than opening a raw terminal first.

```text
 Continue work

 website-redesign                         This Mac · connected
 Claude · Homepage copy                   Running
 Last task update: Copy ready for review
 [ Continue live ]

 OpenCode · Asset audit                    Running
 [ Continue live ]

 History from another device              Recorded yesterday
 [ View history ]
```

The live terminal is one tap away. On a narrow screen, agent input remains primary; project and
task context is a short expandable sheet. A connection loss preserves the desktop process and
shows **Reconnect**. It never silently launches a second agent. Touch gestures must not become
agent keystrokes without an obvious input mode; terminal scrolling and keyboard focus need separate
affordances.

For a later local handoff, the choice must be equally direct:

```text
 Continue on this phone
 This starts a new agent session using the task and shared decisions.
 The desktop conversation will stay on the desktop.

 Project files: Git checkpoint ready
 Shared work: current
 Agent on phone: Claude available
 [ Start phone session ]
```

If any prerequisite is missing, name it where the action would be. For non-developers, “Project
files not available on this phone” is clearer than a Git error or a greyed-out button.

## Rules that protect trust

1. **One execution owner.** A session runs on exactly one host. Remote viewing does not create a
   second session. One device has keyboard control at a time, with an obvious take-control action.
2. **Local truth stays local.** Only the desktop can say its process is running or exact-resumable.
   If contact is lost, mobile says “last seen” with a time and stops claiming a live state.
3. **Pairing is explicit and revocable.** A short-lived pairing offer identifies the desktop and
   the session. Remote input is accepted only from a paired device with a scoped, expiring grant.
   The desktop shows and can revoke connected phones. No credential or native agent store is sent.
4. **Handoff is versioned.** A local-phone successor starts from a named task and memory revision.
   A changed task or checkpoint requires review before starting. Conflict resolution never silently
   overwrites a phone or desktop edit.
5. **Conversation identity is never guessed.** A new phone session can link to a desktop session as
   its predecessor without pretending it is the same native CLI conversation.
6. **The terminal remains optional for orientation.** Session and task names, next action, device,
   and connection state are readable before entering the terminal.

## Minimum acceptance journeys

- Start two desktop agent sessions; pair a phone; choose the intended one; send input; see its
  output on desktop and phone; switch to the other without changing either task owner.
- Disconnect and reconnect the phone. The same desktop process and native agent conversation are
  still there. The UI says “last seen” while disconnected and does not invent new output.
- Take control back on desktop while the phone is open. Only one side can type; both show who has
  control. Revoke the pairing and confirm further phone input is rejected.
- Quit the desktop host. Mobile says the session is unavailable and offers the recorded task and
  history, without claiming the process is still live or automatically starting a replacement.
- Later, start a phone-local successor from a task handoff. Verify that the new session has a new
  identity, the checkpoint and shared decisions match what was approved, and the desktop history
  still identifies the predecessor.

## Implementation boundary for this pass

This bridge does not make `.vcont` executable. The first live slice now has a pinned TLS relay,
QR/deep-link pairing, a mobile receiver, bounded screen snapshots, explicit input control,
heartbeat expiry, and desktop revocation. The web workbench offers **Phone** only for a live hosted
terminal; the TUI flow uses `verb mobile share SESSION_ID`. Automated tests prove the desktop TLS
path to a real PTY and Android link parsing. The physical-phone acceptance journeys above still
need a device on the same network, especially camera launch, touch layout, and interruption
behavior. Discovery of all project sessions on the phone and phone-local successor execution are
later product work.
