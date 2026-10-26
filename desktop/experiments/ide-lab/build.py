#!/usr/bin/env python3
"""Build only synthetic trial images; no host project mounts or credential copying."""
import argparse,json,subprocess
from pathlib import Path
from lab import load,save
BASES={'code-server':'codercom/code-server@sha256:377b66bec5645510ed52549bb00e648c273edc604d230a3f90fd2b2433afcb97','theia':'verb-ide-theia:1.76.0'}
def build(root,d,name):
 template='''FROM node@sha256:c3de60bf2f9dd0ac6370e6117950ff62d6e339527e7472301c9c78a017978392 AS tools
FROM {base}
USER root
RUN apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3 && rm -rf /var/lib/apt/lists/*
COPY --from=tools /usr/local/bin/node /usr/local/bin/node22
COPY --from=tools /usr/local/lib/node_modules/npm /opt/verb-lab-npm
RUN printf '#!/bin/sh\\nexec /usr/local/bin/node22 /opt/verb-lab-npm/bin/npm-cli.js "$@"\\n' > /usr/local/bin/npm22 && chmod 755 /usr/local/bin/npm22
COPY --chown=1000:1000 project /home/project
RUN mkdir -p /home/verb-lab/data/User /home/verb-lab/extensions && chown -R 1000:1000 /home/verb-lab /home/project
COPY --chown=1000:1000 user-settings.json /home/verb-lab/data/User/settings.json
ENV HOME=/home/verb-lab SHELL=/bin/bash DO_NOT_TRACK=1 DISABLE_TELEMETRY=1 PYTHONDONTWRITEBYTECODE=1
{extra}
USER 1000:1000
WORKDIR /home/project
LABEL com.verb.ide-lab="{lab}"
'''
 extra='COPY verb-lab-probe-0.0.1.vsix /tmp/probe.vsix\nRUN /usr/bin/code-server --user-data-dir /home/verb-lab/data --extensions-dir /home/verb-lab/extensions --install-extension /tmp/probe.vsix && chown -R 1000:1000 /home/verb-lab' if name=='code-server' else 'COPY verb-lab-probe-0.0.1.vsix /home/theia/plugins/verb-lab-probe.vsix'
 (root/'user-settings.json').write_text(json.dumps({'telemetry.telemetryLevel':'off','update.mode':'none','extensions.autoUpdate':False,'extensions.autoCheckUpdates':False,'workbench.startupEditor':'none'}))
 path=root/('Dockerfile.'+name);path.write_text(template.format(base=BASES[name],extra=extra,lab=root.name))
 (root/'.dockerignore').write_text('**\n!project/\n!project/**\n!verb-lab-probe-0.0.1.vsix\n!user-settings.json\n!Dockerfile.*\n')
 tag='verb-ide-lab-'+name+':'+root.name
 subprocess.run(['docker','build','-f',str(path),'-t',tag,str(root)],check=True,timeout=600)
 d.setdefault('images',{})[name]=tag
 if not d['images'].get('proxy'):
  proxy='verb-ide-lab-proxy:'+root.name
  subprocess.run(['docker','build','--build-arg','LAB_ID='+root.name,'-f',str(Path(__file__).with_name('Dockerfile.proxy')),'-t',proxy,str(Path(__file__).parent)],check=True,timeout=180)
  d['images']['proxy']=proxy
 save(root,d)
 return tag
if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('candidate',choices=BASES);parser.add_argument('--root',required=True);a=parser.parse_args();root,d=load(a.root);print(build(root,d,a.candidate))
