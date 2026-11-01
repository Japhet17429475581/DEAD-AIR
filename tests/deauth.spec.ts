import {expect,test} from '@playwright/test';

test('deauth lesson gates a stateful offline lab and never invokes radio or privileged commands',async({page})=>{
  await page.addInitScript(()=>Object.assign(window,{
    deauthNativeCalls:[],__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},
    __TAURI_INTERNALS__:{transformCallback:()=>1,unregisterCallback:()=>{},invoke:async(cmd:string)=>{
      (window as any).deauthNativeCalls.push(cmd);
      if(cmd==='system_check')return {tools:[],adapters:[],errors:[]};
      if(cmd==='dependency_plan')return {distro:'debian',manager:'apt-get',packages:[],missing:[],helperAvailable:true,profile:'all'};
      if(cmd.startsWith('plugin:event|'))return 1;
      throw `Unexpected native operation in teaching lab: ${cmd}`;
    }}
  }));
  await page.goto('/');await page.getByRole('button',{name:/ACADEMY/}).click();
  const module=page.getByRole('button',{name:/07 \/ Deauth,/});await module.click();
  const lab=page.getByRole('button',{name:/03 ЛАБОРАТОРИЯ/});await expect(lab).toBeDisabled();
  await page.getByRole('button',{name:'ТЕОРИЮ РАЗОБРАЛ → К ТЕСТУ'}).click();
  await page.getByLabel('Deauthentication',{exact:true}).check();
  await page.getByLabel('Нет',{exact:true}).check();
  await page.getByLabel('Применимый ключевой контекст и успешная проверка целостности',{exact:true}).check();
  await page.getByLabel('Нет, переподключение и приём кадров не гарантируются',{exact:true}).check();
  await page.getByRole('button',{name:'ПРОВЕРИТЬ ТЕСТ'}).click();await expect(lab).toBeEnabled();await lab.click();
  await expect(page.getByText('SIMULATION · NO RADIO I/O')).toBeVisible();
  const frame=page.getByLabel('Учебный management frame',{exact:true});
  const prediction=page.getByLabel('Прогноз состояния',{exact:true});
  const receive=page.getByRole('button',{name:'СМОДЕЛИРОВАТЬ ПРИЁМ КАДРА'});
  const state=page.getByTestId('deauth-station-state');
  // Wrong choice changes the model, but does not pass the mission.
  await prediction.selectOption('unauthenticated');await receive.click();
  await expect(state).toContainText('UNAUTHENTICATED');await expect(page.getByRole('button',{name:'СЛЕДУЮЩИЙ КЕЙС →'})).toHaveCount(0);
  await page.getByRole('button',{name:'ВОССТАНОВИТЬ УЧЕБНУЮ СЕССИЮ'}).click();
  await frame.selectOption('open-disassoc');await prediction.selectOption('authenticated');await receive.click();
  await expect(state).toContainText('AUTHENTICATED · без association');
  await page.getByRole('button',{name:'СЛЕДУЮЩИЙ КЕЙС →'}).click();
  await prediction.selectOption('unauthenticated');await receive.click();await expect(state).toContainText('UNAUTHENTICATED');
  await page.getByRole('button',{name:'СЛЕДУЮЩИЙ КЕЙС →'}).click();
  await frame.selectOption('invalid-deauth');await prediction.selectOption('associated');await receive.click();
  await expect(state).toHaveText('ASSOCIATED · соединение установлено');
  await expect(page.getByRole('button',{name:'СЛЕДУЮЩИЙ КЕЙС →'})).toHaveCount(0);
  await frame.selectOption('open-deauth');await receive.click();
  await expect(state).toHaveText('ASSOCIATED · соединение установлено');
  await page.getByRole('button',{name:'СЛЕДУЮЩИЙ КЕЙС →'}).click();
  await frame.selectOption('valid-deauth');await prediction.selectOption('unauthenticated');await receive.click();
  await expect(state).toContainText('UNAUTHENTICATED');
  await page.getByRole('button',{name:'СМОДЕЛИРОВАТЬ ОБЫЧНОЕ ПЕРЕПОДКЛЮЧЕНИЕ'}).click();
  await expect(state).toHaveText('ASSOCIATED · соединение установлено');
  await expect(page.getByText('ЛАБОРАТОРИЯ DEAUTH ПРОЙДЕНА',{exact:true})).toBeVisible();
  await expect(module.locator('i.done')).toHaveCount(3);
  const unexpected=await page.evaluate(()=>(window as any).deauthNativeCalls.filter((cmd:string)=>!['system_check','dependency_plan'].includes(cmd)&&!cmd.startsWith('plugin:event|')));
  expect(unexpected).toEqual([]);
});
