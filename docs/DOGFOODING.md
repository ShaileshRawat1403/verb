# Verb dogfooding protocol

The M1 workspace is frozen. The next milestone is not decided, and this document is how it gets
decided: by using Verb for real work and recording where it stopped helping.

**Record evidence, not feature ideas.** The temptation while writing a note is to also design the
fix. Don't. A note that says "I wish I could ask why this broke" is worth more than a note that says
"add an explain button", because the first one can still surprise us.

---

## Before you start (5 minutes, once)

### 1. Build the current binary

```bash
cd ~/MYAIAGENTS/verb
cargo build --release --manifest-path desktop/Cargo.toml
```

### 2. Put it on your PATH (optional, recommended)

```bash
cargo install --path desktop
```

Then `verb` works from any directory. Without this, use the full path
`~/MYAIAGENTS/verb/desktop/target/release/verb`.

### 3. Set the leader

`Ctrl+O` is the provisional default and **collides with zsh** (`accept-line-and-down-history`).
Use `Ctrl+Space`, which tested clean in bash, zsh and Claude:

```bash
export VERB_LEADER=ctrl-space
```

Put it in your own shell profile if you want it to persist. Verb will not put it there for you.

### 4. Open your notes file

```bash
open ~/verb-notes/friction.md
```

Keep it open in a second window. A note you have to go and find is a note you will not write.

---

## The keys

```text
^Space p    command palette (everything, by name)
^Space s    sessions across projects
^Space ?    help
^Space v    what Verb has observed
Esc         close the topmost Verb surface
^Space ^Space   send a literal Ctrl+Space to the terminal
```

Every other key belongs to the terminal. If that ever stops being true, that is a bug and a note.

---

## The three sessions

Do them in this order. Each one exercises a different part of the claim.

### Session 1 — ordinary shell work (30+ minutes)

Open Verb in a project you actually work in and just work. Git, tests, builds, editing, whatever
the day holds.

```bash
cd ~/some-real-project
verb
```

What to notice:

* Did the status line tell you something you would otherwise have had to check?
* Did the failure band fire when a command failed, and did it tell you anything useful?
* Did you forget Verb was there? (That is the good outcome.)
* Did you type something and have Verb eat it? (That is a bug — note it immediately.)

### Session 2 — a long agent session (60+ minutes)

Start Claude or Codex from the palette (`^Space p` → *New Claude session*) and do real work through
it.

Expect **less** from Verb here, and notice how that feels. Shell integration instruments shells; an
agent is not a shell, so you get session state (LIVE / RECOVERABLE / ENDED) but no command
boundaries. The interesting question is what you wanted to know that Verb could not tell you while
an agent was changing your code.

The specific test worth answering honestly:

> After an hour inside Claude through Verb, had I forgotten Verb was intercepting the keyboard until
> I deliberately invoked it?

If no — what pulled your attention to it?

### Session 3 — something deliberately messy (as long as it takes)

Pick one, or wait until one happens naturally:

* a failing test you have not diagnosed yet
* an agent you interrupt in the middle of an edit
* a Git mistake — wrong branch, bad merge, something you need to undo
* a runtime or dependency mismatch (works here, fails there)

This is the session most likely to produce the note that decides M2. Do not rush it and do not
"perform" it — a real mess is worth ten staged ones.

---

## What to record

One entry per moment of friction. The template is in `~/verb-notes/friction.md`:

```text
TASK
What was I trying to do?

VERB KNEW
What context/state was already visible?

FRICTION
Where did I stop understanding or controlling the work?

I LEFT VERB FOR
Claude / browser / GitHub / docs / another terminal / other

I WISH I COULD ASK
"..."

SEVERITY
minor / annoying / blocked
```

Rules that keep the evidence usable:

* **Write it when it happens**, not afterwards. Reconstructed friction is smoothed friction.
* **Rough is fine.** Fragments, typos, half-sentences.
* **No feature design.** If a solution occurs to you, write the question it answers instead.
* **Record the boring ones too.** Five "minor" notes about the same thing is a stronger signal than
  one "annoying" note about something unique.

Aim for **5–10 entries** across the three sessions. Fewer than five and there is nothing to cluster;
more than ten and you are probably designing rather than observing.

---

## What happens to the notes

Bring them back however rough. They get clustered into what the user actually left Verb *for*:

```text
"why does this happen"        → explanation / research
"what changed since it worked" → comparison / causal context
"can you safely undo this"     → guided action
something that fits none       → the most valuable outcome of all
```

That clustering decides M2. It is deliberately an open question — see `docs/ROADMAP.md`, where M2 is
written as `?` on purpose.

---

## Known at the freeze — not worth noting

These are already understood. Noting them costs you a slot in the log:

* The **Ask Verb** line is inactive. It is reserved space for M2 and says so.
* **One hosted session at a time.** Starting another closes the current one. *Unless it gets in your
  way* — then it is a finding, because that would mean the single-session model is wrong rather than
  merely unfinished.
* The failure band is **quiet inside agents** (see Session 2).
* The leader default is **provisional** until Codex and OpenCode collision rows are filled in, which
  needs the Android device.
* `dsh` cannot be installed on Android, and the Agents card says why.

## If Verb breaks

Not a friction note — a bug report. Capture:

1. what you typed
2. what the screen showed (a screenshot is fine)
3. `~/.verb/events/<project>/<session>.jsonl` if the session is relevant

Those three reproduce almost anything, because the event log is structural rather than narrative.
