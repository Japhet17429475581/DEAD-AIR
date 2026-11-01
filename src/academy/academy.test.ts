import {describe,it,expect} from 'vitest';
import {lessons,sources,readProgress,canEnterLab} from './course';
import {derivePmkid,EXPECTED_PMKID} from './pmkidLab';
describe('learning progression and curriculum',()=>{
  it('requires theory and a passed quiz before opening the lab',()=>{expect(canEnterLab({read:true,quiz:false,lab:false})).toBe(false);expect(canEnterLab({read:false,quiz:true,lab:false})).toBe(false);expect(canEnterLab({read:true,quiz:true,lab:false})).toBe(true);});
  it('rejects imported impossible progress states',()=>{expect(()=>readProgress(JSON.stringify({course:'DEADAIR-ACADEMY-1',lessons:{scope:{read:false,quiz:false,lab:true}}}))).toThrow();expect(readProgress(JSON.stringify({course:'DEADAIR-ACADEMY-1',lessons:{scope:{read:true,quiz:true,lab:true},unknown:{lab:true}}}))).toEqual({scope:{read:true,quiz:true,lab:true}});});
  it('has complete modules, valid answers and primary references',()=>{expect(lessons).toHaveLength(14);expect(new Set(lessons.map(l=>l.id)).size).toBe(14);for(const lesson of lessons){expect(lesson.theory.length).toBeGreaterThanOrEqual(3);expect(lesson.quiz.length).toBeGreaterThanOrEqual(2);expect(lesson.sources.every(id=>sources[id].url.startsWith('https://'))).toBe(true);if(!['pmkid','deauth'].includes(lesson.id))expect(lesson.lab.length).toBeGreaterThanOrEqual(2);for(const question of [...lesson.quiz,...lesson.lab])expect(question.options[question.answer]).toBeTruthy();}});
});
describe('real PMKID educational vector',()=>{
  it('matches the independent fixed PBKDF2/HMAC reference',async()=>{expect(await derivePmkid('LabOnly-2003!')).toBe(EXPECTED_PMKID);});
  it('does not accept wrong password, salt or address order',async()=>{for(const result of await Promise.all([derivePmkid('wrong-password'),derivePmkid('LabOnly-2003!','OTHER-ACADEMY'),derivePmkid('LabOnly-2003!','DEADAIR-ACADEMY',true)]))expect(result).not.toBe(EXPECTED_PMKID);});
});
