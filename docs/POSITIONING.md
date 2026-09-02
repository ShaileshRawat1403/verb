# Verb Positioning

This document defines how Verb is positioned, explained, and communicated across developer communities, social platforms, and creator ecosystems.

---

## 1. The claim

> **Verb is the control layer around coding agents. The agents generate. Verb owns the environment
> they run in, the record of what actually happened, and the way back when something breaks.**

You bring Claude Code, Codex or OpenCode, and the subscription you already pay for. They keep doing
the AI work. Verb answers the questions they cannot answer about themselves: which environment am I
in, what actually ran, what changed on disk, why did it fail, and what is safe to do next.

### Why that is the claim and not "AI coding on your phone"

Models are converging. A capability that is a differentiator this quarter is a checkbox two
quarters later, and every agent vendor ships the same features to every other vendor's users. What
does not converge is the layer underneath: environment identity, session survival, evidence of
execution, recovery, and the boundary around credentials. That layer is unglamorous, it is where
real adoption actually fails, and almost nobody is building it as a product.

So Verb is not positioned as a place to run agents. It is positioned as the thing that makes running
them accountable.

### Where mobile fits

Mobile is the proof, not the pitch.

The phone is the most hostile host an agent can be given. The OS kills long-running processes
without asking. Uninstalling the app destroys everything it owns. There is no second window to check
state in, and no laptop to fall back to. A control layer that keeps sessions, credentials and
evidence intact *there* is trivially credible anywhere else, which is why Verb was built on Android
first and why the desktop host implements the same contract rather than a looser one.

It also happens to be the only place a large number of people can build software at all. That is a
real consequence and worth saying. It is not the reason the product exists.

---

## 2. What Verb is not

Stated plainly, because each of these is a comparison a reader will make in the first ten seconds,
and being the wrong thing badly is worse than being a narrower thing well.

| Verb is not | Because |
| :--- | :--- |
| A coding agent | It contains no model and writes no code. It runs the agents you already use. |
| A model provider | The assistant is optional and replaceable. The context it answers from is the product. |
| An IDE | There is no editor, no build system and no project model. There is a terminal, and evidence about it. |
| A mobile terminal emulator | A terminal gives you a shell. Verb gives you session identity, recovery, command boundaries and an archive of the world. Termux is a dependency of Verb, not a competitor to it. |
| A hosted dev environment | Nothing is executed on someone else's machine. The runtime, the credentials and the record are on the device. |

---

## 3. Who feels this, and what they feel

One product, one claim. These are the three ways the same failure shows up.

### The engineer coordinating agents
Loses terminal state on app switch, cannot say which of three agents did which thing, and has no
record of what a background run actually changed. Verb gives session isolation per PTY, a durable
record keyed by the agent's own conversation id, and command boundaries with exit codes.

*"Multi-agent work you can audit, on a host that keeps its promises."*

### The creator building without a laptop
Pays for Claude or ChatGPT already, and cannot use the CLI because the setup assumes a developer
machine. Verb installs, signs in, and survives the phone doing what phones do. Failures are
explained in plain words rather than a stack trace.

*"You already pay for the agent. This is the studio that keeps its work."*

### The learner whose phone is the computer
Faces a hardware barrier, not a talent barrier. Verb turns an ordinary Android device into a real
Linux workspace with Git, Node, Python and the same agents professionals use.

*"Build on the device you already carry."*

---

## 3a. Messaging pillars

| Angle | Line | What it rests on |
| :--- | :--- | :--- |
| **Governance** | *"Agents generate. Verb governs."* | Session lifecycle, command boundaries, evidence-only answers, mediated recovery. |
| **Survival** | *"The terminal that remembers."* | Sessions resume by the agent's own conversation id after process death, proven on a physical device. |
| **Portability of work** | *"Your working world is a file."* | An encrypted, checksummed archive of agent logins and session records that outlives the app. |
| **Economics** | *"Bring your own subscription."* | No seat fee and no resale markup. Verb never sits between you and your agent provider. |
| **Privacy** | *"Structural memory, not surveillance."* | Durable records hold identity, context and state. Never a PID, command text, terminal bytes, transcripts or credentials. |

### Language to avoid

* Claims about what an agent can do. That is the agent vendor's sentence, not Verb's.
* "Autonomous". Verb's entire argument is that autonomy without a record is the problem.
* Capability claims without a dated device run behind them. `docs/BACKLOG.md` is the standard: if it
  is not accepted on hardware, it is not advertised.

---

