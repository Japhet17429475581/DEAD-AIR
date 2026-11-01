#!/usr/bin/env bash
set -euo pipefail
json=false
[[ ${1:-} != --json ]] || json=true
command -v python3 >/dev/null || { echo 'python3 is needed for redacted diagnostics.' >&2; exit 1; }
python3 - "$json" <<'PY'
import json, os, platform, shutil, subprocess, sys
def probe(name, args):
    binary=shutil.which(name)
    if not binary:return {'available':False,'version':None}
    try:
        p=subprocess.run([binary,*args],capture_output=True,text=True,timeout=8,env={**os.environ,'LC_ALL':'C'})
        line=(p.stdout or p.stderr).splitlines()
        return {'available':True,'version':line[0][:180] if line else 'installed','exit':p.returncode}
    except (OSError,subprocess.TimeoutExpired):return {'available':True,'version':'probe failed'}
data={'platform':platform.system(),'kernel':platform.release(),'session':os.environ.get('XDG_SESSION_TYPE','unknown'),'tools':{name:probe(name,args) for name,args in [('nmcli',['--version']),('iw',['--version']),('aircrack-ng',['--help']),('hashcat',['--version']),('pkexec',['--version']),('pkg-config',['--modversion','webkit2gtk-4.1'])]},'capture_helper':os.path.isfile('/usr/libexec/deadair-capture-helper'),'radio_count':len(os.listdir('/sys/class/ieee80211')) if os.path.isdir('/sys/class/ieee80211') else 0,'redacted':True}
if sys.argv[1]=='true':print(json.dumps(data,indent=2))
else:
    print('DEAD//AIR DOCTOR — no SSIDs, MAC addresses, tokens, or hostnames')
    for key,value in data.items():
        if key!='tools':print(f'{key}: {value}')
    for name,value in data['tools'].items():print(f"[{'OK' if value['available'] else 'MISSING'}] {name}: {value['version']}")
    print('Missing tools: use Settings → INSTALL MISSING TOOLS. No radio: attach a Linux-supported Wi-Fi adapter.')
PY
