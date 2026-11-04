#!/usr/bin/env python3
"""Actual kernel/hostapd/supplicant integration; run only in the isolated VM.
This is a test harness, not a Python application backend or simulated result.
"""
import hashlib, json, os, pathlib, select, signal, subprocess, sys, time, uuid

HELPER=sys.argv[1]
assert os.geteuid()==0
assert 'deadair_helper=' in pathlib.Path('/proc/cmdline').read_text(), 'Use the isolated guest harness'

class Lab:
    def __init__(self, broker=None):
        self.token=str(uuid.uuid4()); self.buffer=b''; self.queue=[]; self.last={}; self.packet_count=0
        self.error_path=pathlib.Path('/run')/('lab-test-'+self.token+'.stderr')
        self.error_file=self.error_path.open('wb')
        args=[HELPER,'virtual-lab','wpa2-basics',self.token]
        identity={}
        if broker:
            args=[HELPER,'broker-client',broker['path'],broker['token'],'virtual-lab','wpa2-basics',self.token]
            identity={'user':1000,'group':1000,'extra_groups':[]}
        self.process=subprocess.Popen(args,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.error_file,env={**os.environ,'PKEXEC_UID':'1000'},**identity)
    def receive(self,timeout):
        deadline=time.monotonic()+timeout
        while not self.queue:
            remaining=deadline-time.monotonic()
            if remaining<=0:raise TimeoutError('No lab message; last state '+json.dumps(self.last))
            ready,_,_=select.select([self.process.stdout],[],[],remaining)
            if not ready:continue
            part=os.read(self.process.stdout.fileno(),65536)
            if not part:raise RuntimeError('Lab exited: '+self.error_path.read_text(errors='replace'))
            self.buffer+=part
            while b'\n' in self.buffer:
                line,self.buffer=self.buffer.split(b'\n',1)
                message=json.loads(line)
                if message.get('kind')=='packet': self.packet_count+=1; continue
                if message.get('kind')=='error':raise RuntimeError(message.get('message','helper error')+'\n'+self.error_path.read_text(errors='replace'))
                if message.get('kind')=='lab-state':
                    self.last=message['status']
                    print(json.dumps({'phase':self.last['phase'],'packets':self.last['packetCount'],'eapol':self.last['eapolMessages'],'connected':self.last['clientConnected'],'authorized':self.last['apAuthorized'],'ping':self.last['pingVerified'],'complete':self.last['completed']}),flush=True)
                self.queue.append(message)
        return self.queue.pop(0)
    def wait(self,predicate,timeout=45):
        deadline=time.monotonic()+timeout
        while True:
            message=self.receive(max(.01,deadline-time.monotonic()))
            if predicate(message):return message
            if time.monotonic()>deadline:raise TimeoutError('Lab predicate failed: '+json.dumps(self.last))
    def action(self,action):
        self.process.stdin.write((action+'\n').encode());self.process.stdin.flush()
    def diagnostics(self):
        directory=pathlib.Path('/run/deadair/virtual')/self.token
        if directory.exists():
            for path in directory.rglob('*'):
                if path.is_file() and path.suffix=='.log':print(path.name+':\n'+path.read_text(errors='replace')[-8000:],flush=True)
            for path in directory.glob('capture/*.cap'):
                result=subprocess.run(['tcpdump','-nne','-r',str(path)],capture_output=True,text=True,timeout=8)
                print('Capture diagnostic:\n'+result.stdout[-10000:]+result.stderr,flush=True)

