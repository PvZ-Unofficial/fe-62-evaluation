from pathlib import Path
import argparse,json,os,subprocess,sys
import evaluate as e
R=Path(__file__).resolve().parents[1]

def matrix():
 selected=e.read(R/'recovery-inputs/selection.json')['shards']
 if len(set(selected))!=len(selected) or any(type(i)is not int or not 0<=i<6 for i in selected):raise ValueError('invalid source shard selection')
 value=json.dumps({'include':[dict(shard=i,part=j)for i in selected for j in range(10)]})
 with open(os.environ['GITHUB_OUTPUT'],'a',encoding='utf-8')as f:f.write('matrix='+value+'\n')

def part(i,j):
 if not 0<=i<6 or not 0<=j<10:raise ValueError('invalid part')
 bundle=R/'bundle';info=e.read(bundle/'build.json');req=e.request(bundle/'request.json')
 for name,sha in info['files'].items():
  if e.digest(bundle/name)!=sha:raise ValueError('frozen input mismatch '+name)
 seeds=req['seeds'][i*50+j*5:i*50+(j+1)*5];assert len(seeds)==5
 folder=R/'out';folder.mkdir(exist_ok=False);sf=folder/'seeds.txt';sf.write_text('\n'.join(map(str,seeds)),encoding='utf-8');(R/'RustVsZombies').mkdir(exist_ok=True)
 cmd=[sys.executable,str(bundle/'run.py'),'--runner',str(bundle/'runner.exe'),'--output',str(folder/'run'),'--seed-file',str(sf),'--trials','5','--workers','4','--rounds','0','--trace','0','--detail','0','--policy','rule']
 for k,v in e.read(bundle/'parameters.json').items():cmd+=['--env',f'{k}={v}']
 subprocess.run(cmd,cwd=R,check=True)
 roots=e.read(folder/'run/results.json')['roots'];rounds=e.parse_rounds(folder/'run');e.validate(roots,rounds,seeds)
 e.save(folder/'report.json',dict(shard=i,part=j,build=info,request=req,roots=roots,rounds=rounds,timing=e.read(folder/'run/timing.json')))

def merge():
 reports=[e.read(p)for p in (R/'recovered').rglob('report.json')]
 for i in range(6):
  group=[d for d in reports if d['shard']==i]
  if not group:raise ValueError('missing original shard')
  first=group[0];expected=list(range(10))if any('part'in d for d in group)else[-1]
  if sorted(d.get('part',-1)for d in group)!=expected:raise ValueError('duplicate or incomplete parts')
  roots=[];rounds={}
  for d in group:
   if d['build']!=first['build'] or d['request']!=first['request']:raise ValueError('mixed frozen inputs')
   seeds=d['request']['seeds'][i*50:(i+1)*50] if 'part'not in d else d['request']['seeds'][i*50+d['part']*5:i*50+(d['part']+1)*5]
   rr={int(k):v for k,v in d['rounds'].items()};e.validate(d['roots'],rr,seeds)
   if rounds.keys() & rr.keys():raise ValueError('duplicate roots')
   roots.extend(d['roots']);rounds.update(rr)
  e.validate(roots,rounds,first['request']['seeds'][i*50:(i+1)*50])
  dest=R/'outputs'/f'shard-{i}';dest.mkdir(parents=True,exist_ok=True)
  e.save(dest/'report.json',dict(shard=i,build=first['build'],request=first['request'],roots=roots,rounds=rounds,timing=dict(seconds=max(d['timing']['seconds']for d in group))))
 e.ROOT=R;e.summarize(None)

if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('mode',choices=['matrix','part','merge']);p.add_argument('--shard',type=int);p.add_argument('--part',type=int);a=p.parse_args()
 {'matrix':matrix,'part':lambda:part(a.shard,a.part),'merge':merge}[a.mode]()
