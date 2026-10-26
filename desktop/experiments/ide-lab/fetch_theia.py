import argparse,concurrent.futures,gzip,hashlib,io,json,subprocess,tarfile,tempfile,urllib.request
from pathlib import Path
repo='eclipse-theia/theia-ide/theia-ide';digest='sha256:85e2ffa998a6815b731cddda5b46e1fd8fc5d9857c02b318bfc2edda67d9b5e9'
parser=argparse.ArgumentParser(description='Fetch exact official ARM64 Theia image, verifying blob and layer hashes');parser.add_argument('--cache',type=Path);args=parser.parse_args()
root=args.cache or Path(tempfile.mkdtemp(prefix='verb-ide-theia-oci-'));root.mkdir(mode=0o700,exist_ok=True)
if root.is_symlink() or root.stat().st_mode&0o077:raise ValueError('private cache directory required')
with urllib.request.urlopen('https://ghcr.io/token?service=ghcr.io&scope=repository:'+repo+':pull',timeout=20) as r:token=json.load(r)['token']
def request(path):return urllib.request.Request('https://ghcr.io/v2/'+repo+'/'+path,headers={'Authorization':'Bearer '+token,'Accept':'application/vnd.oci.image.manifest.v1+json'})
with urllib.request.urlopen(request('manifests/'+digest),timeout=30) as r:raw=r.read()
assert 'sha256:'+hashlib.sha256(raw).hexdigest()==digest
manifest=json.loads(raw)
def download(item):
 h=item['digest'].split(':')[1];dest=root/h
 if dest.exists() and hashlib.sha256(dest.read_bytes()).hexdigest()==h:return dest
 sh=hashlib.sha256();size=0
 with urllib.request.urlopen(request('blobs/'+item['digest']),timeout=90) as r,dest.with_suffix('.part').open('wb') as out:
  while chunk:=r.read(1024*1024):out.write(chunk);sh.update(chunk);size+=len(chunk)
 assert size==item['size'] and sh.hexdigest()==h
 dest.with_suffix('.part').replace(dest);print('Verified blob',h[:12],size,flush=True);return dest
items=[manifest['config'],*manifest['layers']]
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:files=list(pool.map(download,items))
config=json.loads(files[0].read_text());assert config['architecture']=='arm64'
archive=root/'theia-docker-image.tar'
with tarfile.open(archive,'w') as tar:
 configName=manifest['config']['digest'].split(':')[1]+'.json';tar.add(files[0],arcname=configName)
 layerNames=[]
 for i,(item,path) in enumerate(zip(manifest['layers'],files[1:])):
  target=root/(path.name+'.tar');sh=hashlib.sha256()
  if not target.exists():
   with gzip.open(path,'rb') as src,target.open('wb') as out:
    while chunk:=src.read(1024*1024):out.write(chunk)
  with target.open('rb') as src:
   while chunk:=src.read(1024*1024):sh.update(chunk)
  assert 'sha256:'+sh.hexdigest()==config['rootfs']['diff_ids'][i]
  name=path.name+'/layer.tar';layerNames.append(name);tar.add(target,arcname=name)
 data=json.dumps([{'Config':configName,'RepoTags':['verb-ide-theia:1.76.0'],'Layers':layerNames}]).encode();info=tarfile.TarInfo('manifest.json');info.size=len(data);tar.addfile(info,io.BytesIO(data))
print('Importing verified image',flush=True)
subprocess.run(['docker','load','-i',str(archive)],check=True,timeout=240)
(root/'provenance.json').write_text(json.dumps({'upstream':'ghcr.io/'+repo,'manifestDigest':digest,'configDigest':manifest['config']['digest'],'tag':'1.76.0','arch':'arm64'},indent=2))
