"""Recover incomplete 50-root shards as independent ten-root jobs, preserving frozen inputs."""
from pathlib import Path
import argparse,json,os,subprocess,sys,shutil
import evaluate as e
R=Path(__file__).resolve().parents[1]
def plan():
 info=e.read(R/'bundle/build.json');req=e.request(R/'bundle/request.json');matrix=[]
 for i in range(6):
  p=R/f'recovered/shard-{i}/report.json'
  good=False
  if p.exists():
   d=e.read(p)
   if d['build']==info and d['request']==req:
    e.validate(d['roots'],{int(k):v for k,v in d['rounds'].items()},req['seeds'][i*50:(i+1)*50]);good=True
  matrix.extend([dict(shard=i,part=-1)] if good else [dict(shard=i,part=j)for j in range(5)])
 with open(os.environ['GITHUB_OUTPUT'],'a')as f:f.write('matrix='+json.dumps(dict(include=matrix))+'\n')
def part(i,j):
 bundle=R/'bundle';info=e.read(bundle/'build.json');req=e.request(bundle/'request.json')
 for name,digest in info['files'].items():
  if e.digest(bundle/name)!=digest:raise ValueError('frozen bundle mismatch '+name)
 seeds=req['seeds'][i*50+j*10:i*50+(j+1)*10];folder=R/'out';folder.mkdir(exist_ok=False);sf=folder/'seeds.txt';sf.write_text('\n'.join(map(str,seeds)),encoding='utf-8')
 (R/'RustVsZombies').mkdir(exist_ok=True)
 cmd=[sys.executable,str(bundle/'run.py'),'--runner',str(bundle/'runner.exe'),'--output',str(folder/'run'),'--seed-file',str(sf),'--trials','10','--workers','4','--rounds','0','--trace','0','--detail','0','--policy','rule']
 for k,v in e.read(bundle/'parameters.json').items():cmd+=['--env',f'{k}={v}']
 subprocess.run(cmd,cwd=R,check=True)
 roots=e.read(folder/'run/results.json')['roots'];rounds=e.parse_rounds(folder/'run');e.validate(roots,rounds,seeds)
 e.save(folder/'report.json',dict(shard=i,part=j,build=info,request=req,roots=roots,rounds=rounds,timing=e.read(folder/'run/timing.json')))
def merge():
 reports=[e.read(p)for p in (R/'recovered').rglob('report.json')];out=R/'outputs';out.mkdir(exist_ok=True)
 for i in range(6):
  group=[d for d in reports if d['shard']==i];first=group[0]
  expected=list(range(5)) if any('part'in d for d in group)else[-1]
  if sorted(d.get('part',-1)for d in group)!=expected:raise ValueError('incomplete/duplicate recovery parts')
  roots=[];rounds={}
  for d in group:
   if d['build']!=first['build']or d['request']!=first['request']:raise ValueError('mixed frozen inputs')
   roots+=d['roots']
   for k,v in d['rounds'].items():
    if int(k)in rounds:raise ValueError('duplicate root')
    rounds[int(k)]=v
  e.validate(roots,rounds,first['request']['seeds'][i*50:(i+1)*50])
  folder=out/f'shard-{i}';folder.mkdir(exist_ok=True)
  e.save(folder/'report.json',dict(shard=i,build=first['build'],request=first['request'],roots=roots,rounds=rounds,timing={'seconds':max(d['timing']['seconds']for d in group)},recovery_parts=len(group)))
 e.summarize(None)
p=argparse.ArgumentParser();p.add_argument('mode',choices=['plan','part','merge']);p.add_argument('--shard',type=int);p.add_argument('--part',type=int);a=p.parse_args()
{'plan':plan,'part':lambda:part(a.shard,a.part),'merge':merge}[a.mode]()
