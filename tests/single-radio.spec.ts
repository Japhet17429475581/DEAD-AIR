import {expect,test} from '@playwright/test';
// IPC contract fixture, not a claim of hardware/driver validation.
test('connected built-in radio requires explicit disconnect consent and passes it to capture',async({page})=>{
  await page.addInitScript(()=>{
    const ap={ssid:'OWNED-FIXTURE',bssid:'02:da:00:00:01:01',channel:6,frequency:2437,quality:75,security:'WPA2-Personal',cipher:'CCMP',pmf:'Unknown'};
    Object.assign(window,{captureRequests:[],discoveryRequests:0,__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string,args:any={})=>{
      if(cmd==='system_check')return {tools:[],errors:[],adapters:[{name:'wlan0',driver:'builtin-fixture',phy:'phy0',mode:'managed',frequency:2437,connected:true,bands:['2.4 GHz'],monitorSupported:true}]};
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:'all'};
      if(cmd==='session_start')return {id:'fixture',policy:args.policy,state:'ACTIVE',path:null,files:[]};
      if(cmd==='discover'){(window as any).discoveryRequests++;return {networks:[ap],adapter:'wlan0',driver:'builtin-fixture',source:'explicit test fixture'};}
      if(cmd==='capture_start'){(window as any).captureRequests.push(args.request);return {running:false,status:'TEST IPC ONLY',packets:[],count:0,dropped:0,error:null,exchanges:[],pmkids:[]};}
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected IPC command ${cmd}`;
    }}});
  });
  await page.goto('/');await page.getByRole('button',{name:/START SESSION/}).click();
  await page.getByRole('button',{name:/DISCOVER NETWORKS/}).click();
  await expect(page.getByRole('button',{name:/OWNED-FIXTURE/})).toBeVisible();
  await page.getByRole('button',{name:/PASSIVE LAB/}).click();
  await page.getByLabel('I own this exact AP and have consent from the participating clients.').check();
  const start=page.getByRole('button',{name:/START OBSERVATION/});await expect(start).toBeDisabled();
  await page.getByLabel(/I allow temporary Wi-Fi disconnection/).check();await expect(start).toBeEnabled();await start.click();
  expect(await page.evaluate(()=>(window as any).captureRequests)).toEqual([{adapter:'wlan0',bssid:'02:da:00:00:01:01',frequency:2437,authorized:true,allowDisconnect:true}]);
  await page.getByRole('button',{name:/ACADEMY/}).click();await page.locator('h1').click();await page.keyboard.press('r');
  await page.evaluate(()=>new Promise(requestAnimationFrame));
  expect(await page.evaluate(()=>(window as any).discoveryRequests)).toBe(1);
});