## 4. Ready-to-Use Promotion Copy Templates

### Channel 1: X (Twitter) Thread Launch Template

```text
1/ The most powerful AI coding agents (Claude Code, OpenAI Codex) are trapped on desktop command lines.

If you don't have a laptop or deep terminal knowledge, you're locked out.

We built Verb to fix this: an on-device mobile studio for AI development.

Here is how it works: [Link]

2/ What is Verb?
Verb gives your Android phone a full Linux userland and genuine PTY engine.

You can install and run Anthropic's Claude Code, OpenAI Codex, or OpenCode with one tap.

No laptop required. Just sign in with the subscription you already have.

3/ What makes Verb different:

- Session durability: If Android kills the app to free memory, your conversation and project state restore automatically on reopen.
- Multi-terminal isolation: Run Claude in tab 1, Codex in tab 2, and shell in tab 3 without cross-talk.
- Ask Verb: Explains terminal errors in plain English.

4/ Zero-surveillance privacy:
Verb never stores command text, terminal output, or private code in durable memory. It operates on structured facts, not surveillance.

5/ Download the v0.1.0-beta.7 direct APK on GitHub today:
[GitHub Link]
```

---

### Channel 2: Hacker News (Show HN) Template

```text
Show HN: Verb - Run Claude Code, Codex, and OpenCode with real PTY isolation on Android

Hi HN,

We built Verb (https://github.com/ShaileshRawat1403/verb), a terminal-first development substrate that brings CLI coding agents (Claude Code, Codex, OpenCode) to Android devices and desktop.

The problem:
Frontier coding agents are distributed as interactive CLIs. On mobile, stock shells lack package ecosystems, and standard terminals lose all session state when the OS reclaims RAM.

What Verb does:
1. Embeds a PRoot Linux userland and PTY engine on Android (no root needed).
2. Implements a 4-state session contract (LIVE -> INTERRUPTED -> RECOVERABLE -> ENDED) that recovers interrupted agent sessions by their native conversation tokens.
3. Provides concrete session-bound PTY isolation, allowing concurrent agents (e.g. Claude in T1, Codex in T2, Shell in T3) without lifecycle cross-contamination.
4. Includes an evidence-bound assistant ("Ask Verb") that translates execution errors from structural facts without sending raw source code or PTY output to LLMs.

The project is open source (Apache 2.0) with an immutable supply-chain CI pipeline.

GitHub: https://github.com/ShaileshRawat1403/verb
Architecture Guide: https://github.com/ShaileshRawat1403/verb/blob/main/docs/learn/README.md

We would love your feedback on the session model and mobile PTY performance.
```

---

### Channel 3: Reddit (r/ChatGPT, r/ClaudeAI, r/androiddev, r/LocalLLaMA)

```text
Title: You can now run Claude Code and Codex CLI directly on your Android phone without a PC

Post:
If you pay for Claude Pro or ChatGPT Plus, you might know that Anthropic and OpenAI recently released powerful CLI coding agents. However, they usually require a Mac/Linux terminal and developer setup.

We built Verb, a free and open-source Android app that hosts these agents directly on your phone:

- One-tap install for Node, Git, Claude Code, and Codex.
- Uses your existing subscriptions (no third-party API reselling).
- Crash-proof: If Android puts the app to sleep, opening Verb resumes your exact agent conversation.
- Multi-terminal: Switch between agent tabs and shell tabs seamlessly.
- Ask Verb: Explains failed commands and exit codes in plain language if you get stuck.

Download the direct APK and read the open-source architecture guide here:
https://github.com/ShaileshRawat1403/verb
```

---

### Channel 4: Short-Form Video Hook (TikTok / YouTube Shorts / Reels)

* **Visual:** Close-up of an Android phone screen running a colorful Claude Code terminal interface refactoring a Python script.
* **Voiceover:** *"Did you know you can run Claude Code and OpenAI Codex directly on your phone without ever opening a laptop? This is Verb. It turns any Android device into a complete AI development studio. You log in with the subscription you already pay for, and it handles Git, packages, and crash recovery in your pocket. Link in bio to try the open-source build."*

---

## 5. Promotion Checklist Before Sharing

- [x] Verify direct APK download link points to the latest verified release tag (`v0.1.0-beta.7`).
- [x] Verify SHA-256 checksum is clearly visible for security verification.
- [x] Link to the `docs/learn/` guide for technical users wanting architectural proof.
- [x] Highlight that Verb uses the user's existing subscriptions (no hidden costs).
