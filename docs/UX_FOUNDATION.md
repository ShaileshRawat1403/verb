# Verb UX foundation

How Verb should feel, and the rules that keep it feeling that way. Host-agnostic on purpose: this
survives Ratatui, and would survive a desktop window, an Android surface or an SSH session.
`docs/TUI_VISION.md` is the current implementation of what is written here.

---

## 1. Product feel

Verb should feel like:

> **a terminal that quietly understands the work around it, and reveals help at the moment it
> becomes useful.**

Not:

> a terminal with eighty commands, forty skills and a settings catalogue.

The distinction is not modesty. Claude, Codex and OpenCode are extraordinary at breadth, and Verb
will never win that race — nor should it enter. Verb's advantage is that it knows *this* project,
*this* session, *this* failure, and can therefore be small on screen while being useful at exactly
the right moment.

**The interface can become very capable while still feeling small.**

---

## 2. Interaction grammar

### One surface answers one question

```text
Status line        → Where am I?
Action bar         → What can I do right now?
Context band       → What just happened?
Leader menu        → What else can I do?
Evidence view      → What does Verb actually know?
Scrollback         → What did I miss?
Sessions           → Where was I working?
Palette            → Give me everything, by name.
Ask Verb (later)   → Help me understand / decide / act.
```

### Quiet does not mean blank

Quiet means **obvious without demanding attention**. A screen with nothing actionable on it is not
restraint; it is a puzzle. The action bar is the primary visible affordance, and it is why the
bottom row exists at all — it carries what is useful *now*, and nothing else.

**The action bar must never become a toolbar.** Four actions is the ceiling and already close to too
many. When capability grows, the new things go behind Commands; the bar keeps showing only the
handful that matter in the current moment.

Nothing may sit in the chrome advertising something that does not work yet. The Ask row was reserved
for M2 and spent a month telling people about a feature they could not use; it is gone until M2
exists and can earn it back.

If a proposed element cannot be written as a clear user question, it does not exist yet. This is the
whole defence against the catalogue problem: a catalogue is what you get when surfaces are organised
by *what the software can do* instead of *what the person is asking*.

### The leader is the navigation

There is no permanent navigation chrome — no tab bar, no sidebar, no menu strip. One key opens
everything, and announces itself when pressed. Chrome that is always visible is chrome that is
usually wrong.

### Nothing appears without evidence

Verb may open exactly one thing on its own: the context band, and only because an observed fact
justified it. Every other surface is the user's move. A tool that volunteers a panel because it
*suspects* something is one people learn to fight.

---

## 3. The utility model: human moments, not feature categories

Verb is not organised around Git, sessions, agents, runtimes, CI, tools or skills. It is organised
around the moments a person actually has:

```text
ORIENT      Where am I? What is running?
NOTICE      Something changed or failed.
UNDERSTAND  What happened? What does Verb know?
RECOVER     Can I resume, retry, undo, or get back?
ACT         Do the next safe thing.
```

Git, Claude, Codex, npm, Docker, CI and whatever arrives next are **sources and capabilities
underneath these moments** — never top-level product categories. That is what makes the shape
future-proof: a new tool becomes a new source of evidence or a new safe action, not a new tab.

Mapping today's surfaces onto the moments:

```text
ORIENT       status line · workspace line · workspace sheet
NOTICE       context band
UNDERSTAND   evidence view · scrollback
RECOVER      resume · start new · workspace sheet
ACT          palette · contextual actions
```

### The workspace line is the status line Android never had

ORIENT has always been assigned to a status line here, and the desktop workspace has one. Android
did not. It had a project chip inside the header, and the header degrades under width pressure --
which arrives at exactly the moment a second terminal opens. So on the one screen where "which
project, which terminal" is hardest to hold in your head, the chip fell back to a folder glyph and
nothing named the terminal at all. Two people, including the author, lost track of which project
they were typing into on a physical Vivo I2202.

The workspace line sits below the header and never competes for its width, so the header's degrade
order is untouched. It states the project, the terminal, and what is running in it, and one tap
opens the workspace sheet -- projects and terminals in one list, terminals first because switching
between the agent's terminal and your own is an hourly move while changing project is a daily one.

This is not a retreat from *the leader is the navigation*. What that rule refuses is **browsing**:
permanent surfaces organised by what the software can do, which grow a row per destination. A line
that states where you are does not grow, offers no destinations, and is the same thing the desktop
status line was always meant to be. A workspace that will not say which directory it is in is not
minimal; it is withholding.

Two moments are visibly thin today — UNDERSTAND stops at raw evidence, ACT offers only what already
exists. That is the honest state of the product, and it is what dogfooding is meant to price.

---

## 4. Visual language

### Quiet by default

Most of the screen belongs to the work. Verb's own chrome is one status line and one input line
until something happens.

