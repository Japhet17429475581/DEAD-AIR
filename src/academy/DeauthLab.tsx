import {useState} from 'react';
import {Panel} from '../ui';
import {deauthCases,initialStation,receiveTeachingFrame,stateLabels,teachingFrames,type StationState} from './deauthModel';

export function DeauthLab({onComplete}:{onComplete:()=>void}){
  const [caseIndex,setCaseIndex]=useState(0);const current=deauthCases[caseIndex];
  const [station,setStation]=useState(()=>initialStation(current.pmf));
  const [frameId,setFrameId]=useState('open-deauth');const [prediction,setPrediction]=useState<StationState|''>('');
  const [solved,setSolved]=useState(false);const [finished,setFinished]=useState(false);
  const [feedback,setFeedback]=useState('');const [journal,setJournal]=useState<string[]>([]);
  const frame=teachingFrames.find(item=>item.id===frameId)!;
  function simulate(){
    const result=receiveTeachingFrame(station,frame);setStation(result.station);
    setJournal(old=>[...old.slice(-15),`[SIMULATION] RX ${frame.kind.toUpperCase()} / Protected=${Number(frame.protected)} / ${result.accepted?'ACCEPTED':'REJECTED'} → ${result.station.state.toUpperCase()}`]);
    const correct=frame.id===current.frame&&prediction===current.expected&&result.station.state===current.expected&&result.accepted===current.accepted;
    setSolved(correct);setFeedback(`${correct?'Кейс разобран верно. ':'Условия задания или прогноз не совпали. '}${result.explanation}`);
  }
  function reset(){setStation(initialStation(current.pmf));setSolved(false);setFeedback('');setFinished(false);}
  function next(){const n=caseIndex+1;setCaseIndex(n);setStation(initialStation(deauthCases[n].pmf));setFrameId('open-deauth');setPrediction('');setSolved(false);setFeedback('');}
  return <Panel title="03 / DEAUTH STATE LAB" meta="SIMULATION · NO RADIO I/O">
    <div className="academy-lab-banner">Только учебная модель состояний. Кадры не создаются для адаптера и не передаются в эфир; live capture, daemon и Hashcat не вызываются.</div>
    <div className="lab-step-count">КЕЙС {caseIndex+1} / {deauthCases.length} · {current.title}</div>
    <p className="body-copy">{current.task}</p>
    <div className="deauth-state-flow" aria-label="Учебная схема состояний станции">{(['unauthenticated','authenticated','associated'] as const).map((id,i)=><div key={id} className={station.state===id?'state-active':''} aria-current={station.state===id?'step':undefined}><small>0{i+1}</small><strong>{id.toUpperCase()}</strong><span>{id==='unauthenticated'?'Нет authentication':id==='authenticated'?'Authentication без association':'Association установлена'}</span></div>)}</div>
    <dl className="key-values"><dt>Станция в модели</dt><dd data-testid="deauth-station-state">{stateLabels[station.state]}</dd><dt>PMF согласован</dt><dd>{station.pmfNegotiated?'ДА':'НЕТ'}</dd><dt>Ключи установлены</dt><dd>{station.keysInstalled?'ДА':'НЕТ'}</dd><dt>Адреса из кейса</dt><dd>AP 02:DA:00:00:01:01 / STA 02:DA:00:00:02:02</dd></dl>
    <label className="field-label">Учебный management frame<select aria-label="Учебный management frame" value={frameId} disabled={solved} onChange={e=>setFrameId(e.target.value)}>{teachingFrames.map(item=><option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
    <pre className="lab-evidence">{`SIMULATION ONLY\nType: Management / subtype ${frame.kind==='deauth'?12:10}\nProtected: ${Number(frame.protected)}\nReceiver integrity result: ${frame.integrity} (условие кейса, не криптопроверка)\nReason code: ${frame.protected?'скрыт для пассивного наблюдателя без ключей':frame.reason}\nSource address authenticity: ${frame.integrity==='valid'?'подтверждена получателем только в этой модели':'не доказана адресным полем'}`}</pre>
    <label className="field-label">Прогноз состояния после приёма<select aria-label="Прогноз состояния" value={prediction} disabled={solved} onChange={e=>setPrediction(e.target.value as StationState)}><option value="">Выбери состояние</option>{Object.entries(stateLabels).map(([id,label])=><option key={id} value={id}>{label}</option>)}</select></label>
    <div className="button-row"><button className="primary" disabled={!prediction||solved} onClick={simulate}>СМОДЕЛИРОВАТЬ ПРИЁМ КАДРА</button><button disabled={finished} onClick={reset}>ВОССТАНОВИТЬ УЧЕБНУЮ СЕССИЮ</button>{solved&&caseIndex<deauthCases.length-1&&<button className="primary" onClick={next}>СЛЕДУЮЩИЙ КЕЙС →</button>}</div>
    {feedback&&<p role="status" className={solved?'answer-correct':'answer-wrong'}>{feedback}</p>}
    {solved&&caseIndex===deauthCases.length-1&&!finished&&<div className="lab-completed"><p>Последний шаг: обычное учебное переподключение. Реальный клиент может не переподключиться; настоящий handshake здесь не захватывается.</p><button className="primary" onClick={()=>{setStation(initialStation(true));setFinished(true);setJournal(old=>[...old,'[SIMULATION] Обычное подключение и установка ключей заданы моделью. Реальных EAPOL-кадров нет.']);onComplete();}}>СМОДЕЛИРОВАТЬ ОБЫЧНОЕ ПЕРЕПОДКЛЮЧЕНИЕ</button></div>}
    {finished&&<div className="lab-completed"><strong>ЛАБОРАТОРИЯ DEAUTH ПРОЙДЕНА</strong><p>Разобраны disassociation, deauth без PMF, отказ неподтверждённому кадру при PMF и подлинное завершение. Все события остались в учебной модели.</p></div>}
    <ol className="lab-journal" aria-label="Учебные события deauth">{journal.map((line,i)=><li key={i}>{line}</li>)}</ol>
  </Panel>;
}
