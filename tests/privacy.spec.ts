import {expect,test} from '@playwright/test';
test('PANIC uses configured scope, captures a new shortcut and shows backend failures',async({page})=>{
  await page.addInitScript(()=>{
    const calls:{cmd:string;args:any}[]=[];
    Object.assign(window,{privacyCalls:calls,__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string,args:any={})=>{
      calls.push({cmd,args});
      if(cmd==='system_check')return {tools:[],adapters:[],errors:[]};
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:'all'};
      if(cmd==='privileged_status')return true;
      if(cmd==='panic_cleanup')throw 'Session is locked by another application; cleanup incomplete';
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected call: ${cmd}`;
    }}});
  });
  await page.goto('/');await page.getByRole('button',{name:/SETTINGS/}).click();
  await page.getByLabel('Erase ALL DEAD//AIR sessions', {exact:false}).uncheck();
  await page.getByLabel('Overwrite regular session files',{exact:false}).uncheck();
  const shortcut=page.getByRole('textbox',{name:'PANIC shortcut'});
  await shortcut.focus();await shortcut.press('Control+Shift+P');
  await expect(shortcut).toHaveValue('Ctrl + Shift + P');
  await page.getByRole('textbox',{name:/API token/}).fill('temporary-token');
  await page.keyboard.press('Control+Shift+P');
  await expect(page.getByRole('dialog',{name:'PANIC cleanup'})).toBeVisible();
  await expect(page.getByText('PANIC // CLEANUP INCOMPLETE')).toBeVisible();
  await expect(page.getByText('Session is locked by another application; cleanup incomplete')).toBeVisible();
  const calls=await page.evaluate(()=>(window as any).privacyCalls.filter((c:any)=>c.cmd==='panic_cleanup'));
  expect(calls).toHaveLength(1);expect(calls[0].args).toEqual({allSessions:false,shred:false});
  await page.getByRole('button',{name:'SHOW RECOVERY CONTROLS'}).click();
  await expect(page.getByRole('textbox',{name:/API token/})).toHaveValue('');
});
test('browser PANIC is explicitly a preview and never pretends to erase files',async({page})=>{
  await page.goto('/');await page.getByRole('button',{name:'PANIC · ERASE & EXIT'}).click();
  await expect(page.getByRole('dialog',{name:'PANIC cleanup'})).toContainText('No native files were deleted');
});
test('MAC profile IPC receives the selected adapter and real vendor profile',async({page})=>{
  await page.addInitScript(()=>{
    let mac={active:false,adapter:'',profile:'',original:'',assigned:'',current:'',error:null};
    Object.assign(window,{macCalls:[],__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string,args:any={})=>{
      if(cmd==='system_check')return {tools:[],errors:[],adapters:[{name:'wlan1',driver:'fixture',phy:'phy1',mode:'managed',frequency:null,connected:false,bands:[],monitorSupported:true}]};
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:'all'};
      if(cmd==='mac_status')return mac;
      if(cmd==='mac_start'){(window as any).macCalls.push(args);mac={...mac,active:true,adapter:args.adapter,profile:args.profile,original:'02:00:00:00:00:01',assigned:'F0:D1:A9:01:02:03',current:'F0:D1:A9:01:02:03'};return mac;}
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected call ${cmd}`;
    }}});
  });
  await page.goto('/');await page.getByRole('button',{name:/ADAPTERS/}).click();
  await page.getByLabel('MAC adapter',{exact:true}).selectOption('wlan1');
  await page.getByLabel('MAC profile',{exact:true}).selectOption('apple');
  await page.getByRole('button',{name:'APPLY MAC PROFILE'}).click();
  await expect(page.getByText('LEASE ACTIVE',{exact:true})).toBeVisible();
  expect(await page.evaluate(()=>(window as any).macCalls)).toEqual([{adapter:'wlan1',profile:'apple'}]);
});