```text
 ~/work/api   main clean   claude   ● running                        leader ^Space

 $ npm test

 PASS  terminal/session.test.ts
 PASS  git/state.test.ts




 Ask Verb…                                                       available in M2
```

### Borders have meaning

A border says *this is a surface Verb has put in front of your work, and Escape will take it away*.
The terminal itself is never boxed, the workspace draws no rules between its regions — position and
emphasis already say what a line would — and nothing is nested inside another border.

```text
                ┌ Command Palette ──────────────────────────────┐
                │ > ncs                                         │
                │                                               │
                │ ▸ New Claude session                          │
                │   New Codex session                           │
                │   New OpenCode session                        │
                └───────────────────────────────────────────────┘
```

### Colour has meaning, and never carries it alone

```text
green    confirmed · running · succeeded
yellow   recoverable · needs a decision
red      an observed failure
dim      secondary · unknown · caveat
```

Four roles. No palette, no theming, no decoration. Every coloured thing also carries a word or a
glyph, because `NO_COLOR` is honoured, terminals disagree about palettes, and some readers cannot
see the difference:

```text
● running        ◐ recoverable        ◌ checking        ○ ended
✕ npm test · exit 1 · 2.0s
```

### Emphasis is structural

Bold marks a heading or a key to press. Reverse marks a selection or a mode badge. Italic and
underline are not used: terminals treat them inconsistently, and underline collides with hyperlink
rendering.

### Glyphs are narrow, on purpose

`● ◐ ◌ ○ ✕ ▸ … · ─` and nothing wider. Emoji are double-width in a fixed grid and break every column
after them.

### Plain language on screen, exact vocabulary underneath

```text
on screen        running · recoverable · checking · ended
in the contract  LIVE · RECOVERABLE · INTERRUPTED · ENDED
```

Both spellings appear together in help, so neither vocabulary is a secret. The change is the
reading, never the record.

### Density adapts; identity and control survive

Narrow terminals lose detail in a fixed order — never the leader hint, never the session state:

```text
110  ~/work/api   main clean   claude   ● running              leader ^Space
 60  …/api   claude   ● running                                leader ^Space
 40  …/api   ● running                                         leader ^Space
```

Below the minimum, Verb says so plainly rather than drawing badly.

---

## 5. The complexity budget

Verb has **three visual states**, and no fourth.

### 1. Quiet — what a person sees most of the time

```text
 ~/work/api   main clean   claude   ● running                        leader ^Space

 $ npm test

 PASS  session.test.ts
 PASS  git.test.ts


 Ask Verb…                                                       available in M2
```

### 2. Something happened — Verb takes two lines, because a fact justified them

```text
 ~/work/api   main   claude   ● running                              leader ^Space

 $ npm test
 …
 FAIL runtime.test.ts

 ✕ npm test · exit 1 · 2.0s
 ^Space v  what Verb knows
```

When the moment passes, Verb goes quiet again. Nothing lingers to be dismissed.

### 3. The user asked for more — one overlay, entered deliberately

```text
             ┌ What Verb knows ───────────────────────┐
             │ Observed now                           │
             │   ~/work/api · main · 3 changed        │
             │                                        │
             │ Recorded                               │
             │   command failed · exit 1              │
             │   process running                      │
             │                                        │
             │ Esc close                              │
             └────────────────────────────────────────┘
```

Everything powerful lives behind intentional invocation.

### The rule

> **Verb may become extremely capable internally while remaining extremely small externally.**

That is probably the differentiator. Claude, Codex and OpenCode can each carry hundreds of
capabilities; Verb does not need to *show* hundreds of things.

```text
many capabilities
       ↓
few human moments
       ↓
few surfaces
       ↓
one obvious next action
```

### The target

**90% terminal, 10% Verb** — a design target, not a literal guarantee at every terminal size. Verb
expands only when the user or the situation asks for it. In rows: one status line, at most two lines
of context band, one Ask line — four rows of chrome at the absolute maximum, and two of those only
when something happened.

What that means at the extremes, stated rather than implied:

```text
40 rows   quiet     2 chrome · 38 terminal    95%
40 rows   a moment  4 chrome · 36 terminal    90%
12 rows   quiet     2 chrome · 10 terminal    83%
12 rows   a moment  4 chrome ·  8 terminal    67%
```

At the 12-row minimum a contextual moment leaves two thirds of the screen to the work. That is still
terminal-majority, which is the guarantee; 90/10 is what it converges to at the sizes people
actually work in.

### Explicitly prohibited

* No permanent sidebar.
* No home dashboard full of cards.
* No top-level "Tools / Skills / Agents / Integrations" catalogue.
* No settings maze.
* No feature getting screen space merely because it exists.
* **No more than one Verb overlay at a time.**

### Configuration is searched, not browsed

When Verb has settings, they are found by name in the palette:

```text
^Space p

> leader

  Change Verb leader
```

Not:

