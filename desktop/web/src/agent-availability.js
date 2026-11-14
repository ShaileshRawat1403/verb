// Availability describes this execution environment, not the web server's PATH.
export function isolatedCheckoutAvailability(capabilities) {
  if (!capabilities) return { available: false, reason: "Checking checkout support…" };
  if (capabilities.unavailable) return { available: false, reason: "Checkout support could not be checked. Reopen to retry." };
  return capabilities.isolatedCheckouts === true
    ? { available: true, reason: "" }
    : { available: false, reason: "This host supports terminals in the current checkout only." };
}

export function agentAvailability(capabilities, agent) {
  if (agent === "shell") return { available: true, reason: "" };
  if (!capabilities) return { available: false, reason: "Checking availability…" };
  if (capabilities.unavailable) return { available: false, reason: "Availability check failed. Reopen to retry." };
  if (agent === "custom") {
    const commands = capabilities.programs ?? [];
    return capabilities.restricted
      ? { available: commands.length > 0, reason: `Approved commands: ${commands.join(", ")}` }
      : { available: true, reason: "Any executable available on this host" };
  }
  const available = capabilities.installed?.[agent] === true;
  return {
    available,
    reason: available ? "" : capabilities.restricted
      ? "Unavailable in this isolated runtime"
      : "Not installed on this host",
  };
}

export function customCommandAvailable(capabilities, command) {
  if (!capabilities) return false;
  if (!capabilities.restricted) return true;
  // Match the server's fixed canonical entry points; never accept arbitrary basename aliases.
  const match = /^(?:(\/usr\/local\/bin|\/usr\/bin|\/bin)\/)?([a-z0-9-]+)$/.exec(command);
  if (!match || !capabilities.programs?.includes(match[2])) return false;
  if (!match[1]) return true;
  return ["claude", "codex", "agy", "opencode"].includes(match[2])
    ? match[1] === "/usr/local/bin"
    : match[2] === "bash" || match[2] === "git"
      ? match[1] === "/usr/bin" || match[1] === "/bin"
      : match[1] === "/usr/bin";
}
