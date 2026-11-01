import {describe,it,expect} from 'vitest';
import {bindingFrom,bindingLabel,DEFAULT_PANIC_BINDING,matchesBinding} from './privacyKeys';
const event=(options:Partial<KeyboardEvent>={})=>({code:'F12',ctrlKey:true,shiftKey:true,altKey:false,metaKey:false,repeat:false,isComposing:false,...options}) as KeyboardEvent;
describe('panic binding',()=>{
  it('requires the exact modifiers and suppresses key repeat',()=>{expect(matchesBinding(event(),DEFAULT_PANIC_BINDING)).toBe(true);for(const change of [{shiftKey:false},{altKey:true},{repeat:true},{isComposing:true}])expect(matchesBinding(event(change),DEFAULT_PANIC_BINDING)).toBe(false);});
  it('records a single physical key and rejects modifier-only input',()=>{const binding=bindingFrom(event({code:'KeyP',ctrlKey:false,shiftKey:false}));expect(bindingLabel(binding!)).toBe('P');expect(bindingFrom(event({code:'ControlLeft'}))).toBeNull();});
});