sentinel=None
broker=None
try:
    inventory=subprocess.check_output(['iw','dev'],text=True)
    interface=next(line.strip().split(' ',1)[1] for line in inventory.splitlines() if line.strip().startswith('Interface '))
    sentinel_conf=pathlib.Path('/run/sentinel.conf')
    sentinel_conf.write_text(f'interface={interface}\ndriver=nl80211\nssid=OUTSIDE-SENTINEL\nhw_mode=g\nchannel=6\n')
    sentinel_log=open('/run/sentinel.log','wb')
    sentinel=subprocess.Popen(['hostapd',str(sentinel_conf)],stdout=sentinel_log,stderr=sentinel_log)
    time.sleep(1)
    assert sentinel.poll() is None, pathlib.Path('/run/sentinel.log').read_text()
    before=subprocess.check_output(['iw','dev'],text=True)
    lab=Lab()
    try:
        lab.wait(lambda m:m.get('kind')=='lab-state' and m['status']['apReady'] and not m['status']['busy'])
        lab.action('scan')
        lab.wait(lambda m:m.get('kind')=='lab-state' and m['status']['scanConfirmed'] and not m['status']['busy'])
        assert lab.last['scanSsids']==['DEADAIR-LAB'], 'Foreign virtual medium leaked into scan: '+repr(lab.last['scanSsids'])
        lab.wait(lambda m:m.get('kind')=='lab-state' and m['status']['packetCount']>0)
        lab.action('connect')
        lab.wait(lambda m:m.get('kind')=='lab-state' and m['status']['clientConnected'] and m['status']['apAuthorized'])
        lab.action('verify')
        lab.wait(lambda m:m.get('kind')=='lab-state' and m['status']['completed'])
        assert lab.last['eapolMessages']==[1,2,3,4]
        assert lab.packet_count>=4
        lab.action('stop')
        lab.wait(lambda m:m.get('kind')=='lab-done' and m.get('clean'))
        assert lab.process.wait(timeout=10)==0
        assert not (pathlib.Path('/run/deadair/virtual')/lab.token).exists()
        assert subprocess.check_output(['iw','dev'],text=True)==before, 'Existing radios were modified'
    except BaseException:
        lab.diagnostics();lab.process.kill();lab.process.wait();raise
    # Independent second run tests absence of stale global state, abrupt owner
    # death, auto-destroy radios, and recovery scoped to the same UID.
    crashed=Lab()
    try:
        crashed.wait(lambda m:m.get('kind')=='lab-state' and m['status']['apReady'])
        crashed.process.kill();crashed.process.wait(timeout=10);time.sleep(.5)
        record=pathlib.Path('/run/deadair/virtual')/crashed.token
        assert record.exists()
        result=subprocess.run([HELPER,'virtual-recover'],env={**os.environ,'PKEXEC_UID':'1000'},capture_output=True,text=True,timeout=20)
        assert result.returncode==0, result.stdout+result.stderr
        assert not record.exists()
        assert subprocess.check_output(['iw','dev'],text=True)==before, 'Crash recovery modified existing radios'
    except BaseException:
        crashed.diagnostics()
        if crashed.process.poll() is None:crashed.process.kill();crashed.process.wait()
        raise
    # Exercise the actual installed daemon/client transport with an unprivileged
    # client. Polkit's graphical prompt itself remains a desktop acceptance test.
    assert hashlib.sha256(pathlib.Path(HELPER).read_bytes()).digest()==hashlib.sha256(pathlib.Path('/usr/libexec/deadair-capture-helper').read_bytes()).digest(), 'Install this helper in the disposable container before testing the broker'
    broker_token=str(uuid.uuid4())
    broker=subprocess.Popen([HELPER,'broker',broker_token],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env={**os.environ,'PKEXEC_UID':'1000'})
    ready=json.loads(broker.stdout.readline())
    assert ready.get('kind')=='broker-ready', ready
    leased=Lab({'path':ready['path'],'token':broker_token})
    leased.wait(lambda m:m.get('kind')=='lab-state' and m['status']['apReady'])
    assert pathlib.Path('/proc') .joinpath(str(leased.process.pid),'status').read_text().split('Uid:\t')[1].split()[0]=='1000'
    leased.process.stdin.close()
    leased.wait(lambda m:m.get('kind')=='lab-done' and m.get('clean'))
    assert leased.process.wait(timeout=10)==0
    assert not (pathlib.Path('/run/deadair/virtual')/leased.token).exists()
    broker.stdin.close()
    assert broker.wait(timeout=15)==0
    assert not pathlib.Path(ready['path']).exists()
    assert subprocess.check_output(['iw','dev'],text=True)==before
    print(json.dumps({'real_hwsim':True,'station_scan':True,'foreign_medium_isolated':True,'wpa_completed':True,'ap_authorized':True,'eapol_m1_m4':True,'ip_connectivity':True,'normal_cleanup':True,'crash_recovery':True,'sentinel_preserved':True,'unprivileged_broker_client':True,'broker_eof_cleanup':True}),flush=True)
finally:
    if broker and broker.poll() is None:
        broker.kill();broker.wait()
    if sentinel and sentinel.poll() is None:
        sentinel.terminate()
        try:sentinel.wait(timeout=4)
        except subprocess.TimeoutExpired:sentinel.kill();sentinel.wait()
