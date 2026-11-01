/** Offline teaching model only. No networking, timers, IPC or frame encoding. */
export type StationState='associated'|'authenticated'|'unauthenticated';
export interface Station {state:StationState;pmfNegotiated:boolean;keysInstalled:boolean}
export interface TeachingFrame {id:string;label:string;kind:'deauth'|'disassoc';protected:boolean;integrity:'valid'|'invalid'|'unknown';reason:number}
export const teachingFrames:TeachingFrame[]=[
  {id:'open-deauth',label:'Deauth · открытый, подлинность неизвестна',kind:'deauth',protected:false,integrity:'unknown',reason:3},
  {id:'open-disassoc',label:'Disassociation · открытый, подлинность неизвестна',kind:'disassoc',protected:false,integrity:'unknown',reason:8},
  {id:'valid-deauth',label:'Deauth · защищённый, проверен получателем в кейсе',kind:'deauth',protected:true,integrity:'valid',reason:3},
  {id:'invalid-deauth',label:'Deauth · Protected=1, проверка целостности не прошла',kind:'deauth',protected:true,integrity:'invalid',reason:3},
];
export const stateLabels:Record<StationState,string>={associated:'ASSOCIATED · соединение установлено',authenticated:'AUTHENTICATED · без association',unauthenticated:'UNAUTHENTICATED · нет authentication/association'};
export const initialStation=(pmfNegotiated:boolean):Station=>({state:'associated',pmfNegotiated,keysInstalled:true});
export function receiveTeachingFrame(station:Station,frame:TeachingFrame):{station:Station;accepted:boolean;explanation:string}{
  if(station.state==='unauthenticated')return {station,accepted:false,explanation:'В модели станция уже отключена. Для нового опыта восстанови учебную сессию.'};
  if(frame.protected&&(!station.pmfNegotiated||!station.keysInstalled||frame.integrity!=='valid'))return {station,accepted:false,explanation:'Нет согласованного PMF/ключевого контекста или не пройдена проверка целостности. Бит Protected не делает кадр достоверным.'};
  if(station.pmfNegotiated&&station.keysInstalled&&!frame.protected)return {station,accepted:false,explanation:'В этом кейсе PMF уже действует: неподтверждённый открытый кадр не разрывает защищённую сессию. Детальные процедуры SA Query здесь не моделируются.'};
  return {station:{...station,state:frame.kind==='deauth'?'unauthenticated':'authenticated',keysInstalled:false},accepted:true,explanation:frame.kind==='deauth'?'Модель завершила authentication и association. Это не доказывает личность отправителя открытого кадра.':'Модель завершила association, сохранив authentication. Рабочие ключи в упрощённой модели освобождены.'};
}
export const deauthCases=[
  {id:'disassoc',title:'Разорвать только association',pmf:false,frame:'open-disassoc',expected:'authenticated' as StationState,accepted:true,task:'Выбери кадр, который завершит association, но оставит станцию аутентифицированной. Предскажи состояние и смоделируй приём.'},
  {id:'unprotected',title:'Deauth без действующего PMF',pmf:false,frame:'open-deauth',expected:'unauthenticated' as StationState,accepted:true,task:'Разбери открытый deauth в сессии без PMF. Приём кадра и подлинность заявленного отправителя не равнозначны.'},
  {id:'pmf',title:'Неподтверждённый deauth при PMF',pmf:true,frame:'open-deauth',expected:'associated' as StationState,accepted:false,task:'PMF согласован, ключи установлены. Смоделируй открытый deauth и предскажи, сохранится ли соединение.'},
  {id:'legitimate',title:'Подлинное защищённое завершение',pmf:true,frame:'valid-deauth',expected:'unauthenticated' as StationState,accepted:true,task:'PMF не запрещает законное завершение. Выбери защищённый deauth с успешной проверкой целостности, заданной кейсом.'},
];
