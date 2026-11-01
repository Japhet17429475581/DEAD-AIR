import {describe,it,expect} from 'vitest';
import {deauthCases,initialStation,receiveTeachingFrame,teachingFrames} from './deauthModel';
const frame=(id:string)=>teachingFrames.find(item=>item.id===id)!;
describe('offline deauthentication teaching model',()=>{
  it('distinguishes disassociation from deauthentication',()=>{
    expect(receiveTeachingFrame(initialStation(false),frame('open-disassoc')).station.state).toBe('authenticated');
    expect(receiveTeachingFrame(initialStation(false),frame('open-deauth')).station.state).toBe('unauthenticated');
  });
  it('keeps an established PMF association after unprotected deauth',()=>{
    const before=initialStation(true);const result=receiveTeachingFrame(before,frame('open-deauth'));
    expect(result.accepted).toBe(false);expect(result.station).toEqual(before);
  });
  it('does not mistake Protected=1 for verified integrity',()=>{
    const before=initialStation(true);const result=receiveTeachingFrame(before,frame('invalid-deauth'));
    expect(result.accepted).toBe(false);expect(result.station).toEqual(before);
  });
  it('accepts a legitimate protected teardown only in an applicable context',()=>{
    const valid=frame('valid-deauth');expect(receiveTeachingFrame(initialStation(true),valid).station.state).toBe('unauthenticated');
    expect(receiveTeachingFrame(initialStation(false),valid).accepted).toBe(false);
    expect(receiveTeachingFrame({...initialStation(true),keysInstalled:false},valid).accepted).toBe(false);
  });
  it('repeated teardown does not manufacture a new connection or authentication',()=>{
    const disconnected=receiveTeachingFrame(initialStation(false),frame('open-deauth')).station;
    const repeated=receiveTeachingFrame(disconnected,frame('open-disassoc'));
    expect(repeated.accepted).toBe(false);expect(repeated.station.state).toBe('unauthenticated');
  });
  it('all four cases have internally consistent outcomes',()=>{
    expect(deauthCases).toHaveLength(4);
    for(const task of deauthCases){const result=receiveTeachingFrame(initialStation(task.pmf),frame(task.frame));expect(result.station.state).toBe(task.expected);expect(result.accepted).toBe(task.accepted);}
  });
});
