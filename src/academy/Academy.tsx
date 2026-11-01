import {useEffect,useRef,useState} from 'react';
import {isDesktop,native} from '../bridge';
import {Panel} from '../ui';
import type {Page} from '../domain';
import {lessons,sources,emptyProgress,canEnterLab,readProgress,type Lesson,type ProgressMap,type Progress} from './course';
import {CANDIDATES,VECTOR,EXPECTED_PMKID,derivePmkid} from './pmkidLab';
import {DeauthLab} from './DeauthLab';
import './academy.css';

export function useAcademy(){
  const [progress,setProgress]=useState<ProgressMap>({});const [selected,setSelected]=useState('scope');
  const mark=(id:string,field:keyof Progress)=>setProgress(old=>({...old,[id]:{...(old[id]||emptyProgress()),[field]:true}}));
  return {progress,selected,setSelected,mark,importProgress:setProgress,reset:()=>setProgress({})};
}
export type AcademyState=ReturnType<typeof useAcademy>;
const contexts:Partial<Record<Page,string>>={dashboard:'scope',discover:'discovery',inspector:'sae',spectrum:'discovery',capture:'eapol',adapter:'radio',sessions:'storage',lab:'compute',settings:'systems'};
export function LearningShelf({page,onOpen}:{page:Page;onOpen:(id:string)=>void}){
  const id=contexts[page];if(!id)return null;const lesson=lessons.find(l=>l.id===id)!;
  return <div className="learning-shelf"><span>◇ УЧЕБНЫЙ РЕЖИМ</span><p>{lesson.summary}</p><button onClick={()=>onOpen(id)}>ТЕОРИЯ ЭТОГО ЭТАПА ↗</button></div>;
}
function Quiz({lesson,onPass}:{lesson:Lesson;onPass:()=>void}){
  const [answers,setAnswers]=useState<Record<number,number>>({});const [graded,setGraded]=useState(false);
  const score=lesson.quiz.filter((q,i)=>q.answer===answers[i]).length;
  return <Panel title="02 / ПРОВЕРКА ПОНИМАНИЯ" meta={`${lesson.quiz.length} ВОПРОСА`}>
    {lesson.quiz.map((q,i)=><fieldset className="academy-question" key={q.prompt}><legend>{i+1}. {q.prompt}</legend>{q.options.map((option,n)=><label key={option}><input type="radio" name={`quiz-${lesson.id}-${i}`} checked={answers[i]===n} onChange={()=>{setAnswers({...answers,[i]:n});setGraded(false);}}/>{option}</label>)}{graded&&<p className={answers[i]===q.answer?'answer-correct':'answer-wrong'}>{answers[i]===q.answer?'ВЕРНО. ':'ПЕРЕСМОТРИ РЕШЕНИЕ. '}{q.explanation}</p>}</fieldset>)}
    <div className="button-row"><button className="primary" disabled={Object.keys(answers).length<lesson.quiz.length} onClick={()=>{setGraded(true);if(score===lesson.quiz.length)onPass();}}>ПРОВЕРИТЬ ТЕСТ</button><span role="status">{graded?`${score} / ${lesson.quiz.length}${score===lesson.quiz.length?' · ЛАБОРАТОРИЯ ОТКРЫТА':' · можно повторить'}`:'Ответь на все вопросы'}</span></div>
  </Panel>;
}
function CaseLab({lesson,onComplete}:{lesson:Lesson;onComplete:()=>void}){
  const [step,setStep]=useState(0);const [feedback,setFeedback]=useState('');const [journal,setJournal]=useState<string[]>([]);
  const current=lesson.lab[step];
  return <Panel title="03 / ПОШАГОВАЯ ЛАБОРАТОРИЯ" meta="SIMULATION · NO RADIO I/O">
    <div className="academy-lab-banner">Изолированный учебный сценарий. Сеть, кадры и события ниже — заданные условия, а не LIVE-наблюдения.</div>
    {current?<><div className="lab-step-count">ШАГ {step+1} / {lesson.lab.length}</div><pre className="lab-evidence">{current.evidence}</pre><h3 className="lab-prompt">{current.prompt}</h3><div className="lab-choices">{current.options.map((option,i)=><button key={option} onClick={()=>{if(i===current.answer){setJournal([...journal,`[${step+1}] ${option} — ${current.explanation}`]);setStep(step+1);setFeedback('');if(step+1===lesson.lab.length)onComplete();}else{setFeedback(`Решение не подходит. ${current.explanation}`);}}}>{option}</button>)}</div></>:<div className="lab-completed"><strong>КЕЙС ПРОЙДЕН</strong><p>Все решения подтверждены условиями сценария. Это учебный результат, не отчёт о реальной сети.</p><button onClick={()=>{setStep(0);setJournal([]);setFeedback('');}}>ПОВТОРИТЬ СЦЕНАРИЙ</button></div>}
    {feedback&&<p className="answer-wrong" role="status">{feedback}</p>}{journal.length>0&&<ol className="lab-journal">{journal.map(item=><li key={item}>{item}</li>)}</ol>}
  </Panel>;
}
function PmkidLab({onComplete}:{onComplete:()=>void}){
  const [variant,setVariant]=useState('eapol');const [wrongSsid,setWrongSsid]=useState(false);const [swap,setSwap]=useState(false);const [includeAnswer,setIncludeAnswer]=useState(true);
  const [running,setRunning]=useState(false);const [lines,setLines]=useState<string[]>([]);const [result,setResult]=useState('');
  const alive=useRef(true);useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  async function run(){setRunning(true);setLines([]);setResult('');try{let matched=false;for(const candidate of CANDIDATES){if(!includeAnswer&&candidate==='LabOnly-2003!')continue;const value=await derivePmkid(candidate,wrongSsid?'OTHER-ACADEMY':VECTOR.ssid,swap);if(!alive.current)return;const good=value===EXPECTED_PMKID;setLines(old=>[...old,`${value}  ${good?'MATCH':'NO MATCH'}`]);if(good){matched=true;setResult(`Совпадение с известным учебным вектором: ${candidate}. Пароль реальной AP не проверялся.`);onComplete();break;}}if(!matched&&alive.current)setResult('Среди выбранных кандидатов совпадение не найдено. Проверь SSID, порядок адресов и состав учебного словаря; это не доказательство стойкости секрета.');}catch(e){if(alive.current)setResult(String(e));}finally{if(alive.current)setRunning(false);}}
  const descriptions:Record<string,string>={eapol:'Кейс: доступный PMKID KDE в key data M1 от AP. Условия синтетические; расчёт кандидатов настоящий.',rsn:'Кейс: PMKID из RSN association/reassociation request клиента. Это кэшированный идентификатор, а не универсальная client-less атака.',active:'Кейс: активное client-less взаимодействие с AP. Здесь показывается только модель получения проверочного материала; кадры не передаются, реальная AP не вызывается.'};
  return <Panel title="03 / PMKID CRYPTO SANDBOX" meta="BUILT-IN VECTOR · REAL WEBCRYPTO">
    <div className="academy-lab-banner">Только встроенный лабораторный вектор. Импорт захваченных хешей и работа с реальными AP отсутствуют.</div>
    <label className="field-label">Вариант сценария<select aria-label="PMKID scenario" value={variant} disabled={running} onChange={e=>setVariant(e.target.value)}><option value="eapol">Пассивный EAPOL PMKID KDE</option><option value="rsn">RSN association / reassociation</option><option value="active">Client-less / активный сценарий — модель</option></select></label><p className="body-copy">{descriptions[variant]}</p>
    <dl className="key-values"><dt>Учебный SSID</dt><dd>{VECTOR.ssid}</dd><dt>AP / STA</dt><dd>{VECTOR.ap} / {VECTOR.station}</dd><dt>PMKID reference</dt><dd className="break">{EXPECTED_PMKID}</dd><dt>Алгоритм</dt><dd>PBKDF2-HMAC-SHA1 → PMK → HMAC-SHA1 → 16 bytes</dd></dl>
    <label className="checkbox"><input type="checkbox" checked={wrongSsid} disabled={running} onChange={e=>setWrongSsid(e.target.checked)}/>Подставить неправильный SSID и исследовать результат</label>
    <label className="checkbox"><input type="checkbox" checked={swap} disabled={running} onChange={e=>setSwap(e.target.checked)}/>Переставить AP/STA местами</label>
    <label className="checkbox"><input type="checkbox" checked={includeAnswer} disabled={running} onChange={e=>setIncludeAnswer(e.target.checked)}/>Включить правильный кандидат во встроенный учебный словарь</label>
    <div className="button-row"><button className="primary" disabled={running} onClick={()=>void run()}>{running?'ВЫЧИСЛЯЕТСЯ…':'ПРОВЕРИТЬ УЧЕБНЫЕ КАНДИДАТЫ'}</button></div>
    <pre className="lab-evidence">{lines.join('\n')||'Вычисления ещё не запускались. Числа скорости и успешные результаты не генерируются таймером.'}</pre>{result&&<p className="body-copy" role="status">{result}</p>}
  </Panel>;
}
export function Academy({state,onReturn}:{state:AcademyState;onReturn:(page:Page)=>void}){
  const [tab,setTab]=useState<'theory'|'quiz'|'lab'>('theory');const [filter,setFilter]=useState('');const [error,setError]=useState('');
  const lesson=lessons.find(l=>l.id===state.selected)||lessons[0];const progress=state.progress[lesson.id]||emptyProgress();
  const completed=lessons.filter(l=>state.progress[l.id]?.lab).length;
  useEffect(()=>{setTab('theory');setError('');},[lesson.id]);
  async function open(url:string){try{if(isDesktop())await native('open_reference',{url});else window.open(url,'_blank','noopener,noreferrer');}catch(e){setError(String(e));}}
  function exportProgress(){const blob=new Blob([JSON.stringify({course:'DEADAIR-ACADEMY-1',lessons:state.progress},null,2)],{type:'application/json'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download='deadair-learning-progress.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
  return <div className="academy"><div className="academy-intro"><div><span className="tiny">DEAD//AIR / LEARNING PATH</span><h2>ТЕОРИЯ. ПРОВЕРКА. ЛАБОРАТОРИЯ.</h2><p>Разбирай каждый процесс, проверяй понимание и решай учебные кейсы. Симуляции отделены от реального радиоэфира.</p></div><div className="academy-total"><b>{completed.toString().padStart(2,'0')} / {lessons.length}</b><span>ЛАБОРАТОРИЙ ПРОЙДЕНО</span><progress max={lessons.length} value={completed}/></div></div>
    <div className="academy-tools"><input aria-label="Поиск учебных модулей" placeholder="Найти тему: PMKID, monitor, PANIC…" value={filter} onChange={e=>setFilter(e.target.value)}/><button onClick={exportProgress}>EXPORT PROGRESS</button><label className="progress-import">IMPORT PROGRESS<input type="file" aria-label="Import learning progress" accept="application/json,.json" onChange={e=>{const file=e.target.files?.[0];if(!file)return;if(file.size>65536){setError('Progress file is too large');return;}void file.text().then(text=>{state.importProgress(readProgress(text));setError('');}).catch(err=>setError(String(err)));e.currentTarget.value='';}}/></label></div>
    <p className="muted small">Прогресс хранится в памяти. Экспорт создаётся только по нажатию; сохранённый отдельно файл PANIC не удаляет. WifiForge приведён как внешний проект для виртуальных сетей; эти лаборатории — собственные кейсы DEAD//AIR.</p>
    {error&&<p className="error-inline" role="alert">{error}</p>}
    <div className="academy-layout"><aside className="academy-roadmap" aria-label="Учебная дорожная карта">{lessons.filter(l=>(l.title+' '+l.summary).toLocaleLowerCase().includes(filter.toLocaleLowerCase())).map(l=>{const p=state.progress[l.id]||emptyProgress();return <button key={l.id} className={lesson.id===l.id?'selected':''} onClick={()=>state.setSelected(l.id)} aria-current={lesson.id===l.id?'step':undefined}><small>{l.level} / {l.minutes} МИН</small><strong>{l.title}</strong><span><i className={p.read?'done':''}>ТЕОРИЯ</i><i className={p.quiz?'done':''}>ТЕСТ</i><i className={p.lab?'done':''}>ЛАБА</i></span></button>;})}</aside>
    <section className="academy-content"><Panel title={lesson.title} meta={lesson.level}><p className="body-copy">{lesson.summary}</p><div className="academy-tabs"><button className={tab==='theory'?'active':''} onClick={()=>setTab('theory')}>01 ТЕОРИЯ {progress.read?'✓':''}</button><button className={tab==='quiz'?'active':''} disabled={!progress.read} onClick={()=>setTab('quiz')}>02 ТЕСТ {progress.quiz?'✓':''}</button><button className={tab==='lab'?'active':''} disabled={!canEnterLab(progress)} onClick={()=>setTab('lab')}>03 ЛАБОРАТОРИЯ {progress.lab?'✓':''}</button></div></Panel>
      {tab==='theory'&&<Panel title="01 / РАЗБОР ПРОЦЕССА"><article className="academy-theory">{lesson.theory.map(block=><section key={block.title}><h3>{block.title}</h3><p>{block.text}</p></section>)}</article><div className="button-row"><button className="primary" onClick={()=>{state.mark(lesson.id,'read');setTab('quiz');}}>ТЕОРИЮ РАЗОБРАЛ → К ТЕСТУ</button></div></Panel>}
      {tab==='quiz'&&<Quiz key={lesson.id} lesson={lesson} onPass={()=>state.mark(lesson.id,'quiz')}/>}
      {tab==='lab'&&canEnterLab(progress)&&(lesson.id==='pmkid'?<PmkidLab key={lesson.id} onComplete={()=>state.mark(lesson.id,'lab')}/>:lesson.id==='deauth'?<DeauthLab key={lesson.id} onComplete={()=>state.mark(lesson.id,'lab')}/>:<CaseLab key={lesson.id} lesson={lesson} onComplete={()=>state.mark(lesson.id,'lab')}/>)}
      <Panel title="ПЕРВОИСТОЧНИКИ / МАНУАЛЫ"><div className="academy-sources">{lesson.sources.map(id=><a key={id} href={sources[id].url} target="_blank" rel="noreferrer noopener" onClick={e=>{e.preventDefault();void open(sources[id].url);}}>{sources[id].title}<span>↗</span></a>)}</div><p className="muted small">Материал — авторское объяснение на русском; ссылки ведут к документации разработчиков и первоисточникам. Исторические публикации не заменяют актуальную документацию инструментов.</p></Panel>
      {progress.lab&&<div className="academy-next"><span>МОДУЛЬ ПРОЙДЕН ✓</span><button onClick={()=>{const next=lessons[lessons.indexOf(lesson)+1];if(next)state.setSelected(next.id);}}>СЛЕДУЮЩАЯ ТЕМА →</button><button onClick={()=>onReturn(lesson.id==='radio'?'adapter':lesson.id==='compute'?'lab':lesson.id==='storage'?'sessions':'capture')}>К ИНСТРУМЕНТАМ АУДИТА ↗</button></div>}
    </section></div>
  </div>;
}
