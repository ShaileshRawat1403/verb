# Module 03: The PTY Engine & Multi-Terminal Isolation

To build a reliable developer environment on mobile and desktop, Verb cannot rely on simple command execution (like `Runtime.getRuntime().exec("ls")`). Real developer tools—such as `git commit` (which opens an editor), `top`, `vim`, Claude Code, and Codex CLI—require a full interactive **Pseudoterminal (PTY)**.

Let's explore how Verb's PTY architecture works across Android and Desktop, and how it solves multi-terminal isolation.

---

## 1. What is a PTY?

A **Pseudoterminal (PTY)** is a pair of virtual devices provided by Unix operating systems:
1. **The Leader (Master):** Controlled by the application (Verb). Verb reads raw terminal bytes from here and writes user keyboard input to it.
2. **The Follower (Slave):** Connects to the child process (bash, zsh, node, python). To the child process, it looks and behaves exactly like a real hardware teletype terminal.

```text
+-----------------------+                    +------------------------+
|      Verb UI          |                    | Child Process          |
| (Compose / Ratatui)   |                    | (bash / claude / etc.) |
+-----------------------+                    +------------------------+
           |                                              ^
     Reads/Writes raw                               Interacts via
       escape bytes                                  stdin/stdout
           |                                              |
           v                                              v
  +------------------+     kernel PTY pair      +-------------------+
  |   PTY Master     | <======================> |    PTY Slave      |
  +------------------+                          +-------------------+
```

---

## 2. The Two Host Implementations

Verb runs on both Android and Desktop, implementing the exact same session contract on both:

### Android: Proot + Vendored Terminal Engine
* **The Problem:** Android's stock `/system/bin/sh` shell lacks essential Linux packages (`git`, `node`, `python`, `gcc`, `tar`). Furthermore, Android apps are sandboxed in private data directories (`/data/data/com.aistudio.verb.app/files`).
* **The Solution:** Verb embeds a Termux-derived PTY engine and a proot-backed Debian/Termux userland. The userland is compiled specifically for Verb's application ID, giving you a real Linux shell and package manager (`pkg`) without requiring root access.

### Desktop: Native Unix PTY in Rust
* **The Implementation:** Built in Rust (`desktop/src/pty.rs`), using native POSIX PTY APIs (`openpty`, `forkpty`).
* **Shell Integration:** Desktop reads the shell's OSC markers (`OSC 7` for working directory changes, `OSC 133` / `OSC 633` for command start/end boundaries) to record structured events without snooping on typed text.

---

## 3. Multi-Terminal Session Isolation

In modern workflows, developers frequently use multiple terminal tabs simultaneously:
* **Terminal 1 ($T_1$):** Running Claude Code refactoring backend code.
* **Terminal 2 ($T_2$):** Running OpenAI Codex writing unit tests.
* **Terminal 3 ($T_3$):** An interactive bash shell for running `git status` or `curl`.

### The Bug That Almost Shipped (And How It Was Fixed)
In early beta designs, lifecycle coordinators listened to the *currently visible* terminal. If you were running Claude in $T_1$, switched your screen to $T_2$, and typed `echo done`, the coordinator in $T_1$ would hear the command completion from $T_2$ and erroneously mark Claude as finished!

In `v0.1.0-beta.5`, this was permanently fixed with **Session-Bound Concrete Runtimes**:

```text
UI Viewport (Switchable)
  │
  ├── User switches active tab from T1 to T2 to T3
  │   (Changes ONLY visual rendering and keyboard routing)
  │
  ▼
Concrete Session Bindings (Immutable per session)
  ├── Session T1  ──>  PTY 1  ──>  ClaudeSessionCoordinator (strictly watches PTY 1)
  ├── Session T2  ──>  PTY 2  ──>  CodexSessionCoordinator  (strictly watches PTY 2)
  └── Session T3  ──>  PTY 3  ──>  Shell Process            (strictly watches PTY 3)
```

### The Invariant:
> **Once an agent or command starts in a terminal session, all lifecycle monitoring, event streams, and command dispatches are bound strictly to that session's concrete runtime.**

Switching tabs, rotating your phone, or running commands in other shells produces **zero** state changes in background agents.

---

## 4. Physical Proof of Isolation

This invariant is verified on physical hardware (Vivo I2202, Android 14):
1. Launch Claude Code in $T_1$ (`Running`).
2. Create $T_2$ and launch OpenAI Codex (`Running`).
3. Create $T_3$ and run shell commands (`echo testing_t3_isolation`).
4. Switch to $T_2$ and press `^C` to interrupt Codex.
5. **Result:** $T_2$ transitions to `Session interrupted`, while $T_1$ (Claude) remains `Running` without dropping a single frame.

---

## 5. Key Takeaways

* **Real PTYs everywhere:** Verb uses genuine POSIX PTYs on desktop and proot-backed PTYs on Android.
* **Zero cross-talk:** Background terminal sessions run independently; UI tab switching never mutates session lifecycle states.
* **Hardware-proven reliability:** Multi-agent concurrent execution is tested against real Android lifecycle events.

---

Next: **[Module 04: Agent Adapters & Evidence](04_agent_adapters_and_evidence.md)** explores how Verb extracts facts from different AI tools.
