# Verb Positioning and Promotion Playbook

This document defines how Verb is positioned, explained, and promoted across developer communities, social platforms, and creator ecosystems.

---

## 1. The Core Hook and Elevator Pitch

### The 10-Second Pitch
> **"Verb turns the Android phone in your pocket into an autonomous AI development studio. Run Claude Code, OpenAI Codex, and OpenCode with real Git and a full Linux userland. No laptop, no command-line expertise, and no complex setup required. Just bring the subscription you already use."**

### The 30-Second Pitch
> **"Frontier AI coding agents like Anthropic's Claude Code and OpenAI's Codex CLI are incredibly capable, but they are trapped on desktop terminals and require complex developer setups. Verb brings these exact tools to your mobile device. With a full Linux userland, automatic crash recovery, multi-terminal isolation, and plain-English diagnostics, Verb lets anyone -- from seasoned engineers to first-time creators -- build real software from anywhere."**

---

## 2. The Three Target Audiences and Value Propositions

```text
+-----------------------------------------------------------------------------+
|                                    VERB                                     |
+------------------------------------+----------------------------------------+
| 1. The Mobile-First Creator        | 2. The Professional Engineer           |
|    * Has a Claude/ChatGPT sub      |    * Needs mobile-to-desktop continuity|
|    * May not own a laptop or PC    |    * Runs concurrent background agents |
|    * Needs fear-free simple setup  |    * Demands zero-surveillance privacy |
+------------------------------------+----------------------------------------+
| 3. The Global Student & Learner                                             |
|    * Mobile is their primary computer                                       |
|    * Learns software development directly on their phone                    |
|    * Explains cryptic errors in plain language via Ask Verb                 |
+-----------------------------------------------------------------------------+
```

### Audience A: The Non-Developer / Mobile-First Creator
* **Core Pain Point:** Paying $20/month for Claude Pro or ChatGPT Plus, but unable to use powerful CLI tools because they do not have a developer setup or laptop.
* **The Solution:** Install Verb APK -> Tap "Start Agent" -> Log in with existing account -> Start building.
* **Key Message:** *"You already have the AI subscription. Now you have the studio to build with it on your phone."*

### Audience B: The Professional Engineer / Power User
* **Core Pain Point:** Losing terminal state when switching apps on mobile, dealing with fragile SSH connections, and needing multi-agent coordination.
* **The Solution:** True PTY multi-terminal isolation ($T_1$ Claude + $T_2$ Codex + $T_3$ Shell), cross-device `.vcont` continuity, and crash-proof conversation recovery.
* **Key Message:** *"A mobile terminal that respects your state. Run multi-agent workflows with guaranteed session isolation and zero surveillance."*

### Audience C: The Global Student and Aspiring Builder
* **Core Pain Point:** Expensive hardware barrier to learning modern AI-assisted software engineering.
* **The Solution:** Turns any budget $100-$200 Android smartphone into a complete Linux workstation with Git, Node, Python, and AI agents.
* **Key Message:** *"Software creation for the next billion builders. Learn and build on the device you already carry."*

---

## 3. Key Messaging Pillars and Taglines

| Angle | Primary Tagline | Supporting Copy |
| :--- | :--- | :--- |
| **Accessibility** | *"Software creation in your pocket."* | Run frontier AI coding agents directly on your phone with zero developer configuration. |
| **Economics** | *"Bring your own subscription."* | No seat fees, no resale markups. Use the Claude Pro or ChatGPT Plus account you already pay for. |
| **Reliability** | *"The terminal that remembers."* | Sessions survive app kills, OS memory sweeps, and device reboots without losing conversation state. |
| **Privacy** | *"Structural memory, not surveillance."* | Your code, credentials, and keystrokes stay private. Verb stores state structure, not your data. |

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

5/ Download the v0.1.0-beta.5 direct APK on GitHub today:
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

- [x] Verify direct APK download link points to the latest verified release tag (`v0.1.0-beta.5`).
- [x] Verify SHA-256 checksum is clearly visible for security verification.
- [x] Link to the `docs/learn/` guide for technical users wanting architectural proof.
- [x] Highlight that Verb uses the user's existing subscriptions (no hidden costs).
