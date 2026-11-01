"""Container-only integration test of the real root broker (not polkit UI)."""
import json, os, pathlib, select, subprocess, uuid
assert pathlib.Path('/.dockerenv').exists() and os.geteuid() == 0
helper = '/usr/libexec/deadair-capture-helper'
uid = int(subprocess.check_output(['id', '-u', 'deadair-test']))
token = str(uuid.uuid4())
daemon = subprocess.Popen([helper, 'broker', token], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={**os.environ, 'PKEXEC_UID': str(uid)})
path = None
try:
    assert select.select([daemon.stdout], [], [], 10)[0], 'daemon readiness timeout'
    ready = json.loads(daemon.stdout.readline())
    assert ready['kind'] == 'broker-ready', ready
    path = pathlib.Path(ready['path'])
    assert path.stat().st_mode & 0o777 == 0o600
    assert path.stat().st_uid == uid
    def request(args, access_token=token, user='deadair-test'):
        return subprocess.run(['runuser', '-u', user, '--', helper, 'broker-client', str(path), access_token, *args], input=b'', capture_output=True, timeout=20)
    for _ in range(2):
        result = request(['adapter-report'])
        assert result.returncode == 0, (result.stdout, result.stderr)
        assert daemon.poll() is None
    assert request(['adapter-report'], access_token=str(uuid.uuid4())).returncode != 0
    assert request(['adapter-report'], user='root').returncode != 0
    assert request(['install-dependencies', 'arbitrary-package']).returncode != 0
    assert request(['mac-profile', 'missing0', 'private', str(uuid.uuid4())]).returncode != 0
    daemon.stdin.close()
    assert daemon.wait(timeout=20) == 0, daemon.stderr.read().decode()
    assert not path.exists()
    print(json.dumps({'result': 'PASS', 'broker_pid_reused': ready['pid'], 'real_adapter_reports': 2,
                      'wrong_token_rejected': True, 'wrong_uid_rejected': True,
                      'arbitrary_package_rejected': True, 'missing_adapter_rejected': True,
                      'socket_removed_on_shutdown': True}))
finally:
    if daemon.poll() is None:
        if not daemon.stdin.closed: daemon.stdin.close()
        try: daemon.wait(timeout=20)
        except subprocess.TimeoutExpired: daemon.terminate(); daemon.wait(timeout=10)
