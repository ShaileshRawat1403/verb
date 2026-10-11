#!/usr/bin/env python3
"""Disposable Linux container acceptance, real sudo/PTY/Talk/Git/cgroup; synthetic data only.

Run after copying the candidate verb and launcher into a --memory=512m Ubuntu test container
and installing sudo, python3, git, acl. Never run on the phone or a non-disposable host.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import urllib.request

assert os.environ.get("VERB_DISPOSABLE_TEST") == "1" and Path("/.dockerenv").exists()
assert os.geteuid() == 0

def run(*args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kwargs)

for group in ("verb-work", "verb-host", "verb-agent"):
    if subprocess.run(["getent", "group", group], capture_output=True).returncode:
        run("groupadd", "--system", group)
for user in ("verb-host", "verb-agent"):
    if subprocess.run(["getent", "passwd", user], capture_output=True).returncode:
        run("useradd", "--system", "--create-home", "--gid", user, "--groups", "verb-work", "--shell", "/bin/bash", user)
run("chmod", "700", "/home/verb-host", "/root")
run("chgrp", "verb-host", "/home/verb-agent")
run("chmod", "750", "/home/verb-agent")
run("install", "-d", "-o", "root", "-g", "verb-work", "-m", "2770", "/srv/verb-workspaces")
run("install", "-d", "-o", "verb-host", "-g", "verb-work", "-m", "2750", "/var/lib/verb-shell")
run("install", "-d", "-o", "root", "-g", "root", "-m", "755", "/etc/verb", "/usr/local/libexec")
run("install", "-o", "root", "-g", "root", "-m", "755", "/tmp/verb-agent-launch", "/usr/local/libexec/verb-agent-launch")
config = Path("/etc/verb/agent-launch.conf")
config.write_text("AGENT_HOME=/home/verb-agent\nWORKSPACE_ROOT=/srv/verb-workspaces\nAGENT_CGROUP=/sys/fs/cgroup\n")
config.chmod(0o644)
rule = Path("/etc/sudoers.d/verb-agent")
rule.write_text("Defaults!/usr/local/libexec/verb-agent-launch !use_pty, !log_input, !log_output, !log_allowed, !log_denied, !syslog, logfile=/dev/null, closefrom_override\nverb-host ALL=(verb-agent) NOPASSWD: /usr/local/libexec/verb-agent-launch\n")
rule.chmod(0o440)
run("visudo", "-cf", str(rule))
# Model Android's hidden cross-UID process list. The agent's absolute /usr/bin/ps still sees its UID.
host_uid = int(run('id','-u','verb-host').stdout)
ps_fixture = Path('/usr/local/bin/ps')
ps_fixture.write_text(f'#!/bin/sh\n[ "$(id -u)" != {host_uid} ] || exit 0\nexec /usr/bin/ps "$@"\n')
ps_fixture.chmod(0o755)
audit = Path('/tmp/fixture-sudo-audit.log')
audit.unlink(missing_ok=True)
audit_policy = Path('/etc/sudoers.d/fixture-audit')
audit_policy.write_text('Defaults logfile="/tmp/fixture-sudo-audit.log"\n')
audit_policy.chmod(0o440)
run('visudo', '-cf', str(audit_policy))
project = Path("/srv/verb-workspaces/fixture")
shutil.rmtree(project, ignore_errors=True)
shutil.rmtree('/home/verb-host/.verb', ignore_errors=True)
run("install", "-d", "-o", "verb-agent", "-g", "verb-work", "-m", "2770", str(project))
for args in (("init", "-q", "-b", "main"), ("config", "user.name", "Synthetic Agent"), ("config", "user.email", "fixture@example.com"), ("commit", "-q", "--allow-empty", "-m", "init")):
    run("runuser", "-u", "verb-agent", "--", "git", "-C", str(project), *args)
for path in (Path("/root/secret-marker"), Path("/home/verb-host/secret-marker")):
    path.write_text("SYNTHETIC_PRIVATE_MARKER")
    path.chmod(0o600)
run("chown", "verb-host:verb-host", "/home/verb-host/secret-marker")
identity = project / "identity.py"
identity.write_text('''import json, os
from pathlib import Path
def denied(path):
    try: Path(path).read_text(); return False
    except PermissionError: return True
status = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines() if ":" in line)
print(json.dumps({"uid": os.geteuid(), "home": os.environ["HOME"], "root_denied": denied("/root/secret-marker"), "host_denied": denied("/home/verb-host/secret-marker"), "token_absent": "VERB_TOKEN" not in os.environ, "nnp": status["NoNewPrivs"].strip(), "caps": [status[k].strip() for k in ["CapEff", "CapPrm", "CapInh", "CapAmb"]], "limit": Path("/sys/fs/cgroup/memory.max").read_text().strip(), "lock": any(".lock" in os.readlink(fd) for fd in Path("/proc/self/fd").iterdir() if fd.exists())}))
''')
agy = Path("/usr/local/bin/agy")
agy.write_text('#!/usr/bin/python3\nimport json, os, sys\nsys.stderr.write("x" * 600000)\nsys.stderr.flush()\nprint(json.dumps({"event":"result", "result":{"status":"SUCCESS", "response":"fixture UID " + str(os.geteuid())}}))\n')
agy.chmod(0o755)
cmd = ["runuser", "-u", "verb-host", "--", "env", "HOME=/home/verb-host", "VERB_AGENT_USER=verb-agent", "VERB_TOKEN=synthetic-owner-token", "/bin/bash", "-c", "umask 0002; exec /usr/local/bin/verb web --port 0"]
server = subprocess.Popen(cmd, cwd=project, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
try:
    line = server.stdout.readline()
    assert line.startswith("Verb web UI:"), line
    origin = line.split("Verb web UI: ", 1)[1].split(" (", 1)[0].strip()
    def api(method, path, data=None):
        body = None if data is None else json.dumps(data).encode()
        request = urllib.request.Request(origin + path, body, {"X-Verb-Token": "synthetic-owner-token", "Content-Type": "application/json", "Origin": origin}, method=method)
        try:
            with urllib.request.urlopen(request, timeout=15) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise AssertionError(error.read().decode()) from error
    terminal = api("POST", "/api/terminals", {"agent":"shell", "isolated":False})["sessionId"]
    api("POST", f"/api/terminals/{terminal}/input", {"data":f"python3 {identity} > identity.json\n"})
    result = project / "identity.json"
    for _ in range(100):
        if result.exists() and result.stat().st_size: break
        time.sleep(.1)
    facts = json.loads(result.read_text())
    uid = int(run("id", "-u", "verb-agent").stdout)
    assert facts["uid"] == uid and facts["home"] == "/home/verb-agent", facts
    assert facts["root_denied"] and facts["host_denied"] and facts["token_absent"], facts
    assert facts["nnp"] == "1" and all(int(mask, 16) == 0 for mask in facts["caps"]), facts
    assert facts["limit"] == "536870912" and facts["lock"], facts
    stubborn = project / "stubborn.py"
    stubborn.write_text('import os, signal, time\nfrom pathlib import Path\nsignal.signal(signal.SIGHUP, signal.SIG_IGN)\nsignal.signal(signal.SIGTERM, signal.SIG_IGN)\nPath("stubborn.pid").write_text(str(os.getpid()))\nwhile True: time.sleep(.1)\n')
    api("POST", f"/api/terminals/{terminal}/input", {"data":f"python3 {stubborn} &\n"})
    for _ in range(100):
        if (project / "stubborn.pid").exists(): break
        time.sleep(.1)
    stubborn_pid = int((project / "stubborn.pid").read_text())
    assert not run("runuser", "-u", "verb-agent", "--", "true").stderr
    no_root = subprocess.run(["runuser", "-u", "verb-agent", "--", "sudo", "-n", "true"], capture_output=True)
    assert no_root.returncode != 0
    spec = api("POST", "/api/specs", {"title":"Isolation", "problem":"fixture", "criteria":["works"]})
    specfile = next((project / "specs").glob("*.md"))
    run("runuser", "-u", "verb-agent", "--", "test", "-r", str(specfile))
    talk = api("POST", f'/api/specs/{spec["id"]}/talk')["talkId"]
    for _ in range(100):
        view = api("GET", f"/api/talk/{talk}")
        if not view["working"]: break
        time.sleep(.1)
    assert not view["working"], "noisy stderr hung Talk"
    assert any(item.get("text") == f"fixture UID {uid}" for item in view["items"]), view
    assert 'You are talking with the user through Verb' not in (audit.read_text() if audit.exists() else ''), 'sudo persisted the Talk prompt'
    hook = project / ".git/hooks/pre-commit"
    hook.write_text("#!/bin/sh\nid -u > hook-uid\n")
    hook.chmod(0o775)
    # Run the real Verb commit path; repository hooks must run as agent, not host.
    api("POST", "/api/git/commit", {"message":"fixture commit"})
    assert (project / "hook-uid").read_text().strip() == str(uid)
    api("DELETE", f"/api/terminals/{terminal}")
    for _ in range(100):
        stat = Path(f"/proc/{stubborn_pid}/stat")
        if not stat.exists() or stat.read_text().split()[2] == "Z": break
        time.sleep(.1)
    else: raise AssertionError("isolated terminal left a signal-ignoring child alive")
    original = config.read_text()
    config.write_text(original.replace("/sys/fs/cgroup", "/tmp"))
    attempt = subprocess.run(["runuser", "-u", "verb-host", "--", "sudo", "-n", "-u", "verb-agent", "--", "/usr/local/libexec/verb-agent-launch", str(project), "fixture", "--", "touch", str(project / "must-not-execute")], capture_output=True)
    assert attempt.returncode != 0 and not (project / "must-not-execute").exists()
    config.write_text(original)
    root_attempt = subprocess.run(["env", "VERB_AGENT_USER=verb-agent", "/usr/local/bin/verb", "version"], capture_output=True)
    assert root_attempt.returncode != 0
    run('chmod', '755', '/home/verb-host')
    bad_home = subprocess.run(['runuser','-u','verb-host','--','env','HOME=/home/verb-host','VERB_AGENT_USER=verb-agent','/usr/local/bin/verb','version'], capture_output=True)
    run('chmod', '700', '/home/verb-host')
    assert bad_home.returncode != 0, 'public host home was accepted'
    bad_state = subprocess.run(['runuser','-u','verb-host','--','env','HOME=/home/verb-host','VERB_STATE_DIR=/srv/verb-workspaces/state','VERB_AGENT_USER=verb-agent','/usr/local/bin/verb','version'], capture_output=True)
    assert bad_state.returncode != 0, 'shared host state was accepted'
    run('groupadd', '-g', '3004', 'fixture-raw') if subprocess.run(['getent','group','3004'], capture_output=True).returncode else None
    raw_group = run('getent', 'group', '3004').stdout.split(':')[0]
    run('usermod', '-aG', raw_group, 'verb-agent')
    raw_attempt = subprocess.run(['runuser','-u','verb-host','--','sudo','-n','-u','verb-agent','--','/usr/local/libexec/verb-agent-launch',str(project),'fixture','--','touch',str(project/'must-not-execute')], capture_output=True)
    run('gpasswd','-d','verb-agent',raw_group)
    assert raw_attempt.returncode != 0 and not (project/'must-not-execute').exists(), 'raw-network group was accepted'
    print("PASS: real Linux PTY/Talk/Git account separation, secret denial, session lock, no-new-privileges, zero capabilities, 512 MiB cgroup, noisy stderr, fail-closed configuration")
finally:
    server.terminate()
    try: server.wait(timeout=5)
    except subprocess.TimeoutExpired: server.kill(); server.wait()
