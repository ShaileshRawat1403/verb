# Module 03: The PTY Engine and Multi-Terminal Isolation

To host real developer tools (such as interactive editors like `vim` or `nano`, Git interactive rebasing, or terminal agents like Claude Code), an application cannot rely on basic script execution. It must provide a genuine **Pseudoterminal (PTY)**.

Let us explore how Verb's PTY engine works across Android and Desktop, and how it achieves bulletproof multi-terminal isolation.

---

## 1. What is a Pseudoterminal (PTY)?

In Unix-like systems (Linux, macOS, BSD), a **Pseudoterminal (PTY)** is a pair of interconnected virtual devices:
* **The Master (Host):** Controlled by Verb. Verb writes user keystrokes into it and reads raw text and ANSI escape codes (colors, cursor positioning) out of it.
* **The Slave (Child):** Attached to the child program (bash, zsh, node, python). The child program behaves exactly as if it were plugged into a physical hardware terminal screen.

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

## 2. How Verb Runs on Android and Desktop

Verb delivers the exact same session guarantees on two distinct environments:

### Android: PRoot Virtualization + Termux Userland
* **The Challenge:** Android devices do not come with standard Linux package managers (`apt` or `pkg`), and apps are strictly sandboxed inside private folders (`/data/data/com.aistudio.verb.app/files`).
* **The Verb Solution:** Verb bundles an open-source, Termux-derived PTY engine alongside a [PRoot](https://proot-me.github.io/) userland compiled specifically for Verb. This provides full Linux tools (`git`, `node`, `python`, `gcc`, `tar`) without needing root privileges on the device.

### Desktop: Native POSIX PTY in Rust
* **The Implementation:** Written in Rust (`desktop/src/pty.rs`), using standard POSIX PTY system calls (`openpty`, `forkpty`).
* **Shell Integration:** The desktop host listens to standard [OSC (Operating System Command) Escape Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html):
  * `OSC 7`: Signals directory changes (`cd /path/to/project`).
  * `OSC 133` / `OSC 633`: Signals command start and completion boundaries, allowing Verb to record execution timing without logging private keystrokes.

---

## 3. Multi-Terminal Session Isolation

Developers often work across multiple terminal tabs at the same time:
* **Terminal 1 ($T_1$):** Claude Code refactoring an authentication service.
* **Terminal 2 ($T_2$):** OpenAI Codex running unit tests.
* **Terminal 3 ($T_3$):** A general shell running `git diff` or checking log files.

```text
UI Viewport (Switchable by user)
  │
  ├── User switches active tab from T1 to T2 to T3
  │   (Changes ONLY visual rendering and keyboard input routing)
  │
  ▼
Concrete Session Bindings (Strictly isolated per session)
  ├── Session T1  ──>  PTY 1  ──>  ClaudeSessionCoordinator (strictly observes PTY 1)
  ├── Session T2  ──>  PTY 2  ──>  CodexSessionCoordinator  (strictly observes PTY 2)
  └── Session T3  ──>  PTY 3  ──>  Shell Process            (strictly observes PTY 3)
```

### The Isolation Invariant:
> **Once an agent or command launches in a terminal session, all lifecycle watchers, event listeners, and command dispatches are bound strictly to that session's concrete runtime.**

Switching tabs in the UI, rotating the screen, or typing commands into other terminals causes **zero** state changes in background agents.

---

## 4. Physical Proof of Isolation

This invariant has been verified on physical hardware (Vivo I2202, Android 14):
1. **$T_1$:** Launched Claude Code (`Running`).
2. **$T_2$:** Launched OpenAI Codex (`Running`).
3. **$T_3$:** Created an interactive shell and ran `echo testing_t3_isolation`.
4. **Action:** Switched to $T_2$ and pressed `^C` to interrupt Codex.
5. **Observed Outcome:** Only $T_2$ transitioned to `Session interrupted`. Claude in $T_1$ remained `Running` and completely undisturbed.

---

## 5. Related Open-Source References
* [PRoot Userland Virtualization](https://proot-me.github.io/): Allows running Linux distribution userlands inside unprivileged environments.
* [Termux Packages Project](https://github.com/termux/termux-packages): Open-source build infrastructure for Android terminal environments.
* [XTerm Control Sequences Documentation](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html): The reference standard for terminal escape codes and OSC markers.

---

## 6. Key Takeaways

* **Authentic PTYs everywhere:** Verb uses genuine POSIX PTYs on desktop and PRoot-backed userland PTYs on Android.
* **Session-bound routing:** Background sessions run independently; UI navigation never mutates background lifecycles.
* **Device-verified isolation:** Multi-terminal separation is validated under real hardware conditions.

---

Next: **[Module 04: Agent Adapters and Evidence](04_agent_adapters_and_evidence.md)** explores how Verb extracts facts from AI agents without invasive screen scraping.
