import {chromium} from '@playwright/test';
import {mkdirSync} from 'node:fs';
import {createServer} from 'vite';
mkdirSync('artifacts',{recursive:true});
let server;
try{await fetch('http://127.0.0.1:1420/',{signal:AbortSignal.timeout(1000)});}catch{server=await createServer({server:{host:'127.0.0.1',port:1420,strictPort:true}});await server.listen();}
const browser=await chromium.launch({channel:'chrome',headless:true});
try{
 const page=await browser.newPage({viewport:{width:1600,height:1080}});const errors=[];page.on('pageerror',error=>errors.push(error.message));
 await page.goto('http://127.0.0.1:1420/');await page.getByRole('button',{name:/ACADEMY/}).click();await page.getByRole('button',{name:/07 \/ Deauth,/}).click();
 await page.evaluate(()=>window.scrollTo(0,0));await page.screenshot({path:'artifacts/deauth-theory.png',fullPage:true});
 await page.getByRole('button',{name:'ТЕОРИЮ РАЗОБРАЛ → К ТЕСТУ'}).click();
 for(const label of ['Deauthentication','Нет','Применимый ключевой контекст и успешная проверка целостности','Нет, переподключение и приём кадров не гарантируются'])await page.getByLabel(label,{exact:true}).check();
 await page.getByRole('button',{name:'ПРОВЕРИТЬ ТЕСТ'}).click();await page.getByRole('button',{name:/03 ЛАБОРАТОРИЯ/}).click();
 const frame=page.getByLabel('Учебный management frame',{exact:true});const prediction=page.getByLabel('Прогноз состояния',{exact:true});const receive=page.getByRole('button',{name:'СМОДЕЛИРОВАТЬ ПРИЁМ КАДРА'});
 await frame.selectOption('open-disassoc');await prediction.selectOption('authenticated');await receive.click();await page.getByRole('button',{name:'СЛЕДУЮЩИЙ КЕЙС →'}).click();
 await prediction.selectOption('unauthenticated');await receive.click();await page.getByRole('button',{name:'СЛЕДУЮЩИЙ КЕЙС →'}).click();
 await prediction.selectOption('associated');await receive.click();
 await page.evaluate(()=>window.scrollTo(0,0));await page.screenshot({path:'artifacts/deauth-lab.png',fullPage:true});
 console.log(JSON.stringify({errors,theory:'artifacts/deauth-theory.png',lab:'artifacts/deauth-lab.png'}));
}finally{await browser.close();if(server)await server.close();}
