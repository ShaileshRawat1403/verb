#!/usr/bin/env python3
"""Owned, loopback-only IDE trial lifecycle. Never accepts host mounts or phone targets."""
import argparse,datetime,json,os,stat,subprocess,time,urllib.error,urllib.request
from pathlib import Path
LABEL='com.verb.ide-lab'
IMAGES={'code-server':'verb-ide-lab-code-server:20261025','theia':'verb-ide-lab-theia:20261025'}
def docker(*args,timeout=30):
 r=subprocess.run(['docker',*args],capture_output=True,text=True,timeout=timeout,check=True)
 return r.stdout.strip()
def inspect(kind,identity):return json.loads(docker(kind,'inspect',identity))[0]
def load(root):
 root=Path(root)
 if root.is_symlink():raise ValueError('symlink lab root')
 root=root.resolve();p=root/'receipt.json';info=p.lstat()
 if not stat.S_ISREG(info.st_mode) or info.st_uid!=os.getuid() or info.st_mode&0o077:raise ValueError('untrusted receipt')
 d=json.loads(p.read_text())
 if d['schema']!=1 or Path(d['root']).resolve()!=root or not root.name.startswith('verb-ide-lab-'):raise ValueError('invalid lab receipt')
 return root,d
def save(root,d):
 p=root/'receipt.json';p.write_text(json.dumps(d,indent=2));p.chmod(0o600)
def owned(root,d,candidate):
 c=d['containers'][candidate];meta=inspect('container',c['id'])
 if meta['Config']['Labels'].get(LABEL)!=root.name:raise ValueError('container ownership changed')
 if meta.get('Mounts'):raise ValueError('unexpected trial mounts')
 for binding in meta['HostConfig']['PortBindings'].values():
  if any(p['HostIp']!='127.0.0.1' for p in binding):raise ValueError('unexpected published interface')
 return c,meta
