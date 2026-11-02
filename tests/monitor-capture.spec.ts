import {expect,test} from '@playwright/test';

// IPC contract fixtures: no real radio is accessed by these browser scenarios.
for(const failure of ['preflight','collector'])test(`${failure}: existing monitor selection survives refresh and exact failure stays visible`,async({page})=>{
  await page.addInitScript((failure)=>{
    const base={running:false,status:'IDLE',packets:[],count:0,dropped:0,error:null,exchanges:[],pmkids:[]};
    const detail=failure==='preflight'?'iw dev wlan0mon set freq 5180 failed (exit status: 1): fixture driver busy (-16)':'airodump-ng exited unexpectedly: exit status: 1. fixture radio blocked';
    let inventory=0;
    Object.assign(window,{captureRequests:[],__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string,args:any={})=>{
      if(cmd==='system_check'){inventory++;return {platform:'linux',tools:[],errors:[],adapters:[{name:inventory===1?'wlan0':'wlan0mon',driver:'explicit-fixture',phy:'phy0',mode:inventory===1?'managed':'monitor',frequency:2437,connected:false,bands:['2.4 GHz','5 GHz'],monitorSupported:inventory===1}]};}
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:'all'};
      if(cmd==='session_start')return {id:'fixture',policy:args.policy,state:'ACTIVE',path:null,files:[]};
      if(cmd==='discover')return {networks:[{ssid:'OWNED-FIXTURE',bssid:'02:da:00:00:01:01',channel:36,frequency:5180,quality:75,security:'WPA2-Personal',cipher:'CCMP',pmf:'Unknown'}],adapter:'wlan0',driver:'explicit-fixture',source:'test fixture'};
      if(cmd==='capture_start'){(window as any).captureRequests.push(args.request);if(failure==='preflight')throw detail;return {...base,running:true,status:'AUTHORIZATION / PREFLIGHT'};}
      if(cmd==='capture_status')return {...base,status:'FAILED / CHECK RESTORATION',error:detail};
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected IPC ${cmd}`;
    }}});
  },failure);
  await page.goto('/');await page.getByRole('button',{name:/START SESSION/}).click();
  await page.getByRole('button',{name:/DISCOVER NETWORKS/}).click();await expect(page.getByRole('button',{name:/OWNED-FIXTURE/})).toBeVisible();
  await page.getByRole('button',{name:/PASSIVE LAB/}).click();await page.getByRole('button',{name:'REFRESH CAPTURE ADAPTERS'}).click();
  await expect(page.getByLabel('Capture adapter · built-in or USB')).toHaveValue('wlan0mon');
  await expect(page.getByText(/Existing monitor interface: reused directly/)).toBeVisible();
  await page.getByLabel('I own this exact AP and have consent from the participating clients.').check();
  await page.getByRole('button',{name:/START OBSERVATION/}).click();
  await expect(page.locator('.capture-error')).toContainText(failure==='preflight'?'fixture driver busy (-16)':'fixture radio blocked');
  await expect(page.locator('.capture-error')).toContainText('OBSERVATION FAILED');
  await expect(page.getByRole('button',{name:/START OBSERVATION/})).toBeEnabled();
  expect(await page.evaluate(()=>(window as any).captureRequests)).toEqual([{adapter:'wlan0mon',bssid:'02:da:00:00:01:01',frequency:5180,authorized:true,allowDisconnect:false}]);
});
