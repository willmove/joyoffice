"""Exercise actual CLI/stdio MCP and authenticated loopback remote with operation deduplication."""
import argparse,json,subprocess,tempfile,time,socket,os,hashlib
from pathlib import Path
from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,default=binary('joydeck-cli', 'debug'));p.add_argument('--output',type=Path,default=EVIDENCE / 'm14/automation.json');a=p.parse_args();binary=str(a.binary.resolve())
def mcp(command):
 proc=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,bufsize=1, encoding='utf-8')
 def rpc(i,method,params={}):
  proc.stdin.write(json.dumps({'jsonrpc':'2.0','id':i,'method':method,'params':params})+'\n');proc.stdin.flush();v=json.loads(proc.stdout.readline());assert v['id']==i,v;return v
 try:
  assert rpc(0,'tools/list')['error']['code']==-32002
  assert rpc(1,'initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'joydeck-test','version':'1'}})['result']['protocolVersion']=='2025-11-25'
  proc.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');proc.stdin.flush()
  assert len(rpc(2,'tools/list')['result']['tools'])==1
  resources=rpc(3,'resources/list');assert resources['result']['resources'][0]['uri']=='joydeck://current/summary'
  def call(i,op,method,params={},target=None):
   args={'operationId':op,'method':method,'params':params}
   if target is not None:args['target']=target
   v=rpc(i,'tools/call',{'name':'joydeck_dispatch','arguments':args});return json.loads(v['result']['content'][0]['text'])
  commands=call(4,'describe','engine.commands');assert 'result' in commands
  # Read documentId from the supported summary resource, then use explicit target.
  summary=json.loads(rpc(5,'resources/read',{'uri':'joydeck://current/summary'})['result']['contents'][0]['text']);target=summary['documentId']
  first=call(6,'append-once','command.execute',{'command':'page.add'},target);assert 'result' in first,first
  assert call(7,'append-once','command.execute',{'command':'page.add'},target)==first
  assert call(8,'append-once','document.inspect',{},target)['error']['code']=='id-conflict'
  inspected=call(9,'inspect','document.inspect',{},target);assert len(inspected['result']['deck']['slides'])==2
  refreshed=json.loads(rpc(5,'resources/read',{'uri':'joydeck://current/summary'})['result']['contents'][0]['text']);assert refreshed['pages']==2,refreshed
  assert call(10,'dirty-new','document.new',{})['error']
  replacement=call(11,'replace-document','document.new',{'dirtyPolicy':'discard'})
  new_target=replacement['result']['documentId'];assert new_target!=target
  refreshed=json.loads(rpc(12,'resources/read',{'uri':'joydeck://current/summary'})['result']['contents'][0]['text'])
  assert refreshed['documentId']==new_target and refreshed['pages']==1,refreshed
  assert 'result' in call(13,'append-replacement','command.execute',{'command':'page.add'},new_target)
  return {'handshake':'2025-11-25','tools':1,'resources':1,'newTransportIdSameOperationIdReplayed':True,'idConflictRejected':True,'pagesAfterReplay':2,'dirtyNewRejected':True,'replacementDocumentSummary':True}
 finally:
  proc.stdin.close();proc.wait(timeout=8)
local=mcp([binary,'mcp'])
with tempfile.TemporaryDirectory(prefix='joydeck-control-verifier-')as temp:
 descriptor=Path(temp)/'control.json';server=subprocess.Popen([binary,'serve',str(descriptor)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True, encoding='utf-8')
 try:
  for _ in range(100):
   if descriptor.exists():break
   assert server.poll()is None,server.stderr.read();time.sleep(.05)
  credentials=json.loads(descriptor.read_text(encoding='utf-8'));host,port=credentials['address'].split(':')
  with socket.create_connection((host,int(port)),timeout=3)as s:
   s.sendall(b'{"token":"wrong","request":{}}\n');unauth=json.loads(s.makefile().readline());assert unauth['error']['code']=='unauthenticated'
  # Remote resources must describe the remote target, not the bridge's scratch model.
  remote=mcp([binary,'--connect',str(descriptor),'mcp'])
  commands=[{'method':'document.new','params':{'dirtyPolicy':'discard'}},{'method':'command.execute','params':{'command':'page.add'}},{'method':'document.inspect','params':{}}]
  result=json.loads(subprocess.check_output([binary,'--connect',str(descriptor),'run',json.dumps(commands)],text=True,encoding='utf-8'))
  assert result[2]['result']['documentId']==result[0]['result']['documentId'] and len(result[2]['result']['deck']['slides'])==2,result
  inspected=json.loads(subprocess.check_output([binary,'--connect',str(descriptor),'run',json.dumps([{'method':'document.inspect','params':{}}])],text=True,encoding='utf-8'))
  assert inspected[0]['result']['documentId']==result[0]['result']['documentId'] and len(inspected[0]['result']['deck']['slides'])==2,inspected
 finally:
  server.stdin.write('\n');server.stdin.flush();server.wait(timeout=8)
 assert not descriptor.exists()
result={'binarySha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'headless':local,'remote':remote,'remoteCliRunAfterReplacement':True,'freshClientFindsCurrentDocument':True,'unauthenticatedRejected':True,'descriptorRemovedOnShutdown':True,'passed':True}
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n', encoding='utf-8');print(json.dumps(result,indent=2))