def start(root,d,name):
 if any(inspect('container',c['id'])['State']['Running'] for c in d['containers'].values()):raise ValueError('run one IDE at a time for resource comparison')
 if not d['network']:
  d['network']=docker('network','create','--internal','--label',LABEL+'='+root.name,root.name);save(root,d)
 net=inspect('network',d['network'])
 if not net['Internal'] or net['Labels'].get(LABEL)!=root.name:raise ValueError('unexpected trial network')
 if name not in d['containers']:
  image_ref=d.get('images',{}).get(name,IMAGES[name]);image=inspect('image',image_ref)
  if image['Architecture']!='arm64' or image['Config']['Labels'].get(LABEL)!=root.name:raise ValueError('unverified trial image')
  port=8080 if name=='code-server' else 3000
  cmd=['create','--name',root.name+'-'+name,'--label',LABEL+'='+root.name,'--init','--cpus','2','--memory','3g','--pids-limit','512','--cap-drop','ALL','--security-opt','no-new-privileges','--network',d['network'],'--publish','127.0.0.1::'+str(port),'--publish','127.0.0.1::3001','--tmpfs','/tmp:rw,nosuid,nodev,size=268435456']
  if name=='code-server':cmd+=['--entrypoint','/usr/bin/code-server',image_ref,'--auth','none','--disable-telemetry','--disable-update-check','--bind-addr','0.0.0.0:8080','--user-data-dir','/home/verb-lab/data','--extensions-dir','/home/verb-lab/extensions','/home/project']
  else:cmd+=['--entrypoint','/usr/local/bin/node',image_ref,'/home/theia/applications/browser/lib/backend/main.js','/home/project','--hostname=0.0.0.0','--port=3000']
  cid=docker(*cmd);d['containers'][name]={'id':cid,'imageId':image['Id'],'imageBytes':image['Size'],'internalPort':port};save(root,d)
 c,meta=owned(root,d,name)
 if not c.get('proxyId'):
  proxy_ref=d.get('images',{}).get('proxy','verb-ide-lab-proxy:20261025')
  proxy_image=inspect('image',proxy_ref)
  if proxy_image['Config']['Labels'].get(LABEL)!=root.name:raise ValueError('proxy image ownership changed')
  proxy=docker('create','--name',root.name+'-'+name+'-proxy','--label',LABEL+'='+root.name,'--init','--read-only','--cpus','0.25','--memory','128m','--pids-limit','32','--cap-drop','ALL','--security-opt','no-new-privileges','--network','bridge','--publish','127.0.0.1::'+str(c['internalPort']),'--publish','127.0.0.1::3001','--env','UPSTREAM='+meta['Name'].lstrip('/'),'--env','IDE_PORT='+str(c['internalPort']),proxy_ref)
  docker('network','connect',d['network'],proxy);c['proxyId']=proxy;save(root,d)
 proxy=inspect('container',c['proxyId'])
 if proxy['Config']['Labels'].get(LABEL)!=root.name or proxy.get('Mounts') or not proxy['HostConfig']['ReadonlyRootfs']:raise ValueError('unexpected proxy ownership')
 start=time.monotonic();started=False
 try:
  docker('start',c['id']);started=True;docker('start',c['proxyId'])
  for i in range(90):
   meta=inspect('container',c['id'])
   if not meta['State']['Running']:raise RuntimeError('trial exited; inspect owned container logs')
   proxy=inspect('container',c['proxyId'])
   bindings=proxy['NetworkSettings']['Ports'].get(str(c['internalPort'])+'/tcp')
   if bindings:
    port=bindings[0]['HostPort'];origin='http://127.0.0.1:'+port
    try:
     with urllib.request.urlopen(origin,timeout=2) as r:
      if r.status==200:break
    except (urllib.error.URLError,TimeoutError):pass
   time.sleep(1)
  else:raise RuntimeError('trial readiness timeout')
 except BaseException:
  if started:stop(root,d,name)
  raise
 c.update({'origin':origin,'url':origin+('/?folder=/home/project' if name=='code-server' else '/#/home/project'),'previewOrigin':'http://127.0.0.1:'+proxy['NetworkSettings']['Ports']['3001/tcp'][0]['HostPort'],'httpReadySeconds':round(time.monotonic()-start,3),'lastStarted':datetime.datetime.now(datetime.timezone.utc).isoformat()});save(root,d)
 return c
def status(root,d,name):
 c,meta=owned(root,d,name)
 result={'candidate':name,'running':meta['State']['Running'],'imageBytes':c['imageBytes'],'httpReadySeconds':c.get('httpReadySeconds'),'url':c.get('url')}
 if result['running']:
  result['dockerStats']=json.loads(docker('stats','--no-stream','--format','{{json .}}',c['id']))
  result['processes']=docker('top',c['id'],'-eo','pid,ppid,rss,comm')
  result['runtimeVersions']=docker('exec',c['id'],'sh','-c','node22 --version; python3 --version; git --version')
 return result
def stop(root,d,name):
 c,meta=owned(root,d,name)
 proxy=inspect('container',c['proxyId']) if c.get('proxyId') else None
 if proxy and proxy['Config']['Labels'].get(LABEL)!=root.name:raise ValueError('proxy ownership changed')
 if meta['State']['Running']:docker('stop','--time','15',c['id'])
 if proxy and proxy['State']['Running']:docker('stop','--time','10',c['proxyId'])
 return {'stopped':not inspect('container',c['id'])['State']['Running'],'candidate':name,'dataPreserved':True}
def main():
 a=argparse.ArgumentParser();a.add_argument('action',choices=['start','status','stop']);a.add_argument('candidate',choices=IMAGES);a.add_argument('--root',required=True);args=a.parse_args();root,d=load(args.root)
 if args.action=='start':result=start(root,d,args.candidate)
 elif args.action=='status':result=status(root,d,args.candidate)
 else:result=stop(root,d,args.candidate)
 print(json.dumps(result,indent=2))
if __name__=='__main__':main()
