import importlib.util,json,os,subprocess,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import lab,prepare

class OwnershipGuards(unittest.TestCase):
 def meta(self,label='verb-ide-lab-test',mounts=None,ip='127.0.0.1'):
  return {'Config':{'Labels':{lab.LABEL:label}},'Mounts':mounts or [],'HostConfig':{'PortBindings':{'8080/tcp':[{'HostIp':ip}]}}}
 def test_unrelated_container_cannot_be_operated(self):
  with patch.object(lab,'inspect',return_value=self.meta(label='someone-else')):
   with self.assertRaises(ValueError):lab.owned(Path('/tmp/verb-ide-lab-test'),{'containers':{'theia':{'id':'owned-id'}}},'theia')
 def test_host_mounts_are_refused(self):
  with patch.object(lab,'inspect',return_value=self.meta(mounts=[{'Source':'/a-real-project'}])):
   with self.assertRaises(ValueError):lab.owned(Path('/tmp/verb-ide-lab-test'),{'containers':{'theia':{'id':'owned-id'}}},'theia')
 def test_public_port_is_refused(self):
  with patch.object(lab,'inspect',return_value=self.meta(ip='0.0.0.0')):
   with self.assertRaises(ValueError):lab.owned(Path('/tmp/verb-ide-lab-test'),{'containers':{'theia':{'id':'owned-id'}}},'theia')
 def test_second_running_ide_is_refused_before_mutation(self):
  with patch.object(lab,'inspect',return_value={'State':{'Running':True}}),patch.object(lab,'docker') as docker:
   with self.assertRaises(ValueError):lab.start(Path('/tmp/verb-ide-lab-test'),{'containers':{'theia':{'id':'owned-id'}}},'code-server')
   docker.assert_not_called()
 def test_non_internal_network_is_refused(self):
  with patch.object(lab,'inspect',return_value={'Internal':False,'Labels':{lab.LABEL:'verb-ide-lab-test'}}),patch.object(lab,'docker') as docker:
   with self.assertRaises(ValueError):lab.start(Path('/tmp/verb-ide-lab-test'),{'containers':{},'network':'net-id'},'theia')
   docker.assert_not_called()
 def test_symlink_receipt_is_refused(self):
  with tempfile.TemporaryDirectory(prefix='verb-ide-lab-') as name:
   root=Path(name);target=root/'elsewhere';target.write_text('{}');(root/'receipt.json').symlink_to(target)
   with self.assertRaises(ValueError):lab.load(root)
 def test_world_readable_receipt_is_refused(self):
  with tempfile.TemporaryDirectory(prefix='verb-ide-lab-') as name:
   root=Path(name);p=root/'receipt.json';p.write_text('{}');p.chmod(0o644)
   with self.assertRaises(ValueError):lab.load(root)

class FailureCleanup(unittest.TestCase):
 def test_failed_proxy_start_cleans_only_the_started_trial(self):
  with tempfile.TemporaryDirectory(prefix='verb-ide-lab-') as name:
   root=Path(name);d={'containers':{},'network':'net'}
   label={lab.LABEL:root.name}
   def inspect(kind,identity):
    if kind=='network':return {'Internal':True,'Labels':label}
    if kind=='image':return {'Architecture':'arm64','Config':{'Labels':label},'Id':'image','Size':1}
    if identity=='c1':return {'Config':{'Labels':label},'Mounts':[],'Name':'/'+root.name+'-code-server','HostConfig':{'PortBindings':{}},'State':{'Running':False}}
    return {'Config':{'Labels':label},'Mounts':[],'HostConfig':{'ReadonlyRootfs':True},'State':{'Running':False}}
   def docker(*args,**kw):
    if args[:2]==('start','p1'):raise RuntimeError('proxy startup failed')
    if args[0]=='create':return 'p1' if '-proxy' in args[args.index('--name')+1] else 'c1'
    return ''
   with patch.object(lab,'inspect',side_effect=inspect),patch.object(lab,'docker',side_effect=docker),patch.object(lab,'stop') as stop:
    with self.assertRaises(RuntimeError):lab.start(root,d,'code-server')
    stop.assert_called_once_with(root,d,'code-server')
 def test_unrelated_proxy_is_rejected_before_any_stop(self):
  root=Path('/tmp/verb-ide-lab-test');d={'containers':{'theia':{'id':'c1','proxyId':'p1'}}}
  with patch.object(lab,'owned',return_value=(d['containers']['theia'],{'State':{'Running':True}})),patch.object(lab,'inspect',return_value={'Config':{'Labels':{lab.LABEL:'someone-else'}}}),patch.object(lab,'docker') as docker:
   with self.assertRaises(ValueError):lab.stop(root,d,'theia')
   docker.assert_not_called()

class FixtureContract(unittest.TestCase):
 def test_synthetic_project_has_no_remote_and_reproducible_conflict(self):
  with tempfile.TemporaryDirectory() as parent:
   root=prepare.prepare(parent);repo=root/'project'
   self.assertEqual(subprocess.check_output(['git','-C',str(repo),'remote']).strip(),b'')
   subprocess.run(['git','-C',str(repo),'switch','-q','conflict-left'],check=True)
   merge=subprocess.run(['git','-C',str(repo),'merge','conflict-right'],capture_output=True,text=True)
   self.assertNotEqual(merge.returncode,0)
   conflicts=subprocess.check_output(['git','-C',str(repo),'diff','--name-only','--diff-filter=U'],text=True).strip()
   self.assertEqual(conflicts,'config/theme.txt')
   settings=json.loads((repo/'.vscode/settings.json').read_text())
   self.assertNotIn('extensions.autoUpdate',settings)
   self.assertEqual(os.stat(root/'receipt.json').st_mode&0o777,0o600)
 def test_node_fixture_builds_and_tests_without_dependencies(self):
  with tempfile.TemporaryDirectory() as parent:
   root=prepare.prepare(parent);repo=root/'project'
   subprocess.run(['node','tools/build.cjs'],cwd=repo,check=True,capture_output=True)
   subprocess.run(['node','--test','test/app.test.cjs'],cwd=repo,check=True,capture_output=True)
   subprocess.run(['python3','-m','unittest','discover','-s','python'],cwd=repo,check=True,capture_output=True)
   self.assertTrue((repo/'dist/index.html').is_file())

if __name__=='__main__':unittest.main()
