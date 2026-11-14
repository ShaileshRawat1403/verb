#!/usr/bin/env python3
"""Create synthetic, dependency-free IDE trial material; never read a real project."""
import json,os,subprocess,tempfile,zipfile
from pathlib import Path

def write(root,name,text):
 p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
def git(root,*args):
 subprocess.run(['git','-c','core.hooksPath=/dev/null','-c','commit.gpgSign=false','-C',str(root),*args],check=True,capture_output=True,env={'PATH':os.environ['PATH'],'HOME':str(root.parent/'fixture-home'),'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null','LANG':'C.UTF-8'})
def prepare(parent=None):
 root=Path(tempfile.mkdtemp(prefix='verb-ide-lab-',dir=parent));root.chmod(0o700)
 project=root/'project';project.mkdir()
 files={
 'README.md':'''# Verb IDE laboratory
Synthetic project. No credentials, remote, provider login or production data.

- Build: `node22 tools/build.cjs`
- Test: `node22 --test test/app.test.cjs`
- Python: `python3 -m unittest discover -s python`
- Preview: `node22 src/server.cjs`, port 3001
- Debug: choose **Verb Lab: debug calculation**, breakpoint at `const total`.
- Extension: command **Verb Lab: Verify Extension**.
- Conflict: merge `conflict-right` into `conflict-left`; resolve only this fixture.
''',
 '.gitignore':'dist/\n.verb-lab/\n__pycache__/\n*.pyc\n',
 'package.json':json.dumps({'name':'verb-ide-lab-fixture','version':'1.0.0','private':True,'scripts':{'build':'node22 tools/build.cjs','test':'node22 --test test/app.test.cjs','start':'node22 src/server.cjs'}},indent=2),
 'src/greet.cjs':'function greet(name) { return `Hello, ${name}!`; }\nmodule.exports = { greet };\n',
 'src/server.cjs':'''const http = require('node:http');
const fs = require('node:fs');
const { greet } = require('./greet.cjs');
http.createServer((req,res) => {
 if (req.url === '/api/hello') { res.setHeader('Content-Type','application/json'); res.end(JSON.stringify({message:greet('Verb')})); return; }
 res.setHeader('Content-Type','text/html'); res.end(fs.readFileSync('public/index.html'));
}).listen(3001, '0.0.0.0', () => console.log('Verb Lab preview ready on 3001'));
''',
 'src/debug.cjs':'''function calculate(price, quantity) {
 const total = price * quantity;
 return total;
}
const result = calculate(7, 3);
console.log('Calculated total:', result);
''',
 'test/app.test.cjs':'''const { test } = require('node:test');
const assert = require('node:assert/strict');
const { greet } = require('../src/greet.cjs');
test('greeting preserves the requested name', () => assert.equal(greet('Verb'), 'Hello, Verb!'));
''',
 'tools/build.cjs':'''const fs = require('node:fs');
fs.mkdirSync('dist', {recursive:true});
fs.copyFileSync('public/index.html','dist/index.html');
console.log('Built dist/index.html');
''',
 'public/index.html':'''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Verb IDE Lab</title><style>body{font:18px system-ui;max-width:700px;margin:60px auto;padding:24px;background:#eef4f1;color:#163c31}button{padding:12px}</style><h1>Built with the Verb IDE lab</h1><p>This is a disposable sample app.</p><button id="hello">Test application</button><p id="result" role="status"></p><script>document.querySelector('#hello').onclick=async()=>{document.querySelector('#result').textContent=(await(await fetch('/api/hello')).json()).message;};</script></html>''',
 'python/calc.py':'def total(price, quantity):\n    return price * quantity\n',
 'python/test_calc.py':'import unittest\nfrom calc import total\nclass Calculation(unittest.TestCase):\n    def test_total(self):\n        self.assertEqual(total(7, 3), 21)\n',
 'config/theme.txt':'theme=blue\n',
 '.vscode/tasks.json':json.dumps({'version':'2.0.0','tasks':[{'label':'Verb Lab: build','type':'process','command':'node22','args':['tools/build.cjs'],'group':'build','problemMatcher':[]},{'label':'Verb Lab: test','type':'process','command':'node22','args':['--test','test/app.test.cjs'],'group':'test','problemMatcher':[]},{'label':'Verb Lab: Python test','type':'process','command':'python3','args':['-m','unittest','discover','-s','python'],'problemMatcher':[]},{'label':'Verb Lab: preview','type':'process','command':'node22','args':['src/server.cjs'],'isBackground':True,'problemMatcher':[]}]},indent=2),
 '.vscode/launch.json':json.dumps({'version':'0.2.0','configurations':[{'type':'pwa-node','request':'launch','name':'Verb Lab: debug calculation','runtimeExecutable':'/usr/local/bin/node22','program':'${workspaceFolder}/src/debug.cjs','cwd':'${workspaceFolder}','stopOnEntry':True,'skipFiles':['<node_internals>/**']}]},indent=2),
 '.vscode/settings.json':json.dumps({'files.autoSave':'off','workbench.startupEditor':'none'},indent=2),
 }
 for name,text in files.items():write(project,name,text)
 git(project,'init','-q','-b','main');git(project,'config','user.name','Verb IDE Lab');git(project,'config','user.email','fixture@example.invalid');git(project,'config','core.autocrlf','false');git(project,'add','.');git(project,'commit','-qm','Synthetic IDE fixture')
 for branch,color in [('conflict-left','purple'),('conflict-right','green')]:
  git(project,'switch','-qc',branch,'main');write(project,'config/theme.txt',f'theme={color}\n');git(project,'add','config/theme.txt');git(project,'commit','-qm',f'Fixture {branch}')
 git(project,'switch','-q','main')
 extension={
 'package.json':json.dumps({'name':'probe','displayName':'Verb Lab Probe','publisher':'verb-lab','version':'0.0.1','engines':{'vscode':'^1.90.0'},'main':'./extension.js','activationEvents':['onStartupFinished','onCommand:verbLab.verify'],'contributes':{'commands':[{'command':'verbLab.verify','title':'Verb Lab: Verify Extension'}]}},indent=2),
 'extension.js':'''const vscode = require('vscode');
exports.activate = function(context) {
 async function record(name, value) {
  const folder = vscode.workspace.workspaceFolders?.[0]; if (!folder) return;
  const dir = vscode.Uri.joinPath(folder.uri, '.verb-lab'); await vscode.workspace.fs.createDirectory(dir);
  await vscode.workspace.fs.writeFile(vscode.Uri.joinPath(dir, name), Buffer.from(JSON.stringify(value, null, 2)));
 }
 context.subscriptions.push(vscode.commands.registerCommand('verbLab.verify', async () => {
  await record('extension.json', {command:true, version:vscode.version, workspace:vscode.workspace.workspaceFolders?.length});
  vscode.window.showInformationMessage('Verb Lab extension command passed');
 }));
 if (vscode.debug.registerDebugAdapterTrackerFactory) context.subscriptions.push(vscode.debug.registerDebugAdapterTrackerFactory('*', {
  createDebugAdapterTracker(session) { const events=[]; return { onDidSendMessage(message) {
   if (message.type === 'event' && ['initialized','stopped','continued','terminated'].includes(message.event)) {
    events.push({event:message.event,reason:message.body?.reason}); record('debug.json',{type:session.type,events}).catch(()=>{});
   }
  }}; }
 }));
};
''',
 'README.md':'# Verb Lab Probe\nLocal synthetic extension; records command and debugger event proofs inside this fixture only.\n',
 'LICENSE':'MIT License\nCopyright 2026 Verb IDE Lab\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies, subject to inclusion of this notice. THE SOFTWARE IS PROVIDED AS IS, WITHOUT WARRANTY OF ANY KIND.\n'
 }
 vsix=root/'verb-lab-probe-0.0.1.vsix'
 with zipfile.ZipFile(vsix,'w',zipfile.ZIP_DEFLATED) as z:
  z.writestr('[Content_Types].xml','<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="json" ContentType="application/json"/><Default Extension="js" ContentType="application/javascript"/><Default Extension="md" ContentType="text/markdown"/><Default Extension="vsixmanifest" ContentType="text/xml"/></Types>')
  z.writestr('extension.vsixmanifest','''<?xml version="1.0"?><PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011"><Metadata><Identity Language="en-US" Id="probe" Version="0.0.1" Publisher="verb-lab"/><DisplayName>Verb Lab Probe</DisplayName><Description xml:space="preserve">Synthetic IDE verification</Description><Categories>Other</Categories><Properties><Property Id="Microsoft.VisualStudio.Code.Engine" Value="^1.90.0"/></Properties></Metadata><Installation><InstallationTarget Id="Microsoft.VisualStudio.Code"/></Installation><Dependencies/><Assets><Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true"/></Assets></PackageManifest>''')
  for name,text in extension.items():z.writestr('extension/'+name,text)
 receipt={'schema':1,'root':str(root),'fixture':str(project),'vsix':str(vsix),'containers':{},'network':None,'images':{}}
 (root/'receipt.json').write_text(json.dumps(receipt,indent=2));(root/'receipt.json').chmod(0o600)
 return root
if __name__=='__main__':print(prepare())
