// Compare the editable text. Publication carries forward the managed audit separately.
export function documentDiff(published, draft) {
  const original = published.split(/(?:^|\n)## Audit trail[ \t]*(?:\r?\n|$)/, 1)[0].trimEnd();
  const before = original.split("\n");
  const after = draft.trimEnd().split("\n");
  let start = 0;
  while (start < before.length && start < after.length && before[start] === after[start]) start++;
  let oldEnd = before.length;
  let newEnd = after.length;
  while (oldEnd > start && newEnd > start && before[oldEnd - 1] === after[newEnd - 1]) {
    oldEnd--;
    newEnd--;
  }
  const oldLines = before.slice(start, oldEnd);
  const newLines = after.slice(start, newEnd);
  if (!oldLines.length && !newLines.length) return "No differences in document text.";
  const width = newLines.length + 1;
  const rows = oldLines.length + 1;
  const at = start + 1;
  const diff = [`@@ Published line ${at} · Draft line ${at} @@`];
  // Keep review responsive when a document is replaced wholesale.
  if (width * rows > 250_000) {
    return [...diff, ...oldLines.map(line => `- ${line}`), ...newLines.map(line => `+ ${line}`)].join("\n");
  }
  const common = new Uint32Array(width * rows);
  for (let old = oldLines.length - 1; old >= 0; old--) {
    for (let next = newLines.length - 1; next >= 0; next--) {
      const index = old * width + next;
      common[index] = oldLines[old] === newLines[next]
        ? common[index + width + 1] + 1
        : Math.max(common[index + width], common[index + 1]);
    }
  }
  let old = 0;
  let next = 0;
  while (old < oldLines.length || next < newLines.length) {
    const index = old * width + next;
    if (old < oldLines.length && next < newLines.length && oldLines[old] === newLines[next]) {
      diff.push(`  ${oldLines[old++]}`);
      next++;
    } else if (old < oldLines.length && (next === newLines.length || common[index + width] >= common[index + 1])) {
      diff.push(`- ${oldLines[old++]}`);
    } else {
      diff.push(`+ ${newLines[next++]}`);
    }
  }
  return diff.join("\n");
}
