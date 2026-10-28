// A real Chromium worker behind Verb, with HTTP fixtures that refuse all iframe embedding.
import {test, expect, chromium} from '@playwright/test';
import {spawn, execFileSync} from 'node:child_process';
import {mkdtempSync, mkdirSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {createServer as httpServer} from 'node:http';
import {createServer} from 'node:net';

const TOKEN='browser-owner-fixture-000000000000000000';
const client='browser-test-client';
let host, target, second, base, site, other, url, hits=0, latestSession;
const freePort=()=>new Promise(r=>{const s=createServer().listen(0,'127.0.0.1',()=>{const p=s.address().port;s.close(()=>r(p));});});
const listen=(server)=>new Promise(r=>server.listen(0,'127.0.0.1',()=>r(`http://127.0.0.1:${server.address().port}`)));
async function call(body, token=TOKEN){
 const response=await fetch(`${base}/api/browser`,{method:'POST',headers:{'X-Verb-Token':token,'Content-Type':'application/json'},body:JSON.stringify({client,session:latestSession,...body})});
 const result=await response.json(); if(result.session) latestSession=result.session;
 return {status:response.status,body:result};
}
const action=async(op,view,extra={})=>{
 const result=await call({op,generation:view?.generation,...extra});
 expect(result.status,JSON.stringify(result.body)).toBe(200);expect(result.body.error,JSON.stringify(result.body)).toBeFalsy();return result.body;
};
test.describe.configure({mode:'serial'});
test.beforeAll(async()=>{
 const root=mkdtempSync(join(tmpdir(),'verb-real-browser-')), repo=join(root,'project'), home=join(root,'home');mkdirSync(repo);mkdirSync(home);
 execFileSync('git',['init','-q','-b','main',repo]);execFileSync('git',['-C',repo,'config','user.name','Browser Fixture']);execFileSync('git',['-C',repo,'config','user.email','fixture@example.invalid']);
 writeFileSync(join(repo,'README.md'),'# Browser workspace\n');execFileSync('git',['-C',repo,'add','.']);execFileSync('git',['-C',repo,'commit','-qm','initial']);
 second=httpServer((req,res)=>{hits++;res.end('<h1>Other site</h1>');});other=await listen(second);
 target=httpServer((req,res)=>{
  if(req.url==='/redirect'){res.writeHead(302,{Location:other+'/landing'});res.end();return;}
  if(req.url==='/slow'){setTimeout(()=>{if(!res.destroyed)res.end('<h1>Slow</h1>');},12000).unref();return;}
  res.writeHead(200,{'Content-Type':'text/html','X-Frame-Options':'DENY','Content-Security-Policy':"frame-ancestors 'none'"});
  res.end(`<!doctype html><meta name="viewport" content="width=device-width"><title>${req.url==='/second'?'Second page':'Browser fixture'}</title><style>body{margin:0;font:18px system-ui;background:#f8f5ef;padding:20px}h1{font-size:26px;line-height:1.2;margin:0;max-width:320px}button{position:absolute;left:20px;top:90px;padding:12px;border:0;border-radius:8px;background:#2d634a;color:white}input{position:absolute;left:20px;top:150px;padding:10px;border:1px solid #ccc;border-radius:8px}#scope{position:absolute;left:20px;top:235px;margin:0;max-width:320px}#typed{position:absolute;top:195px}img{display:none}</style><h1>${req.url==='/second'?'Second page':'Real browser fixture'}</h1><button onclick="this.textContent='Count '+(++window.count)">Count 0</button><input aria-label="Fixture text" oninput="document.querySelector('#typed').textContent=this.value"><p id="typed"></p><p id="scope"></p><img src="${other}/pixel"><script>window.count=0;document.querySelector('#scope').textContent='Top-level page: '+(window.top===window)+'; stored: '+(localStorage.getItem('probe')||'none');localStorage.setItem('probe','present');</script>`);
 });site=await listen(target);
 const port=await freePort();base=`http://127.0.0.1:${port}`;url=`${base}/#${TOKEN}`;
 host=spawn(resolve(process.env.VERB_BIN??'../target/debug/verb'),['web','--port',String(port)],{cwd:repo,env:{PATH:process.env.PATH,HOME:home,VERB_STATE_DIR:join(root,'state'),VERB_TOKEN:TOKEN,VERB_BROWSER_NODE:process.execPath,VERB_BROWSER_WORKER:resolve('runtime/browser-worker.mjs'),VERB_BROWSER_EXECUTABLE:chromium.executablePath()},stdio:['ignore','pipe','pipe']});
 await new Promise((ok,fail)=>{const timer=setTimeout(()=>fail(Error('Verb did not start')),20000);host.stdout.on('data',c=>{if(String(c).includes('Verb web UI')){clearTimeout(timer);ok();}});host.on('error',fail);host.on('exit',code=>fail(Error(`Verb ended ${code}`)));});
});
test.afterAll(async()=>{await call({op:'stop'}).catch(()=>{});host?.kill();target?.closeAllConnections();target?.close();second?.close();});

test('real pages render with iframe denial while unapproved sites and Verb itself stay blocked',async()=>{
 expect((await call({op:'start'},'wrong')).status).toBe(403);
 let view=await action('start');expect(view.running).toBe(true);expect(view.image.length).toBeGreaterThan(100);
 view=await action('open',view,{url:site+'/redirect',allow:true});expect(view.blocked).toEqual([other]);expect(hits).toBe(0);
 view=await action('open',view,{url:site,allow:true});expect(view.tabs[0].title).toBe('Browser fixture');expect(view.blocked).toEqual([other]);expect(hits).toBe(0);
 view=await action('inspect',view);expect(view.text).toContain('Top-level page: true');expect(view.text).toContain('stored: none');
 for(const address of [base,base.replace('127.0.0.1','localhost'),`file:///etc/passwd`,`http://name:password@127.0.0.1:${target.address().port}`]){
  const refused=await call({op:'open',url:address,allow:true,generation:view.generation});expect(refused.body.error).toBeTruthy();
 }
 view=await action('open',view,{url:site+'/redirect',allow:true});expect(hits).toBe(0);
 view=await action('open',view,{url:site,allow:true});
 view=await action('click',view,{x:50,y:100});view=await action('inspect',view);expect(view.text).toContain('Count 1');
 view=await action('click',view,{x:60,y:170});view=await action('type',view,{text:'Typed in real browser'});view=await action('inspect',view);expect(view.text).toContain('Typed in real browser');
 view=await action('viewport',view,{width:390,height:720});expect(view.viewport.width).toBe(390);
 view=await action('inspect',view);expect(view.text).toContain('Count 1');
 await action('stop',view);
 const oldSession=view.session;
 view=await action('start');view=await action('open',view,{url:site,allow:true});view=await action('inspect',view);expect(view.text).toContain('stored: none');
 const staleSession=await call({op:'open',session:oldSession,url:site,allow:true,generation:view.generation});expect(staleSession.body.error).toContain('session changed');
 const staleStop=await call({op:'stop',session:oldSession});expect(staleStop.status).toBe(409);
 await action('stop',view);
});

test('navigation, tabs and stale browser leases reflect the real browser session',async()=>{
 let view=await action('start');view=await action('open',view,{url:site,allow:true});view=await action('open',view,{url:site+'/second',allow:true});expect(view.tabs[0].title).toBe('Second page');
 view=await action('back',view);expect(view.tabs[0].title).toBe('Browser fixture');view=await action('forward',view);expect(view.tabs[0].title).toBe('Second page');
 view=await action('new',view);expect(view.tabs).toHaveLength(2);
 const old=view.generation;
 const take=await call({op:'take',client:'another-browser-client'});expect(take.body.controller).toBe('another-browser-client');expect(take.body.generation).toBeGreaterThan(old);
 const observed=await call({op:'capture',generation:old});expect(observed.body.image.length).toBeGreaterThan(100);expect(observed.body.controller).toBe('another-browser-client');
 const stale=await call({op:'open',url:site,allow:true,generation:old});expect(stale.body.error).toContain('control changed');
 await action('stop',view);
});

test('stop interrupts a pending navigation and discards the browser session',async()=>{
 let view=await action('start');
 const navigation=call({op:'open',url:site+'/slow',allow:true,generation:view.generation});
 await new Promise(r=>setTimeout(r,700));
 const start=Date.now();const stopped=await call({op:'stop'});expect(Date.now()-start).toBeLessThan(7000);expect(stopped.body.running).toBe(false);
 await navigation;view=await action('status');expect(view.running).toBe(false);
});

test('a worker crash clears the session and explicitly starting again creates a fresh identity',async()=>{
 const view=await action('start');
 const pids=execFileSync('pgrep',['-P',String(host.pid)],{encoding:'utf8'}).trim().split(/\s+/).map(Number);
 expect(pids).toHaveLength(1);
 process.kill(pids[0],'SIGKILL');
 await new Promise(r=>setTimeout(r,300));
 const lost=await call({op:'status'});expect(lost.status).not.toBe(200);
 const fresh=await action('start');expect(fresh.session).not.toBe(view.session);
 await action('stop',fresh);
});

test('bounded browser resource and cleanup sample',async()=>{
 const started=Date.now();let view=await action('start');const launchMs=Date.now()-started;
 const opened=Date.now();view=await action('open',view,{url:site,allow:true});const openMs=Date.now()-opened;
 const captures=[];
 for(let i=0;i<3;i++){const began=Date.now();view=await action('capture',view);captures.push(Date.now()-began);}
 const owned=[];
 function children(pid){
  let ids=[];try{ids=execFileSync('pgrep',['-P',String(pid)],{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim().split(/\s+/).filter(Boolean).map(Number);}catch{}
  for(const id of ids){if(owned.length>=32)break;owned.push(id);children(id);}
 }
 children(host.pid);
 const sample=execFileSync('ps',['-p',owned.join(','),'-o','pid=,ppid=,rss=,pcpu=,comm='],{encoding:'utf8'});
 writeFileSync('/private/tmp/verb-browser-resource-sample-20261028.json',JSON.stringify({platform:process.platform,launchMs,openMs,captureMs:captures,ownedProcesses:sample,notes:'One local fixture, one viewport; sum of RSS includes shared pages and is not unique memory. Not phone-hosted or production evidence.'},null,2));
 await action('stop',view);
 await expect.poll(()=>owned.filter(pid=>{try{process.kill(pid,0);return true;}catch{return false;}}).length,{timeout:7000}).toBe(0);
});

test('Studio browser shows pixels, page text, responsive view and teardown without embedding site code',async({page})=>{
 let releaseFiles; const filesReady=new Promise(resolve=>{releaseFiles=resolve;});
 await page.route('**/api/files',async route=>{await filesReady;await route.continue();});
 await page.goto(url);await page.getByRole('button',{name:'Studio',exact:true}).click();await page.getByRole('button',{name:'Browser',exact:true}).click();
 releaseFiles();await expect(page.locator('#studio-browser')).toBeVisible();
 await expect(page.locator('#studio-show-document')).toHaveAttribute('aria-pressed','false');
 await expect(page.locator('#studio-show-browser')).toHaveAttribute('aria-pressed','true');
 await expect(page.locator('#studio-browser-status')).toHaveText('Ready to start. No browser is running.');
 await page.getByRole('button',{name:'Start browser',exact:true}).click();await expect(page.locator('#studio-browser-stage img')).toBeVisible();
 await page.getByRole('textbox',{name:'Website address',exact:true}).fill(site);await page.getByRole('button',{name:'Allow site & open',exact:true}).click();
 await expect(page.locator('#studio-browser-current-url')).toHaveText(site+'/');await expect(page.locator('#studio-browser-tabs')).toContainText('Browser fixture');
 let refusedCapture=false;
 await page.route('**/api/browser',async route=>{
  if(!refusedCapture && route.request().method()==='POST' && route.request().postDataJSON()?.op==='capture') {
   refusedCapture=true;await route.fulfill({status:500,json:{error:'Browser is busy. Wait for the current action; it was not queued.'}});return;
  }
  await route.continue();
 });
 await page.waitForResponse(response=>response.url().endsWith('/api/browser') && response.status()===500);
 await expect(page.getByRole('button',{name:'Stop browser',exact:true})).toBeVisible();
 await expect(page.locator('#studio-browser-stage img')).toBeVisible();
 await page.waitForResponse(response=>response.url().endsWith('/api/browser') && response.request().method()==='POST' && response.request().postDataJSON()?.op==='status');
 await expect(page.locator('#studio-browser-tabs')).toContainText('Browser fixture');
 await page.unroute('**/api/browser');
 await page.getByRole('button',{name:'Inspect page',exact:true}).click();await page.locator('#studio-browser-text').locator('..').locator('summary').click();await expect(page.locator('#studio-browser-text')).toContainText('Real browser fixture');
 expect(await page.locator('#studio-browser iframe, #studio-browser script').count()).toBe(0);
 const tab=page.locator('#studio-browser-tabs button').first();await tab.focus();
 await page.waitForResponse(response=>response.url().endsWith('/api/browser') && response.request().method()==='POST' && response.request().postDataJSON()?.op==='capture');
 await expect(tab).toBeFocused();
 await page.locator('.studio-browser-entry summary').click();
 const type=page.getByRole('textbox',{name:'Text for focused page field',exact:true});
 await type.fill('Keep my browser draft');
 await page.route('**/api/browser', async route=>{
  if(route.request().method()==='POST' && route.request().postDataJSON()?.op==='type') {
   await route.fulfill({status:200,json:{error:'Fixture refused input',running:true}});return;
  }
  await route.continue();
 });
 await page.getByRole('button',{name:'Type into page',exact:true}).click();
 await expect(page.locator('#studio-browser-status')).toContainText('Fixture refused input');
 await expect(type).toHaveValue('Keep my browser draft');
 await page.unroute('**/api/browser');
 const image=page.locator('#studio-browser-stage img');
 const bounds=await image.boundingBox();
 const width=await image.evaluate(img=>img.naturalWidth);
 await image.click({position:{x:60*bounds.width/width,y:170*bounds.width/width}});
 await expect(page.locator('#studio-browser-send-text')).toBeEnabled();
 await type.press('Enter');await expect(type).toHaveValue('');
 await page.getByRole('button',{name:'Inspect page',exact:true}).click();
 await expect(page.locator('#studio-browser-text')).toContainText('Keep my browser draft');
 await page.locator('.studio-browser-entry summary').click();
 await page.getByLabel('Browser viewport',{exact:true}).selectOption('390');await expect.poll(()=>page.locator('#studio-browser-stage img').evaluate(i=>i.naturalWidth)).toBe(390);
 await page.locator('.studio-browser-head').scrollIntoViewIfNeeded();
 await page.screenshot({path:test.info().outputPath('verb-browser-desktop.png'),fullPage:false});
 await page.setViewportSize({width:390,height:844});await page.getByRole('button',{name:'Canvas',exact:true}).click();await page.locator('#studio-browser-stage').scrollIntoViewIfNeeded();
 expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);await page.screenshot({path:test.info().outputPath('verb-browser-phone.png'),fullPage:false});
 await page.getByRole('button',{name:'Stop browser',exact:true}).click();await expect(page.locator('#studio-browser-stage img')).toHaveCount(0);
});
