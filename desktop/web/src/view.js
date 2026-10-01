// Pure view helpers: data in, text or HTML out, no DOM and no network. Kept apart from app.js so
// they can be tested with `node --test` (see test/view.test.js), which is where the rule that every
// value from the repository or Git is escaped before it reaches the page is checked.

export function escapeHtml(value) {
  return String(value ?? "").replace(
    /[&<>"']/g,
    (character) =>
      ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;",
      })[character],
  );
}

export function sessionAction(session) {
  if (session.hasTerminal) return "Open terminal";
  if (session.canResume) return "Resume session";
  return "View inbox";
}

export function stateName(state) {
  return (
    {
      live: "Running",
      recoverable: "Ready to resume",
      interrupted: "Recovery unconfirmed",
      ended: "Ended",
    }[state] || state
  );
}

export function taskName(status) {
  return (
    {
      open: "Open",
      active: "In progress",
      "needs review": "Needs review",
      done: "Done",
    }[status] || status
  );
}

export function agentMark(agent) {
  if (/claude/i.test(agent)) return "✳";
  if (/codex/i.test(agent)) return "✦";
  if (/agy|antigravity/i.test(agent)) return "▲";
  if (/gemini/i.test(agent)) return "♊";
  if (/opencode/i.test(agent)) return "◇";
  return "&gt;_";
}

export function agentDisplayName(agent) {
  if (/^shell$/i.test(agent)) return "Terminal";
  if (/^claude$/i.test(agent)) return "Claude Code";
  if (/^codex$/i.test(agent)) return "Codex";
  if (/^agy|antigravity$/i.test(agent)) return "AGY";
  if (/^gemini$/i.test(agent)) return "Gemini";
  return agent;
}

export function filterSessions(sessions, query) {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return sessions;
  return sessions.filter((session) =>
    [
      session.agent,
      session.id,
      stateName(session.state),
      session.isolated ? "isolated" : "main",
    ]
      .join(" ")
      .toLocaleLowerCase()
      .includes(needle),
  );
}

export function filterTasks(tasks, query, ownerLabel) {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return tasks;
  return tasks.filter((task) =>
    [
      task.title,
      task.brief,
      taskName(task.status),
      task.needsHelp ? "help requested" : "",
      task.owner ? ownerLabel(task.owner) : "unassigned",
    ]
      .join(" ")
      .toLocaleLowerCase()
      .includes(needle),
  );
}

// Everything below comes from `verb check`: observed facts with the safe next step beside each.
// Verb runs none of the steps; they are shown so the person can choose.
export function checkRow(mark, tone, fact, next) {
  return `<div class="check-row" data-tone="${tone}">
    <span class="check-mark" aria-hidden="true">${mark}</span>
    <span class="check-copy"><strong>${escapeHtml(fact)}</strong>${next ? `<code>${escapeHtml(next)}</code>` : ""}</span>
  </div>`;
}

export function checksHtml(report) {
  const rows = [];
  if (report.repository === null) {
    rows.push(
      checkRow(
        "?",
        "quiet",
        report.repositoryStatus === "unavailable"
          ? "Git could not read this checkout here."
          : "Not a Git repository.",
        report.repositoryStatus === "unavailable"
          ? "not installed, or it refused a checkout owned by another account"
          : "",
      ),
    );
  } else {
    for (const warning of report.repository) {
      rows.push(
        checkRow(
          "!",
          warning.level,
          warning.fact,
          `safe next: ${warning.safeNext}`,
        ),
      );
    }
  }
  for (const fact of report.runtimes) {
    if (fact.verdict === "satisfied") continue;
    const found =
      fact.found ??
      (fact.probe === "notFound" ? "not installed" : "no version reported");
    rows.push(
      checkRow(
        fact.verdict === "unknown" ? "?" : "!",
        fact.verdict === "unknown" ? "quiet" : "caution",
        `${fact.runtime} ${found} · ${fact.source} wants ${fact.wants}`,
        {
          unknown: "Verb cannot compare this requirement",
          mismatch:
            "Verb's environment runs a different version than the project declares",
          missing: "declared by the project but not installed here",
        }[fact.verdict] || "",
      ),
    );
  }
  const good = report.lastKnownGood;
  if (good) {
    const d = good.distance;
    const parts = [];
    if (d.markMissing) parts.push("the marked commit is gone");
    else if (d.identical) parts.push("exactly as marked");
    else {
      if (d.commitsSince)
        parts.push(
          `${d.commitsSince} commit${d.commitsSince === 1 ? "" : "s"}`,
        );
      if (d.commitsDropped) parts.push(`${d.commitsDropped} dropped`);
      if (d.filesDiffer === null) parts.push("file changes unknown");
      else if (d.filesDiffer > 0 || !parts.length)
        parts.push(
          `${d.filesDiffer} file${d.filesDiffer === 1 ? "" : "s"} differ${d.filesDiffer === 1 ? "s" : ""}`,
        );
    }
    rows.push(
      checkRow(
        "◆",
        "quiet",
        `Last known good ${(good.mark.head || "").slice(0, 12)}: ${parts.join(", ") || "no change"}`,
        d.identical ? "" : "verb good files · lists what differs",
      ),
    );
  }
  const satisfied = report.runtimes.filter((f) => f.verdict === "satisfied");
  if (report.clear) {
    rows.unshift(
      checkRow(
        "✓",
        "clear",
        "Nothing observed calls for care.",
        satisfied.length
          ? satisfied.map((f) => `${f.runtime} ${f.found}`).join(" · ")
          : "",
      ),
    );
  }
  return rows.join("");
}

// The input endpoint bounds UTF-8 bytes, including pasted text.
export function takeInputChunk(input, maxBytes = 8192) {
  const encoder = new TextEncoder();
  let length = 0;
  let bytes = 0;
  for (const character of input) {
    const size = encoder.encode(character).length;
    if (bytes + size > maxBytes) break;
    bytes += size;
    length += character.length;
  }
  return { chunk: input.slice(0, length), rest: input.slice(length) };
}
