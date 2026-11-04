import {expect,test} from '@playwright/test';

test('calm crimson is default and switching to phosphor preserves the interface',async({page})=>{
  await page.goto('/');const app=page.locator('.app');const skull=page.locator('.hero-art .skull');
  await expect(app).toHaveAttribute('data-theme','calm');await expect(app).not.toHaveClass(/\bcrt\b/);
  await expect(page.locator('.hero-baseline')).toContainText('IDLE');
  expect(await app.evaluate(e=>(e as HTMLElement).style.getPropertyValue('--glow'))).toBe('0px');
  const before=await skull.boundingBox();
  const colors=await app.evaluate(e=>{
    const style=getComputedStyle(e);const bone=getComputedStyle(e.querySelector('.skull-cell-bone')!).color;
    const rgb=(s:string)=>(s.match(/[\d.]+/g)||[]).slice(0,3).map(Number);
    const luminance=(v:number[])=>v.map(n=>n/255).map(n=>n<=.04045?n/12.92:((n+.055)/1.055)**2.4).reduce((sum,n,i)=>sum+n*[.2126,.7152,.0722][i],0);
    const fg=luminance(rgb(style.color)),bg=luminance(rgb(style.backgroundColor));
    return {bone,contrast:(Math.max(fg,bg)+.05)/(Math.min(fg,bg)+.05)};
  });
  expect(colors.bone).toBe('rgb(188, 120, 115)');expect(colors.contrast).toBeGreaterThanOrEqual(4.5);
  await page.screenshot({path:'artifacts/theme-calm.png',fullPage:true});
  await page.getByRole('button',{name:/SETTINGS/}).click();await page.getByLabel('Color theme',{exact:true}).selectOption('phosphor');
  await expect(app).toHaveAttribute('data-theme','phosphor');await expect(app).toHaveClass(/\bcrt\b/);
  await expect(page.getByLabel('Phosphor scanlines',{exact:true})).toBeChecked();
  await page.getByRole('button',{name:/OVERVIEW/}).click();
  await expect.poll(()=>skull.locator('.skull-cell-bone').first().evaluate(e=>getComputedStyle(e).color)).toBe('rgb(57, 255, 120)');
  expect(await page.locator('.brand').evaluate(e=>getComputedStyle(e).color)).toBe('rgb(57, 255, 120)');
  const after=await skull.boundingBox();
  expect(after?.width).toBe(before?.width);expect(after?.height).toBe(before?.height);
  await page.screenshot({path:'artifacts/theme-phosphor.png',fullPage:true});
  await page.getByRole('button',{name:/SETTINGS/}).click();await page.getByLabel('Color theme',{exact:true}).selectOption('calm');
  await expect(page.getByLabel('Phosphor scanlines',{exact:true})).not.toBeChecked();
  expect(await app.evaluate(e=>(e as HTMLElement).style.getPropertyValue('--glow'))).toBe('0px');
});
