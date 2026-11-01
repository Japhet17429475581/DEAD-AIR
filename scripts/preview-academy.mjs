import {chromium} from '@playwright/test';
import {mkdirSync} from 'node:fs';
import {createServer} from 'vite';
mkdirSync('artifacts',{recursive:true});
let server;
try{await fetch('http://127.0.0.1:1420/',{signal:AbortSignal.timeout(1000)});}catch{server=await createServer({server:{host:'127.0.0.1',port:1420,strictPort:true}});await server.listen();}
const browser=await chromium.launch({channel:'chrome',headless:true});
try {
  const page=await browser.newPage({viewport:{width:1600,height:1080}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('http://127.0.0.1:1420/');await page.getByRole('button',{name:/ACADEMY/}).click();
  await page.getByRole('button',{name:/03 \/ Monitor mode/}).click();
  await page.screenshot({path:'artifacts/academy-roadmap.png',fullPage:true});
  await page.getByRole('button',{name:/05 \/ PMKID:/}).click();await page.getByRole('button',{name:'ТЕОРИЮ РАЗОБРАЛ → К ТЕСТУ'}).click();
  await page.getByLabel('Нет, это может требовать взаимодействия с AP',{exact:true}).check();await page.getByLabel('Кандидат для этого вектора и его параметров',{exact:true}).check();
  await page.getByRole('button',{name:'ПРОВЕРИТЬ ТЕСТ'}).click();await page.getByRole('button',{name:/03 ЛАБОРАТОРИЯ/}).click();
  await page.getByRole('button',{name:'ПРОВЕРИТЬ УЧЕБНЫЕ КАНДИДАТЫ'}).click();await page.getByText(/^Совпадение с известным учебным вектором/).waitFor();
  await page.screenshot({path:'artifacts/academy-pmkid.png',fullPage:true});
  console.log(JSON.stringify({errors,roadmap:'artifacts/academy-roadmap.png',pmkid:'artifacts/academy-pmkid.png'}));
} finally {await browser.close();if(server)await server.close();}
