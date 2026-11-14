#!/usr/bin/env python3
"""Exercise an unpacked native release with fresh state; never use owner credentials or projects."""
import argparse
import base64
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--version', required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='verb-release-') as temporary:
        root = Path(temporary)
        home, project = root / 'home', root / 'project'
        home.mkdir(mode=0o700)
        project.mkdir()
        env = {'PATH': os.environ['PATH'], 'HOME': str(home), 'LANG': 'C.UTF-8',
               'VERB_STATE_DIR': str(root / 'state'), 'VERB_TOKEN': 'release-fixture-token'}
        version = subprocess.check_output([str(binary), 'version'], env=env, cwd=project, text=True).strip()
        assert version == 'Verb ' + args.version, 'packaged version mismatch'
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        # Pipe-free host logs cannot deadlock this bounded fixture or print bearer URLs.
        host = subprocess.Popen([str(binary), 'web', '--port', str(port)], cwd=project,
                                env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        def request(method, path, body=None):
            payload = None if body is None else json.dumps(body).encode()
            req = urllib.request.Request(f'http://127.0.0.1:{port}' + path, data=payload,
                    method=method, headers={'X-Verb-Token': env['VERB_TOKEN'],
                                             'Content-Type': 'application/json'})
            with urllib.request.urlopen(req, timeout=3) as response:
                return json.load(response)
        try:
            deadline = time.monotonic() + 20
            while True:
                assert host.poll() is None, 'packaged host exited during startup'
                try:
                    state = request('GET', '/api/state')
                    break
                except (OSError, urllib.error.URLError):
                    if time.monotonic() >= deadline:
                        raise AssertionError('packaged host did not become ready')
                    time.sleep(0.1)
            assert state['project']['name'] == 'project'
            started = request('POST', '/api/terminals', {'agent': 'shell', 'isolated': False})
            sid = started['sessionId']
            request('POST', f'/api/terminals/{sid}/input',
                    {'data': "printf 'release-pty-ok\\n'; exit\n"})
            deadline = time.monotonic() + 10
            while True:
                output = request('GET', f'/api/terminals/{sid}/output?after=0')
                if output['running'] is False:
                    break
                assert time.monotonic() < deadline, 'packaged PTY did not exit'
                time.sleep(0.1)
            assert b'release-pty-ok' in base64.b64decode(output['data'])
            state = request('GET', '/api/state')
            row = next(s for s in state['sessions'] if s['id'] == sid)
            assert row['terminal'] == {'running': False, 'replayAvailable': True}
            print('PASS: unpacked release version, fresh workspace, native PTY and exit/replay contract')
        finally:
            host.terminate()
            try:
                host.wait(timeout=5)
            except subprocess.TimeoutExpired:
                host.kill()
                host.wait()


if __name__ == '__main__':
    main()