```text
Settings
 ├ Terminal
 ├ Agents
 ├ Git
 ├ Runtime
 ├ AI
 ├ Appearance
 ├ Integrations
 └ Advanced
```

The second shape is how a tool becomes the thing this document exists to avoid, and it arrives one
reasonable-looking submenu at a time.

## 6. Progressive power

The same capability at different depths, rather than a beginner mode and an expert mode:

```text
See something
      ↓
Understand it
      ↓
Show evidence
      ↓
Offer safe action
      ↓
Reveal exact command / advanced control
```

A builder may stop at *"your tests failed because Node 24 is active."* An expert continues to the
evidence, the environment difference, the raw event, the exact command. One chain, entered at
different points, left at will.

Two rules keep the chain honest:

* **Every level must be skippable downward.** A person who wants the raw event should never have to
  read the explanation first.
* **Nothing deeper contradicts anything shallower.** If the plain sentence and the raw event
  disagree, the plain sentence was a guess and should not have been shown.

---

## 7. Language rules

* **Say the fact, then the action.** `✕ npm test · exit 1` then `enter resumes this conversation`.
* **Name absence explicitly.** "No session running" beats an empty pane. "The shell did not report
  what was running" beats quietly saying less.
* **Never promise what Verb cannot do.** A command called `explain` that explains nothing is the
  exact failure Verb exists to prevent. `context` assembles context, so it is called `context`.
* **Plain words, sentence case, no exclamation.** Hints are lowercase; headings are sentence case.
* **Introduce a term once, in parentheses, then use the plain word.** `running (LIVE)`.
* **Uncertainty is stated, not implied.** `running?` for a record Verb cannot confirm; "recovery
  status unknown" rather than an empty space where a status should be.
* **No exclamation marks, no cheerfulness, no apology.** The user is working.

---

## 8. Input rules

* **The terminal owns the keyboard.** Verb reserves exactly one chord — the leader — which is
  configurable, forwardable (`leader leader`), and forwards anything it does not claim.
* **Verb captures the mouse by default** so its small action surface is directly usable. Full-screen
  applications still own their terminal region; Option-drag preserves native selection, and
  `leader m` hands the mouse back to the terminal entirely. The current mode is stated on screen.
* **The mouse is never required.** Everything reachable by pointer is reachable by name in the
  palette.
* **`Esc` always closes the topmost Verb surface** and never does anything else.
* **No modal traps.** There is no state a person can reach where the terminal stops responding to
  the escape hatch.

---

## 9. Accessibility and constrained terminals

* `NO_COLOR` produces a screen with no escape sequences at all, and it stays readable because
  meaning was never carried by colour alone.
* Selection is marked by a glyph as well as by reverse video.
* Everything works keyboard-only, over SSH, in tmux, and in an editor's embedded terminal.
* Layout survives resize at any moment, including while a full-screen agent is running: the rendered
  rectangle is authoritative and the session is resized to match.
* Below the minimum size Verb reports the constraint instead of rendering something unreadable.

---

## 10. The anti-complexity funnel

The sections above are one thing said five ways, and it is worth saying once directly. Capability
growth is not resisted by taste or by discipline — both of which erode — but by a funnel it has to
survive:

```text
Capability growth
      ↓
must fit an existing human moment
      ↓
must fit an existing surface
      ↓
must fit the chrome budget
      ↓
must offer an obvious next action
```

If something cannot survive that, it probably does not belong in Verb. Note what this is *not*: it
is not "keep the UI simple", which is advice nobody has ever successfully followed. Each stage is a
question with a wrong answer, and two of them are checked by tests rather than by opinion.

## 11. Admitting a new surface

Before anything new is built, all of these must have an answer:

1. **Which user question does it answer?** In the user's words, not the system's.
2. **Which moment does it serve?** ORIENT, NOTICE, UNDERSTAND, RECOVER or ACT.
3. **Which observed fact justifies it appearing?** Or is it always the user's move?
4. **Which existing capability does it map to?** If none, the capability lands first, in the core,
   reachable from the CLI.
5. **What does it replace or fold into?** Additive-only design is how catalogues happen.
6. **What does it look like when empty, and when Verb does not know?**
7. **Is it reachable by name as well as by key?**
8. **Does it fit the complexity budget?** Which of the three states does it live in, and what does it
   cost in rows when quiet? The answer "a new permanent region" is a no.

And the admission test that outranks the list:

> **Does this make an existing development moment easier, clearer or safer — or does it merely add
> another capability to Verb?**

If it is the second, it stays out.

---

## Related documents

* `docs/PRODUCT_VISION.md` — why Verb exists.
* `docs/PRD.md` — the problem, the pillars, the non-goals.
* `docs/TUI_VISION.md` — the current implementation of this document.
* `docs/BACKLOG.md` — what is undone, and the current sprint. Section D0 records the required
  *shape* of work that has not started, so the funnel is applied before anyone writes code.
