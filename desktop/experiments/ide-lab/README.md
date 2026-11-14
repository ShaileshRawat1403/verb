# Full-IDE foundation laboratory

Local, synthetic comparison for Verb's full-capability IDE direction. It is not a
production integration, a phone deployment or a replacement for Verb's Rust host.
See [the first findings](RESULTS.md) before choosing a foundation.

## What is here

- `prepare.py`: creates a private temporary lab, a dependency-free JS web app,
  Python unittest, Git branches with a reproducible conflict, task/debug configuration
  and a locally authored VSIX probe. It copies no existing project or credentials.
- `build.py`: builds trial images with the same Node 22 runtime and Python support.
  Official IDE and toolchain image digests are pinned. Apt package resolution is
  not locked to an archive snapshot; record versions on each run.
- `lab.py`: start/status/stop for receipt-owned containers only. One IDE at a time,
  2 CPUs, 3 GiB, 512 PIDs, non-root, no capabilities and no-new-privileges.
- `forward.py` / `Dockerfile.proxy`: fixed upstream TCP forwarding for IDE/preview
  ports. The proxy is read-only, runs as nobody, and has its own small limits.
- `fetch_theia.py`: optional official-registry fallback when Docker's GHCR HEAD
  requests fail. Checks the pinned ARM64 manifest, every blob digest and every
  uncompressed filesystem-layer digest, then imports a Docker image archive.
- `test_lab.py`: failure/ownership guards and actual synthetic fixture build/test/
  conflict checks. It never contacts Docker or a phone.

IDE containers have an **internal-only network and no mounts**. A small trusted
proxy has a normal bridge plus that internal network and publishes only loopback
ports. It forwards to the fixed owned IDE container; it is not an HTTP forward proxy.
No Docker socket, host HOME, SSH agent, provider login, production ledger or actual
source checkout is given to either IDE. No public DNS/tunnel/service is configured.
Authentication is disabled only inside this synthetic loopback lab. This setup is
not a production authentication or extension-isolation design.

## Run a new trial

Use Docker already running on the Mac. Never restart Docker or other containers as
part of this lab. Normal image pulls use the official public sources. If a credential
helper stalls on public pulls, a private temporary Docker client config with an
explicit existing Docker endpoint can avoid consulting it; do not change the user's
Docker config or read its credential entries.

From this directory:

```sh
python3 -m unittest -v
python3 prepare.py
```

Record the printed lab path as `<lab-root>`. The receipt is owner-only. Then obtain
the exact images:

```sh
docker pull --platform linux/arm64 codercom/code-server@sha256:377b66bec5645510ed52549bb00e648c273edc604d230a3f90fd2b2433afcb97
docker pull --platform linux/arm64 ghcr.io/eclipse-theia/theia-ide/theia-ide:1.76.0@sha256:85e2ffa998a6815b731cddda5b46e1fd8fc5d9857c02b318bfc2edda67d9b5e9
```

`build.py` expects the verified Theia base under `verb-ide-theia:1.76.0`. Tag the
successfully pulled exact image with that local name. Alternatively, the fallback
below produces that name after digest verification:

```sh
python3 fetch_theia.py
```

Build and run candidates sequentially:

```sh
python3 build.py code-server --root <lab-root>
python3 build.py theia --root <lab-root>
python3 lab.py start code-server --root <lab-root>
python3 lab.py status code-server --root <lab-root>
python3 lab.py stop code-server --root <lab-root>
python3 lab.py start theia --root <lab-root>
python3 lab.py status theia --root <lab-root>
python3 lab.py stop theia --root <lab-root>
```

Open the URL printed by start in a dedicated browser tab. The published port may
change after a restart; always use the current receipt/output. The older preview
at port 58290 is unrelated and should stay stopped.

code-server requires trusting this **synthetic `/home/project` folder** before
executing its tasks/extensions. Keep global workspace-trust protection enabled.
Theia's default plugin directory does not install a raw VSIX automatically: use
**Extensions: Install from VSIX…**, switch the remote picker to text input, enter
`/home/theia/plugins/verb-lab-probe.vsix`, press Enter, select it, then Install.
This uses a container-local file, not a host file upload or extension marketplace.

## Workflow and truthful evidence

1. Run **Verb Lab: Verify Extension**; look for the notification and the fixture's
   `.verb-lab/extension.json`. One simple extension does not prove universal API parity.
2. Open `src/greet.cjs`, add a comment and save. Confirm disk/Git show that edit.
3. Create an integrated terminal. Run `node22 tools/build.cjs`,
   `node22 --test test/app.test.cjs`, and `python3 -m unittest discover -s python`.
   `npm22` is also available; no package installation is needed for this fixture.
4. F5 launches **Verb Lab: debug calculation** and pauses at entry. Step over,
   inspect locals/call stack, set a breakpoint at line 3, restart and continue to it.
   The probe records actual debug-adapter events in `.verb-lab/debug.json`; its
   latest-session file is overwritten by a later debug session. Capture each proof
   you need before restarting. The first run also retained browser screenshots.
5. In `test/app.test.cjs`, find the imported `greet`, close Find and use F12. Check
   that it navigates to `src/greet.cjs`. F2 renames the function while preserving
   the exported `greet` property. Save, inspect the actual edit, and rerun the JS test.
6. Commit only the owned source edit. Switch to `conflict-left` and merge
   `conflict-right`. Resolve only `config/theme.txt`; inspect staged paths. Complete
   the merge, then verify no unmerged paths and two actual commit parents. An editor
   showing no conflict markers is insufficient proof of a completed Git merge.
7. Run `node22 src/server.cjs` in the terminal. Open the receipt's `previewOrigin` and
   click **Test application**, confirming the live API returns “Hello, Verb!”. This
   fixed trial port forwarding is not a production arbitrary-port proxy.
8. Set a distinctive shell variable and record its PID before a browser refresh.
   Check terminal/app continuity and the same live shell PID afterwards. Do not call
   retained output alone a successful process reattachment.
9. Capture resource samples at stated points. `docker stats` is container memory,
   not total browser+VM+IDE resource use; HTTP 200 is not a fully interactive UI time.
10. Stop both candidates and their proxies. Preserve synthetic work and receipts.
    Do not use broad kill, Docker prune, or cleanup of unrelated workspaces.

Source control may offer extra provider integrations. Do not sign in or publish the
fixture. Do not attach these trials to public Verb or expose them to the LAN.

## Cleanup and limits

`stop` is idempotent for its own receipt and leaves data in stopped containers.
A startup failure after the IDE was started attempts the same guarded stop.
Containers, their internal network, trial images and fixture data are retained for
review; no background agent or monitoring automation is installed. Removing the
retained containers would remove unsaved container-layer work, so export needed
fixture changes first and act only on exact receipt identities. Never prune Docker.

No production code, host authorization contract, extension containment, safe editor
write protocol, physical phone UI or phone resource budget is established here.
Those belong to subsequent implementation and acceptance work in Verb; Node-owned
boundary dependencies must remain deferred while that repository is out of scope.
