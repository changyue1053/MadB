"""Bounded process execution and frozen-file primitives for C06 workers."""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import selectors
import subprocess
import time

LOG_PATH = 'logs/command.log'
MAX_JSON = 1024 * 1024

def canonical(value):
    return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False).encode()

def decode(data):
    def pairs(items):
        result={}
        for k,v in items:
            if k in result:raise ValueError('duplicate JSON member: '+k)
            result[k]=v
        return result
    if len(data)>MAX_JSON:raise ValueError('control document exceeds 1 MiB')
    return json.loads(data,object_pairs_hook=pairs)

def load(path):return decode(Path(path).read_bytes())

def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return 'sha256:'+h.hexdigest()

def file_ref(path,root):
    return dict(path=path.relative_to(root).as_posix(),sha256=sha(path),size=path.stat().st_size)

def input_path(root,name):
    if not isinstance(name,str) or not name or Path(name).is_absolute() or any(p in ('','.','..') for p in name.split('/')):
        raise ValueError('expected logical input path')
    p=root
    for part in name.split('/'):
        p=p/part
        if p.is_symlink():raise ValueError('input symlink rejected: '+name)
    if not p.is_file() or not p.resolve().is_relative_to(root):raise ValueError('missing frozen input: '+name)
    return p

def copy_file(source,destination):
    destination.parent.mkdir(parents=True,exist_ok=True)
    destination.write_bytes(source.read_bytes())
    destination.chmod(0o644)

class Commands:
    def __init__(self,config,inputs,work,scratch):
        self.config,self.inputs,self.work,self.scratch=config,inputs,work,scratch
        self.observation=config['toolchain']
        self.images=self.observation['tools']
        self.log=work/LOG_PATH;self.log.parent.mkdir(parents=True,exist_ok=True);self.log.write_bytes(b'')
        self.prefixes=[(str(scratch),'$SCRATCH'),(str(inputs),'$INPUTS'),(str(work),'$WORK')]
        self.environment={'PATH':str(Path(self.images['cargo']['path']).parent)+':/usr/bin:/bin:/usr/sbin:/sbin',
            'LC_ALL':'C','LANG':'C','ZERO_AR_DATE':'1','SOURCE_DATE_EPOCH':'0','TMPDIR':str(scratch),
            'SDKROOT':self.observation['sdk'],'MACOSX_DEPLOYMENT_TARGET':config.get('deployment_target','14.0')}
        self.environment.update(self.observation['environment'])
        self.environment['RUSTC']=self.images['rustc']['path']
    def normalize(self,text):
        for path,name in self.prefixes:text=text.replace(path,name)
        return text
    def run(self,name,args,*,cwd=None,extra=None,executable=None):
        path=Path(executable or self.images[name]['path'])
        command=[str(path),*map(str,args)]
        env=dict(self.environment);env.update(extra or {})
        buffers={'stdout':bytearray(),'stderr':bytearray()}
        process=subprocess.Popen(command,cwd=cwd or self.scratch,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        failure=None;code=-1
        try:
            deadline=time.monotonic()+(90 if executable else 600)
            with selectors.DefaultSelector() as s:
                s.register(process.stdout,selectors.EVENT_READ,'stdout');s.register(process.stderr,selectors.EVENT_READ,'stderr')
                while s.get_map():
                    if time.monotonic()>deadline:raise RuntimeError(name+' timed out')
                    for key,_ in s.select(.1):
                        b=os.read(key.fileobj.fileno(),65536)
                        if not b:s.unregister(key.fileobj);continue
                        if len(buffers[key.data])+len(b)>1024*1024:raise RuntimeError(name+' exceeded output budget')
                        buffers[key.data].extend(b)
            code=process.wait(timeout=max(.1,deadline-time.monotonic()))
        except Exception as e:failure=e
        finally:
            if process.poll() is None:process.kill()
            process.stdout.close();process.stderr.close();code=process.wait(timeout=5)
        record=dict(command=[self.normalize(x) for x in command],exit_code=code,
                    stdout=self.normalize(buffers['stdout'].decode(errors='replace')),stderr=self.normalize(buffers['stderr'].decode(errors='replace')))
        data=canonical(record)+b'\n'
        if self.log.stat().st_size+len(data)>4*1024*1024:raise RuntimeError('C06 log exceeded 4 MiB')
        with self.log.open('ab') as f:f.write(data)
        if failure:raise RuntimeError(str(failure)) from failure
        if code:raise RuntimeError(name+' failed: '+record['stderr'][-5000:])
        return bytes(buffers['stdout'])
