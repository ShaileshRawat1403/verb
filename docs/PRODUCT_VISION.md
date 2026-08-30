# Verb Product Vision

**Verb makes AI-assisted development easier to understand, control, debug and recover.**

Modern AI agents can write substantial amounts of code and operate development environments, but the
user is increasingly separated from what is actually happening.

The recurring questions are simple:

* Where am I?
* What is happening?
* What changed?
* Why did it fail?
* What can I safely do now?
* Can I recover or reproduce this?

Verb exists to answer those questions.

Verb is a terminal-first environment where users bring the agents and harnesses they already use.
Claude, Codex, OpenCode, DAX and future systems remain responsible for their own reasoning and
coding capabilities.

Verb owns the layer around them:

**Access · Context · Understanding · Control · Debugging · Recovery**

Verb should become exceptionally good at conversational understanding of development work without
requiring the user to repeatedly explain context the machine can already know.

The terminal remains the primary workspace. Natural language becomes another way to interact with
that workspace rather than replacing it.

## The relationship to agents

```text
Claude / Codex / OpenCode / DAX / future agents
                    ↓
             do the AI work
                    ↓
                  Verb
                    ↓
     understands the environment,
     remembers what happened,
     explains what is happening,
     helps the user control it
```

Verb does not compete on being the smartest coding agent. Those models and harnesses will improve
faster than any wrapper can chase them, and a wrapper that tries becomes a worse version of the
thing it wraps. Verb makes them easier to use, easier to understand, and safer to control.

Stated as the line that decides arguments:

> Claude can write the code. Verb is the thing you ask when you need to understand what Claude just
> did to your world.

## Conversation-capable, not chat-first

A user should not have to open a chatbot and begin by explaining themselves:

> "I'm working on this repo and Claude changed something and now my tests are failing…"

Verb already knows the repo, the branch, the session, the agent, the commands, the exit codes and
the runtime. So the user can simply ask:

> **Why did this break?**
> **What did Claude change?**
> **Can I safely undo this?**
> **Why does this work locally but fail in CI?**

The user supplies intent. Verb supplies context. That is the whole trick, and it is the strongest
principle found so far:

> **Don't make the user explain context that the computer can already know.**

## Capability large, surface small

Users reach a fraction of any tool's functionality. The answer is not to display more of it.

> **Verb can have enormous capability while presenting very little at any one moment.**

Most of the time the screen is a terminal, a single status line, and a way to ask. Capability
appears when the situation calls for it - a failing test, a dangerous Git operation, an agent that
stopped, a runtime mismatch - and recedes when it does not.

## Software creation for everyone, everywhere

Autonomous coding agents should not be trapped behind complex desktop configurations. Millions of people carry smartphones as their primary computer, and millions subscribe to frontier AI services (like Claude Pro or ChatGPT Plus) without having a background in command-line administration.

Verb removes the hardware and terminal cliff:
* **No laptop required:** Packages a full Linux userland and genuine PTY engine directly on mobile.
* **Bring your own subscription:** Connects directly to the AI accounts you already pay for.
* **Fear-free exploration:** Automatic session recovery, multi-terminal isolation, and plain-English diagnostics ("Ask Verb") ensure that a phone sleep, memory sweep, or unexpected shell error never destroys your work.

## The constitutional constraint

> **Verb depends on capabilities and observable contracts, not the continued existence of any
> particular model, agent, harness or vendor.**

Every agent Verb supports is reached through an adapter that reads that agent's own observable
evidence. When an agent changes, one adapter changes. When a new agent appears, one adapter is
added. Nothing in Verb's core knows the name of a vendor.

## Three rules that are part of Verb's identity

```text
Unknown ≠ No
Inference ≠ Fact
Agent claim ≠ Verified execution
```

These are already load-bearing in the implementation rather than aspirational: `ResumeVerdict` has
three values because "unknown" is the absence of an answer and must not be collapsed into "no"; a
persisted `LIVE` session is reported as unconfirmed because nothing durable holds a process handle;
and an agent writing a session record at startup proves it was launched, not that a conversation
exists.

## Related documents

* `docs/PRD.md` — the problem, the product, and what is deliberately not built.
* `docs/ROADMAP.md` — capability milestones and their exit conditions.
* `docs/TUI_VISION.md` — how the desktop experience should work.

Everything else in `docs/` is implementation documentation: it records what was measured, decided
and built, and it answers to these four rather than the other way round.
