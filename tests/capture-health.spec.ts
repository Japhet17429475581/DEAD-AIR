import {expect,test} from '@playwright/test';

// Explicit IPC fixtures. None of these tests claims physical radio reception.
for(const scenario of ['silent','seen','stale','file'])test(`capture health: ${scenario} evidence is visible without invented packets`,async({page})=>{
  await page.addInitScript((scenario)=>{
    const seen=scenario==='seen';
    const state=scenario==='seen'?'TARGET_OBSERVED':scenario==='stale'?'HELPER_REPORTS_STALE':scenario==='file'?'CAPTURE_FILE_NOT_READY':'NO_TARGET_FRAMES';
    const p={state,elapsedSeconds:20,remainingSeconds:280,limitSeconds:300,recordsRead:seen?8:0,pcapBytes:scenario==='file'?null:seen?1500:24,beacons:seen?8:0,eapolFrames:0,actualFrequency:2437,heartbeatAgeSeconds:scenario==='stale'?12:0,helperVersion:'explicit-fixture',note:seen?'Target frames received; no EAPOL-Key frames observed. Reconnect your consenting client normally.':scenario==='stale'?'Current receiver health is unknown.':scenario==='file'?'No readable output file.':'No selected AP frames received; this is unconfirmed reception, not a wait for a client.'};
    const base={running:true,status:state,packets:[],count:seen?8:0,dropped:0,error:null,exchanges:[],pmkids:[],progress:p,stopReason:null};let polls=0;
    Object.assign(window,{healthCalls:[],__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string,args:any={})=>{
      (window as any).healthCalls.push(cmd);
      if(cmd==='system_check')return {platform:'linux',tools:[],errors:[],adapters:[{name:'wlan0',driver:'explicit-fixture',phy:'phy0',mode:'managed',frequency:2437,connected:false,bands:['2.4 GHz'],monitorSupported:true}]};
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:'all'};
      if(cmd==='session_start')return {id:'fixture',policy:args.policy,state:'ACTIVE',path:null,files:[]};
      if(cmd==='discover')return {networks:[{ssid:'OWNED-FIXTURE',bssid:'02:da:00:00:01:01',channel:6,frequency:2437,quality:75,security:'WPA2-Personal',cipher:'CCMP',pmf:'Unknown'}],adapter:'wlan0',driver:'explicit-fixture',source:'test fixture'};
      if(cmd==='capture_start')return base;
      if(cmd==='capture_status'){polls++;return scenario==='silent'&&polls>=2?{...base,running:false,status:'STOPPED / NO TARGET FRAMES',stopReason:'five-minute limit',progress:{...p,elapsedSeconds:300,remainingSeconds:0}}:base;}
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected IPC ${cmd}`;
    }}});
  },scenario);
  await page.goto('/');await page.getByRole('button',{name:/START SESSION/}).click();
  await page.getByRole('button',{name:/DISCOVER NETWORKS/}).click();await expect(page.getByRole('button',{name:/OWNED-FIXTURE/})).toBeVisible();
  await page.getByRole('button',{name:/PASSIVE LAB/}).click();await page.getByLabel('I own this exact AP and have consent from the participating clients.').check();await page.getByRole('button',{name:/START OBSERVATION/}).click();
  const panel=page.locator('.capture-health');
  await expect(panel).toContainText(scenario==='seen'?'ACCESS POINT FRAMES RECEIVED':scenario==='stale'?'HELPER REPORTS ARE STALE':scenario==='file'?'NO READABLE CAPTURE FILE':'NO FRAMES FROM THIS ACCESS POINT');
  await expect(panel).toContainText('2437 MHz');await expect(panel.getByTestId('capture-elapsed')).toHaveText('00:20');
  if(scenario==='seen'){
    await panel.getByText('DIAGNOSTIC TEXT · CLICK FIELD, THEN CTRL+C').click();
    const diagnostic=await panel.getByLabel('Capture diagnostic summary').inputValue();
    expect(JSON.parse(diagnostic).driver).toBe('explicit-fixture');
    expect(diagnostic).not.toContain('02:da:00:00:01:01');
    expect(diagnostic).not.toContain('OWNED-FIXTURE');
    await page.screenshot({path:'artifacts/capture-health-panel.png',fullPage:true});
  }
  await expect(page.getByText('M1–M4 correlated sequence')).toHaveCount(0);
  if(scenario==='silent'){
    await expect(panel).toContainText('Stopped: five-minute limit.');
    await expect(panel).toContainText('A complete EAPOL exchange was not observed.');
    await expect(panel.getByTestId('capture-elapsed')).toHaveText('05:00');
    await expect(page.getByRole('button',{name:/START OBSERVATION/})).toBeEnabled();
    await expect(page.locator('.capture-state')).toHaveText('STOPPED / NO TARGET FRAMES');
  }
});

test('connected built-in MAC profile requires disconnection consent and can be restored',async({page})=>{
  await page.addInitScript(()=>{
    const idle={active:false,adapter:'',profile:'',original:'',assigned:'',current:'',error:null};let mac=idle;
    Object.assign(window,{macRequests:[],__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string,args:any={})=>{
      if(cmd==='system_check')return {tools:[],errors:[],adapters:[{name:'wlan0',driver:'built-in-fixture',phy:'phy0',mode:'managed',frequency:2437,connected:true,bands:[],monitorSupported:true}]};
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:'all'};
      if(cmd==='mac_status')return mac;
      if(cmd==='mac_start'){(window as any).macRequests.push(args);mac={active:true,adapter:'wlan0',profile:'private',original:'02:00:00:00:00:01',assigned:'02:00:00:00:00:02',current:'02:00:00:00:00:02',error:null};return mac;}
      if(cmd==='mac_restore'){mac=idle;return mac;}
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected IPC ${cmd}`;
    }}});
  });
  await page.goto('/');await page.getByRole('button',{name:/ADAPTERS/}).click();
  await page.getByLabel('MAC adapter',{exact:true}).selectOption('wlan0');
  const apply=page.getByRole('button',{name:'APPLY MAC PROFILE'});await expect(apply).toBeDisabled();
  await page.getByLabel(/I allow Wi-Fi disconnection for this MAC lease/).check();await expect(apply).toBeEnabled();await apply.click();
  await expect(page.getByText('LEASE ACTIVE',{exact:true})).toBeVisible();
  expect(await page.evaluate(()=>(window as any).macRequests)).toEqual([{adapter:'wlan0',profile:'private',allowDisconnect:true}]);
  await page.getByRole('button',{name:'RESTORE ORIGINAL MAC'}).click();await expect(apply).toBeEnabled();
});
