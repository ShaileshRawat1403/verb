import { test } from "node:test";
import assert from "node:assert/strict";
import { agentAvailability, customCommandAvailable } from "../src/agent-availability.js";

const privateHost = {
  installed: { claude: true, codex: true, agy: false, opencode: false },
  restricted: true,
  programs: ["bash", "claude", "codex", "git", "node", "python3"],
};
test("isolated availability is explicit, including loading and failures", () => {
  assert.equal(agentAvailability(null, "shell").available, true);
  assert.equal(agentAvailability(null, "agy").available, false);
  assert.equal(agentAvailability(privateHost, "agy").available, false);
  assert.match(agentAvailability(privateHost, "agy").reason, /isolated runtime/);
  assert.equal(agentAvailability(privateHost, "claude").available, true);
  assert.equal(agentAvailability({ installed: {} }, "opencode").available, false);
  assert.match(agentAvailability(privateHost, "custom").reason, /Approved commands/);
});
test("provisioned agents are selectable and custom commands respect canonical paths", () => {
  const provisioned = { ...privateHost, installed: { ...privateHost.installed, agy: true, opencode: true }, programs: [...privateHost.programs, "agy", "opencode"] };
  for (const agent of ["agy", "opencode"]) {
    assert.equal(agentAvailability(provisioned, agent).available, true);
    assert.equal(customCommandAvailable(provisioned, agent), true);
    assert.equal(customCommandAvailable(provisioned, "/usr/local/bin/" + agent), true);
  }
  for (const command of ["opencode", "/tmp/bash", "../bash", "sudo", "git; false", "/usr/local/bin/git", "/usr/bin/claude"]) {
    assert.equal(customCommandAvailable(privateHost, command), false, command);
  }
  assert.equal(customCommandAvailable(privateHost, "/usr/bin/git"), true);
  assert.equal(customCommandAvailable(null, "git"), false);
  assert.equal(customCommandAvailable({ restricted: false }, "/opt/owner-cli"), true);
});

test("failed capability checks explain how to retry", () => {
  assert.equal(agentAvailability({ unavailable: true }, "agy").available, false);
  assert.match(agentAvailability({ unavailable: true }, "agy").reason, /Reopen to retry/);
});
