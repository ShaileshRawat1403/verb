# Module 01: The Verb Philosophy & The Boundary

When people first see Verb running on a phone or desktop, they often ask:
* *"Is this a new AI coding agent like Claude or Codex?"*
* *"Is this a mobile IDE like VS Code or Termux?"*
* *"Is this a wrapper around an LLM API?"*

The answer to all three is **no**.

To understand Verb, you have to understand the **boundary** it draws between the tools that create code and the environment that runs them.

---

## 1. The Separation of Powers

In modern software development, AI coding assistants (like Anthropic's Claude Code, OpenAI Codex CLI, or OpenCode) are incredible at **generation and reasoning**. They generate code, write tests, and suggest terminal commands.

However, when things go wrong, developers often face a cognitive breakdown:
* *What commands did the agent actually run?*
* *Did that background command succeed or fail?*
* *Which environment or working directory was active?*
* *If the phone went to sleep or the app crashed, where was the agent working?*
* *Is it safe to undo the last step?*

Verb was created to solve these problems **around** creation:

```text
+----------------------------------------------------------------+
|  Claude Code  /  OpenAI Codex  /  OpenCode  /  Future Agents   |
|  (They do the thinking, planning, and code writing)            |
+----------------------------------------------------------------+
                               |
                               v
+----------------------------------------------------------------+
|  VERB (The Substrate)                                          |
|  * Manages the execution environment and real PTY processes    |
|  * Observes what actually ran and what files changed           |
|  * Preserves session identity across crashes and restarts      |
|  * Explains failures using factual evidence                    |
+----------------------------------------------------------------+
```

Verb does **not** compete with coding agents; it hosts them. You bring your existing subscriptions and CLI tools, and Verb provides the reliable runtime and memory layer around them.

---

## 2. The Core Architecture Pipeline

In many AI tools, the AI model is placed in front of everything: it guesses what the system state is and acts on its own guesses. 

Verb strictly reverses this relationship:

```text
[ OBSERVED FACT ]
        ↓
[ STRUCTURED VERB STATE ]
        ↓
[ DURABLE MEMORY / CONTEXT ]
        ↓
[ USER INTERFACE ]
        ↓
[ AI INTERPRETATION (Optional) ]
        ↓
[ USER-APPROVED ACTION ]
```

### The Rule: AI Sits *After* Evidence
An AI interpretation that cannot point to a concrete observed fact is merely a guess wearing a confident voice. Verb guarantees that:
1. Every state transition is backed by a real filesystem or PTY observation.
2. Every answer from "Ask Verb" points directly to the structured facts it used.
3. No AI model is ever allowed to invent or rewrite historical records.

---

## 3. The Three Golden Axioms Explained

Let's look at why Verb's three core axioms matter in practice:

### Axiom 1: `Unknown ≠ No`
* **Common trap:** If an app checks if a process is running and fails to connect, it might conclude *"The process has ended."*
* **Verb's approach:** If Verb loses sight of an agent because the app was force-stopped or the device rebooted, it does *not* assert that the agent cleanly ended. It marks the state as `INTERRUPTED` or `UNCONFIRMED`. It only transitions to `ENDED` when it observes a verifiable exit code.

### Axiom 2: `Inference ≠ Fact`
* **Common trap:** An LLM reads a prompt and concludes *"The database migration failed because port 5432 was busy."*
* **Verb's approach:** That is an inference, not an observed event. Verb records only what happened (e.g., `exit_code: 1`, `duration_ms: 420`, `timestamp: 2026-08-30T02:30:00Z`). The explanation is shown as an assistant layer, clearly citing the exit code as evidence.

### Axiom 3: `Agent claim ≠ Verified execution`
* **Common trap:** An agent outputs *"I have created `config.json`"*.
* **Verb's approach:** Verb does not believe the agent's text output. It checks Git status or the filesystem to verify whether `config.json` was actually created on disk.

---

## 4. Key Takeaways

* **Verb is a host and substrate**, not an agent or IDE.
* **Separation of concerns:** The agent reasons; Verb observes and preserves truth.
* **Evidence first:** Facts are captured structurally before any AI interpretation is requested.

---

Next: **[Module 02: Session Lifecycle & Recovery](02_session_lifecycle_and_truth.md)** explores the 4-state lifecycle and how sessions survive process death.
