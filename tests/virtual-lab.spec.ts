import {expect,test,type Page} from '@playwright/test';
async function openLab(page:Page){
  await page.goto('/');await page.getByRole('button',{name:/ACADEMY/}).click();await page.getByRole('button',{name:'REAL VIRTUAL NETWORK LAB ↗'}).click();
  await expect(page.getByRole('button',{name:/03 ЛАБОРАТОРИЯ/})).toBeDisabled();
  await page.getByRole('button',{name:'ТЕОРИЮ РАЗОБРАЛ → К ТЕСТУ'}).click();
  await page.getByLabel('Из Linux, hostapd и wpa_supplicant',{exact:true}).check();await page.getByLabel('Scan, состояния клиента/AP, кадры и IP-связность',{exact:true}).check();
  await page.getByRole('button',{name:'ПРОВЕРИТЬ ТЕСТ'}).click();await page.getByRole('button',{name:/03 ЛАБОРАТОРИЯ/}).click();
}
test('browser exposes the real-lab lesson without pretending it can create networks',async({page})=>{
  await openLab(page);await expect(page.getByRole('button',{name:'CREATE REAL LAB'})).toBeDisabled();
  await expect(page.getByText(/Браузер показывает описание, но не создаёт сети/)).toBeVisible();
  await expect(page.getByText('VIRTUAL LAB VERIFIED ✓',{exact:true})).toHaveCount(0);
});
test('native virtual-lab IPC requires consent and evidence before crediting completion',async({page})=>{
  await page.addInitScript(()=>{
    let status={running:false,busy:false,phase:'NOT STARTED',operationId:'',ssid:'DEADAIR-LAB',bssid:'',apReady:false,scanConfirmed:false,scanSsids:[] as string[],clientStarted:false,clientConnected:false,apAuthorized:false,pingVerified:false,eapolMessages:[] as number[],exchangeCorrelated:false,completed:false,packetCount:0,elapsedSeconds:0,limitSeconds:600,logs:[] as string[],error:null,cleanupComplete:true,packets:[]};
    Object.assign(window,{virtualCalls:[],__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string,args:any={})=>{
      if(cmd==='system_check')return {platform:'linux',tools:[],errors:[],adapters:[]};
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:args.profile||'all'};
      if(cmd==='virtual_lab_check')return {supported:true,platform:'linux',kernelModule:true,moduleLoaded:true,tools:[{name:'hostapd',available:true}],details:['Explicit IPC test fixture; not kernel validation.']};
      if(cmd==='virtual_lab_status')return status;
      if(cmd==='virtual_lab_start'){(window as any).virtualCalls.push({cmd,args});status={...status,running:true,phase:'AP READY / CLIENT IDLE',operationId:'fixture',apReady:true,bssid:'02:da:00:00:01:01',completed:true,cleanupComplete:false};return status;}
      if(cmd==='virtual_lab_action'){(window as any).virtualCalls.push({cmd,args});if(args.action==='scan')status={...status,scanConfirmed:true,scanSsids:['DEADAIR-LAB'],packetCount:1};if(args.action==='connect')status={...status,clientStarted:true,clientConnected:true,apAuthorized:true,eapolMessages:[1,2],packetCount:3};if(args.action==='verify')status={...status,pingVerified:true,exchangeCorrelated:true,eapolMessages:[1,2,3,4],packetCount:5,completed:true,phase:'LAB VERIFIED'};return status;}
      if(cmd==='virtual_lab_stop'){status={...status,running:false,cleanupComplete:true,phase:'STOPPED / OWNED RESOURCES REMOVED'};return status;}
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected IPC ${cmd}`;
    }}});
  });
  await openLab(page);const create=page.getByRole('button',{name:'CREATE REAL LAB'});await expect(create).toBeDisabled();
  await page.getByLabel(/Создать изолированную Linux-лабораторию/).check();await expect(create).toBeEnabled();await create.click();
  await expect(page.getByText('VIRTUAL LAB VERIFIED ✓',{exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'ENTER DEMO',exact:true}).click();await page.getByRole('button',{name:'ENABLE DEMO MODE',exact:true}).click();
  await expect(page.getByRole('alert').filter({hasText:'Stop or recover the real virtual lab before entering DEMO MODE.'})).toBeVisible();
  await page.getByRole('button',{name:'Dismiss error',exact:true}).click();
  await page.getByRole('button',{name:'SCAN VIRTUAL AIR'}).click();await expect(page.getByText('Обнаружено: DEADAIR-LAB',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'CONNECT VIRTUAL CLIENT'}).click();await expect(page.getByText('COMPLETED',{exact:true})).toBeVisible();
  await expect(page.getByText('VIRTUAL LAB VERIFIED ✓',{exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'VERIFY REAL CONNECTIVITY'}).click();await expect(page.getByText('VIRTUAL LAB VERIFIED ✓',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:/15 \/ Настоящая/}).locator('i.done')).toHaveCount(3);
  expect(await page.evaluate(()=>(window as any).virtualCalls)).toEqual([{cmd:'virtual_lab_start',args:{labId:'wpa2-basics',authorized:true}},{cmd:'virtual_lab_action',args:{action:'scan'}},{cmd:'virtual_lab_action',args:{action:'connect'}},{cmd:'virtual_lab_action',args:{action:'verify'}}]);
  await page.screenshot({path:'artifacts/virtual-lab-gui-fixture.png',fullPage:true});
  await page.getByRole('button',{name:'STOP LAB / CLEANUP'}).click();await expect(create).toBeEnabled();
  await expect(page.getByText('STOPPED',{exact:true})).toHaveCount(2);
});
