// One privacy boundary for every conversation mounted in this page, including hidden panels.
// A local off action blocks late responses before the server assigns the next generation.
export function createConversationPrivacy() {
  const panels = new Set();
  let current = null;
  let pending = false;
  return {
    subscribe(clear) {
      panels.add(clear);
      if (current?.enabled === false) clear(current);
      return () => panels.delete(clear);
    },
    hide() {
      pending = true;
      for (const clear of panels) clear({ ...current, enabled: false });
    },
    apply(setting, { explicit = false } = {}) {
      if (!Number.isFinite(setting?.generation)) return false;
      if (current && setting.generation < current.generation) return false;
      if (current && setting.generation === current.generation && setting.enabled !== current.enabled) return false;
      if (pending && setting.enabled && !explicit) return false;
      if (explicit || !setting.enabled) pending = false;
      const changed = !current || current.generation !== setting.generation || current.enabled !== setting.enabled;
      current = { enabled: setting.enabled, generation: setting.generation };
      if (changed) for (const clear of panels) clear(current);
      return true;
    },
    get blocked() { return pending || current?.enabled === false; },
  };
}
export const conversationPrivacy = createConversationPrivacy();
