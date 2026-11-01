// No user-supplied radio hashes or network identifiers are accepted here.
export const VECTOR={ssid:'DEADAIR-ACADEMY',ap:'02:da:00:00:01:01',station:'02:da:00:00:02:02'} as const;
export const CANDIDATES=['winter2003','dead-air-lab','LabOnly-2003!','correct-horse'] as const;
// Independent reference: Node/OpenSSL pbkdf2Sync + HMAC, committed as a fixed vector.
export const EXPECTED_PMKID='b0a1536cff30987c0f2614bf7d869585';
const encode=new TextEncoder();
function address(value:string):Uint8Array<ArrayBuffer>{return Uint8Array.from(value.split(':').map(v=>parseInt(v,16)));}
export async function derivePmkid(password:string,ssid:string=VECTOR.ssid,swap=false):Promise<string>{
  const key=await crypto.subtle.importKey('raw',encode.encode(password),'PBKDF2',false,['deriveBits']);
  const pmk=await crypto.subtle.deriveBits({name:'PBKDF2',hash:'SHA-1',iterations:4096,salt:encode.encode(ssid)},key,256);
  const hmac=await crypto.subtle.importKey('raw',pmk,{name:'HMAC',hash:'SHA-1'},false,['sign']);
  const label=encode.encode('PMK Name');const data=new Uint8Array(label.length+12);data.set(label);
  data.set(address(swap?VECTOR.station:VECTOR.ap),label.length);data.set(address(swap?VECTOR.ap:VECTOR.station),label.length+6);
  const result=new Uint8Array(await crypto.subtle.sign('HMAC',hmac,data));
  return Array.from(result.slice(0,16),v=>v.toString(16).padStart(2,'0')).join('');
}
