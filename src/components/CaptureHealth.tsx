import type {Adapter,Capture} from '../domain';
import {Panel} from '../ui';

const titles:Record<string,string>={
  RECEPTION_UNCONFIRMED:'RECEPTION NOT CONFIRMED',
  NO_TARGET_FRAMES:'NO FRAMES FROM THIS ACCESS POINT',
  NO_MATCHING_TARGET_FRAMES:'FILE RECORDS PRESENT / NO MATCHING TARGET FRAMES',
  TARGET_OBSERVED:'ACCESS POINT FRAMES RECEIVED',
  EAPOL_OBSERVED:'EAPOL-KEY FRAMES OBSERVED',
  HELPER_REPORTS_STALE:'HELPER REPORTS ARE STALE',
  NO_HELPER_TELEMETRY:'NO HELPER RECEIVE-HEALTH REPORT',
  CAPTURE_FILE_NOT_READY:'NO READABLE CAPTURE FILE',
  PCAP_RECORDS_NOT_READ:'CAPTURE FILE DATA / NO COMPLETE RECORDS READ',
  RESTORING:'STOPPING COLLECTOR / RESTORING ADAPTER',
};
const seconds=(n:number)=>`${Math.floor(n/60).toString().padStart(2,'0')}:${(n%60).toString().padStart(2,'0')}`;
export function CaptureHealth({capture,demo,adapter}:{capture:Capture;demo:boolean;adapter?:Adapter}){
  if(demo||(!capture.running&&!capture.progress))return null;
  const p=capture.progress;
  const warning=p&&['NO_TARGET_FRAMES','NO_MATCHING_TARGET_FRAMES','HELPER_REPORTS_STALE','NO_HELPER_TELEMETRY','CAPTURE_FILE_NOT_READY','PCAP_RECORDS_NOT_READ'].includes(p.state);
  return <Panel title="RECEIVER // LIVE EVIDENCE" meta={capture.running?'OBSERVING':'STOPPED'} className="capture-health">
    {!p?<p>Waiting for the helper to finish preflight and report receiver evidence. Capture reception has not been confirmed.</p>:<>
      <h3 className={warning?'danger-text':''}>{titles[p.state]||p.state}</h3>
      <p className="body-copy">{p.note}</p>
      <dl className="key-values">
        <dt>Observation elapsed</dt><dd data-testid="capture-elapsed">{seconds(p.elapsedSeconds)}</dd>
        <dt>Time to capture limit</dt><dd>{capture.running?seconds(p.remainingSeconds):'Observation ended'}</dd>
        <dt>Actual radio frequency</dt><dd>{p.actualFrequency===null?'Not reported':`${p.actualFrequency} MHz`}</dd>
        <dt>Raw PCAP records read</dt><dd>{p.recordsRead??'Not reported'}</dd>
        <dt>PCAP file size</dt><dd>{p.pcapBytes===null?'No file reported':`${p.pcapBytes} bytes`}</dd>
        <dt>Accepted target frames</dt><dd>{capture.count}</dd>
        <dt>Target beacons</dt><dd>{p.beacons}</dd>
        <dt>EAPOL-Key frames</dt><dd>{p.eapolFrames}</dd>
        <dt>Helper report age</dt><dd>{p.heartbeatAgeSeconds===null?'No report':`${p.heartbeatAgeSeconds}s`}</dd>
        <dt>Running helper version</dt><dd>{p.helperVersion||'Not reported — update the installed helper'}</dd>
      </dl>
      <progress aria-label="Capture time limit" max={p.limitSeconds} value={Math.min(p.elapsedSeconds,p.limitSeconds)} />
      {capture.stopReason&&<p className="console-note">Stopped: {capture.stopReason}. {capture.exchanges.some(e=>e.complete)?'A correlated sequence was observed; it is not cryptographic verification.':'A complete EAPOL exchange was not observed.'}</p>}
      {capture.running&&p.remainingSeconds===0&&<p className="console-note">Capture limit reached. Waiting for collector shutdown and adapter restoration; this is not continued handshake collection.</p>}
      <p className="muted small">Maximum observation: {seconds(p.limitSeconds)}, followed by adapter restoration. Timers and health reports never add packets. If your phone has reconnected and target frames are still zero, inspect reception rather than waiting for authentication.</p>
      <details><summary>DIAGNOSTIC TEXT · CLICK FIELD, THEN CTRL+C</summary><textarea aria-label="Capture diagnostic summary" readOnly rows={12} onFocus={event=>event.currentTarget.select()} value={JSON.stringify({application:'DEAD//AIR',interface:adapter?.name||'not reported',driver:adapter?.driver||'not reported',inventoryMode:adapter?.mode||'not reported',running:capture.running,status:capture.status,targetFrames:capture.count,stopReason:capture.stopReason||null,...p},null,2)} /></details>
    </>}
  </Panel>;
}
