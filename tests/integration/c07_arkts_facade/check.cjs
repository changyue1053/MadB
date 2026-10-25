// Independent JS facade oracle. The native layer below is a controllable model,
// never an implementation of NAPI, real resource release or target certification.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const {stripTypeScriptTypes} = require('node:module');
const root = path.resolve(__dirname, '../../..');
const sdk = path.join(root, 'sdk/arkts/facade');
const sourcePath = process.argv[2] || path.join(sdk, 'Boundary.ets.in');
const legacy = process.argv.includes('--legacy-native');
const source = fs.readFileSync(sourcePath, 'utf8');
const framePath = path.join(sdk, 'RuntimeFrame.ets');
const frameSource = fs.readFileSync(framePath, 'utf8');
const layoutPath = path.join(sdk, 'frame-layout.json');
const layoutBytes = fs.readFileSync(layoutPath);
const layout = JSON.parse(layoutBytes);
const frameMatch = frameSource.match(/export enum Frame \{([\s\S]*?)\}/);
assert.ok(frameMatch, 'actual private frame enum is required');
const entries = frameMatch[1].split(',').map(v => v.trim()).filter(Boolean);
const extracted = {};
for (const entry of entries) {
  const match = entry.match(/^([A-Za-z_][A-Za-z0-9_]*) = (0|[1-9][0-9]*)$/);
  assert.ok(match, 'unsupported frame enum syntax');
  assert.equal(Object.hasOwn(extracted, match[2]), false, 'duplicate frame index');
  extracted[match[2]] = match[1];
}
assert.deepEqual(extracted, layout, 'ArkTS frame differs from its frozen native layout');
assert.deepEqual(Object.keys(layout), Array.from({length:54}, (_, i) => String(i)));

const sha = data => crypto.createHash('sha256').update(data).digest('hex');
const js = stripTypeScriptTypes(source.replace(/^import .*\n/gm, '')
  .replace('@MADB_EXPECTED_IDENTITY@', '"independent-host-only"').replace(/\bexport /g,''), {mode:'strip'});
// The frame contains only explicitly numbered enum constants; its unused reverse
// mapping is omitted for Node's type-strip-only mode. Facade logic is unchanged.
const frameJs = stripTypeScriptTypes(frameSource.replace(/\bexport /g,'')
  .replace(/enum Frame \{([\s\S]*?)\}/, (_whole, values) => 'const Frame = {' + values.replace(/ = /g, ': ') + '};'), {mode:'strip'});
const error = code => Object.assign(new Error(code), {code});
function fixture() {
  const timers = new Map();
  const resources = new Map();
  const observations = new Map();
  const calls = {runtimeClose:0, receipt:0, request:0, claim:0, watch:0};
  let nextTimer = 0, sequence = 0, generation = 0, dispatch;
  let engine;
  const control = {timers, resources, observations, calls, failTimer:false,
    failClaim:false, holdOperationClose:false, resultReady:false,
    physical:false, runtimeClosing:false, failTake:'', failDelivery:0, holdJoinAck:false};
  const lookup = key => {const r=resources.get(key); if(!r || r.retired) throw error('InvalidHandleRetired');return r;};
  const closeTarget = r => {
    r.closing = true;
    if (r.kind === 's' || (r.kind === 'o' && !control.holdOperationClose)) r.closed=true;
    if (r.kind === 'e') for(const child of resources.values()) if(child.engine===r.handle) child.closing=true;
  };
  function ackClosed() {
    // The reviewed private contract emits child ACKs before the Engine ACK;
    // this prevents parent retirement from erasing a child's failure outcome.
    const ordered=[...resources.values()].sort((a,b)=>(a.kind==='e')-(b.kind==='e'));
    for (const r of ordered) {
      if(!r.closed || r.acked || (r.kind==='e' && control.holdJoinAck)) continue;
      if(!legacy) dispatch(2,r.handle,{status:r.status,value:0,text:''});
      r.acked=true;
      if(r.kind==='e') r.retired=true;
    }
  }
  control.finishBusiness = () => {
    for (const r of resources.values()) if(r.closing) r.closed=true;
    // Old native afterJoin can retire an unobserved Engine before a JS close retry.
    if(legacy && engine?.closing && observations.size===0) engine.retired=true;
  };
  const native = {
    runtimeOpen(identity, fn) {assert.equal(identity,'independent-host-only');dispatch=fn;return 'r1';},
    connect() {
      if(engine && !engine.retired) throw error('Closing');
      const handle=`r1/g${++generation}/e`;
      engine={handle,engine:handle,kind:'e',closed:false,closing:false,acked:false,status:0};
      resources.set(handle,engine);return handle;
    },
    roundtrip(handle,value,text) {lookup(handle);if(control.failSync)throw control.failSync;return {status:0,value,text};},
    submit(handle,value,text) {
      lookup(handle);const key=`${handle}/o${++sequence}`;
      resources.set(key,{handle:key,engine:handle,kind:'o',value,text,closed:false,closing:false,claimed:false,acked:false,status:0});
      return key;
    },
    observe(handle,stream) {
      lookup(handle);const active=[...resources.values()].filter(r=>r.kind==='s'&&!r.closed).length;
      if(active>=8) throw error('ResourceExhausted');
      const key=`${handle}/s${++sequence}`;
      resources.set(key,{handle:key,engine:handle,kind:'s',stream,closed:false,closing:false,acked:false,status:0});return key;
    },
    publish(handle) {lookup(handle);},
    cancel(handle) {lookup(handle);},
    requestClose(handle) {calls.request++;closeTarget(lookup(handle));},
    claimResult(handle,id) {
      calls.claim++;const r=lookup(handle);
      if(control.failClaim) {control.failClaim=false;throw error('ResourceExhausted');}
      if(r.claimed) throw error('AlreadyConsumed');
      if(observations.size>=32) throw error('ResourceExhausted');
      r.claimed=true;observations.set(id,{record:r,result:true});
    },
    watchClose(handle,id) {
      calls.watch++;const r=lookup(handle);closeTarget(r);
      if(r.closed) return r.status;
      if(observations.size>=32) throw error('ResourceExhausted');
      observations.set(id,{record:r,result:false});return -1;
    },
    tryTake(runtime,id) {
      const item=observations.get(id);if(!item)throw error('TakeBeforeGuardResidual');
      if(control.failTake==='before') throw error('TakeBeforeGuardResidual');
      if(item.result ? !control.resultReady : !item.record.closed) return null;
      observations.delete(id);
      if(control.failTake==='consumed') throw error('DeliveryFailureConsumed');
      return item.result ? {status:0,value:item.record.value+1,text:item.record.text} : {status:item.record.status,value:0,text:''};
    },
    deliver(handle) {
      const r=lookup(handle);
      if(control.failDelivery) {
        control.failDelivery=0;
        try {dispatch(1,handle,{status:0,value:1,text:'state'});} catch (_) {r.closed=true;r.closing=true;r.status=9;return 9;}
        return 0;
      }
      return r.closed ? 4 : 7;
    },
    requestRuntimeClose() {
      calls.runtimeClose++;control.runtimeClosing=true;
      if(engine)closeTarget(engine);
    },
    pollRuntime() {
      ackClosed();const frame=new Array(54).fill(0);frame[0]=1;
      frame[1]=control.physical ? 3 : control.runtimeClosing ? 1 : 0;
      return frame;
    },
    consumeRuntimeReceipt() {
      assert.ok(control.physical,'must not consume receipt before physical ACK');calls.receipt++;
      return native.pollRuntime();
    },
    audit() {return native.pollRuntime();},
  };
  // Instrument only the resolver entry to observe a synchronous reentrant
  // call while the SDK's JS settling permit must remain charged. This is a
  // host harness hook, not a claim that ArkTS users can replace intrinsics.
  class ProbedPromise extends Promise {
    static get [Symbol.species]() { return Promise; }
    constructor(executor) {
      super((resolve,reject)=>executor(value=>{
        if(control.onResolve) control.onResolve();
        resolve(value);
      },reject));
    }
  }
  const sandbox = {console,native,Promise:ProbedPromise,
    setTimeout(fn) {if(control.failTimer)throw error('TimerUnavailable');const id=++nextTimer;timers.set(id,fn);return id;},
    clearTimeout(id) {timers.delete(id);}};
  vm.createContext(sandbox);
  vm.runInContext(frameJs+'\n'+js+'\nglobalThis.open = openRuntime; globalThis.ExportedBoundaryError = BoundaryError;',sandbox);
  const runtime=sandbox.open();
  control.BoundaryError=sandbox.ExportedBoundaryError;
  control.runtime=runtime;control.engine=()=>runtime.connect();
  control.tick=async()=>{const first=timers.entries().next();if(!first.done){const[id,fn]=first.value;timers.delete(id);fn();}await Promise.resolve();};
  control.drain=async(n=6)=>{for(let i=0;i<n;i++)await control.tick();};
  return control;
}
function track(promise) {
  const state={state:'pending',value:undefined};
  promise.then(value=>{state.state='resolved';state.value=value;},reason=>{state.state='rejected';state.value=reason;});
  return state;
}
async function rejected(promise,code) {
  let reason;
  try {await promise;} catch(e) {reason=e;}
  assert.ok(reason,'expected rejection '+code);assert.equal(reason.code,code);
}
const tests = {
  async engine_pending_close_waits_for_join_ack_after_core_receipt() {
    const f=fixture(),e=f.engine();f.holdJoinAck=true;
    const result=track(e.close());assert.equal(f.observations.size,1);
    f.finishBusiness();await f.drain();
    assert.equal(f.observations.size,0,'Core token is actually consumed');
    assert.equal(result.state,'pending','Core.closed alone must not resolve Engine.close');
    assert.equal(f.timers.size,1,'same JS waiter retains progress');
    assert.throws(()=>f.engine(),reason=>reason.code==='Closing');
    f.holdJoinAck=false;await f.drain();assert.equal(result.state,'resolved');
    assert.equal(f.timers.size,0);f.engine();
  },
  async engine_immediate_receipts_keep_32_js_owners_until_join_ack() {
    const f=fixture(),e=f.engine();f.holdJoinAck=true;
    e.requestClose();f.finishBusiness();
    const pending=Array.from({length:32},()=>track(e.close()));
    await f.drain();assert.equal(f.observations.size,0,'immediate Core receipts own no native waiter');
    assert.ok(pending.every(x=>x.state==='pending'));
    const watched=f.calls.watch;
    await rejected(e.close(),'ResourceExhausted');assert.equal(f.calls.watch,watched);
    let reentry=0;
    f.onResolve=()=>{f.onResolve=undefined;reentry++;assert.throws(()=>f.engine(),reason=>reason.code==='Closing');};
    f.holdJoinAck=false;await f.drain();
    assert.equal(reentry,1);assert.ok(pending.every(x=>x.state==='resolved'));
    assert.equal(f.timers.size,0);f.engine();await e.close();
  },
  async engine_close_deferred_error_is_delivered_once_after_ack() {
    const f=fixture(),e=f.engine();f.holdJoinAck=true;
    const result=track(e.close());
    [...f.resources.values()].find(x=>x.kind==='e').status=9;
    f.finishBusiness();await f.drain();assert.equal(result.state,'pending');
    f.holdJoinAck=false;await f.drain();
    assert.equal(result.state,'rejected');assert.equal(result.value.code,'CallbackFailure');
    await rejected(e.close(),'CallbackFailure');f.engine();
  },
  async engine_consumed_copy_failure_remains_observable_before_join_ack() {
    const f=fixture(),e=f.engine();f.holdJoinAck=true;
    const result=track(e.close());f.finishBusiness();f.failTake='consumed';await f.drain();
    assert.equal(result.state,'rejected');assert.equal(result.value.code,'DeliveryFailureConsumed');
    assert.equal(f.observations.size,0);assert.equal(f.timers.size,1);
    f.holdJoinAck=false;await f.drain();f.engine();
  },
  async request_close_later_observe_and_reconnect() {
    const f=fixture(),e=f.engine();e.requestClose();f.finishBusiness();await f.drain();
    await e.close();const next=f.engine();assert.equal(next.roundtrip({value:2,text:'x'}).value,2);
    await e.close(); // old tombstone must not address a new generation
  },
  async request_stop_releases_bounded_listener_slot() {
    const f=fixture(),e=f.engine();
    for(let i=0;i<9;i++){const s=e.observeEvents(()=>{});s.requestStop();await f.drain();await s.close();}
    assert.equal([...f.resources.values()].filter(x=>x.kind==='s'&&!x.closed).length,0);
  },
  async stop_without_promise_reuses_capacity() {
    const f=fixture(),e=f.engine();
    for(let i=0;i<9;i++){e.observeEvents(()=>{}).requestStop();await f.drain();}
  },
  async full_waiter_close_requests_first_and_retries() {
    const f=fixture(),e=f.engine();f.holdOperationClose=true;
    const operation=e.startOperation({value:7,text:'held'});
    const pending=[];
    for(let i=0;i<32;i++)pending.push(track(operation.close()));
    assert.equal(f.observations.size,32);
    const requests=f.calls.request;
    await rejected(e.close(),'ResourceExhausted');assert.ok(f.calls.request>requests);
    f.finishBusiness();await f.drain();assert.ok(pending.every(x=>x.state==='resolved'));
    await e.close();f.engine();
  },
  async runtime_request_precedes_timer_failure() {
    const f=fixture();f.engine();f.failTimer=true;
    const result=track(f.runtime.close());await Promise.resolve();
    assert.equal(result.state,'rejected');assert.equal(f.calls.runtimeClose,1);
  },
  async runtime_waits_for_physical_ack_and_clears_timer() {
    const f=fixture();f.engine();const promise=f.runtime.close();
    assert.equal(f.runtime.close(),promise);const outcome=track(promise);
    f.finishBusiness();await f.drain();assert.equal(outcome.state,'pending');assert.equal(f.calls.receipt,0);
    f.physical=true;await f.drain();assert.equal(outcome.state,'resolved');
    assert.equal(f.calls.receipt,1);assert.equal(f.timers.size,0);
  },
  async result_single_claim_and_admission_failure_rollback() {
    const f=fixture(),e=f.engine(),op=e.startOperation({value:41,text:'answer'});
    f.failClaim=true;await rejected(op.result(),'ResourceExhausted');
    const result=track(op.result());await rejected(op.result(),'AlreadyConsumed');
    assert.equal(f.calls.claim,2);assert.equal(f.observations.size,1);
    f.resultReady=true;await f.drain();assert.equal(result.state,'resolved');assert.equal(result.value.value,42);
  },
  async settling_keeps_shared_32_admission_until_resolver_returns() {
    const f=fixture(),e=f.engine();f.holdOperationClose=true;
    const first=e.startOperation({value:41,text:'first'});
    const held=e.startOperation({value:1,text:'held'});
    const next=e.startOperation({value:2,text:'next'});
    const outcome=track(first.result());
    for(let i=0;i<31;i++)track(held.close());
    assert.equal(f.observations.size,32);
    let reentry;
    f.onResolve=()=>{f.onResolve=undefined;reentry=track(next.result());};
    f.resultReady=true;await f.tick();
    assert.equal(outcome.state,'resolved');assert.equal(reentry.state,'rejected');
    assert.equal(reentry.value.code,'ResourceExhausted');
    assert.equal(f.calls.claim,1,'reentry must be rejected before native second admission');
    const admitted=track(next.result());assert.equal(f.calls.claim,2);
    await f.tick();assert.equal(admitted.state,'resolved');
  },
  async listener_failure_tombstone_survives_reuse() {
    const f=fixture(),e=f.engine(),s=e.observeState(()=>{throw error('UserError');});
    f.failDelivery=1;await f.drain();await rejected(s.close(),'CallbackFailure');
    e.observeState(()=>{});await rejected(s.close(),'CallbackFailure');
  },
  async native_child_failure_survives_engine_generation_retirement() {
    const f=fixture(),e=f.engine(),s=e.observeState(()=>{});
    const nativeSub=[...f.resources.values()].find(r=>r.kind==='s');
    // A latched native callback/conversion failure must be read from its ACK,
    // not inferred from a retired handle or overwritten by parent close success.
    nativeSub.status=9;nativeSub.closing=true;nativeSub.closed=true;
    e.requestClose();f.finishBusiness();await f.drain();await e.close();f.engine();
    await rejected(s.close(),'CallbackFailure');
  },
  async consumed_conversion_failure_rejects_once() {
    const f=fixture(),e=f.engine(),op=e.startOperation({value:5,text:'data'});
    const outcome=track(op.result());f.resultReady=true;f.failTake='consumed';await f.drain();
    assert.equal(outcome.state,'rejected');assert.equal(outcome.value.code,'DeliveryFailureConsumed');
    assert.equal(f.observations.size,0);await rejected(op.result(),'AlreadyConsumed');
  },
  async preguard_failure_preserves_native_residual_and_fails_closed() {
    const f=fixture(),e=f.engine(),op=e.startOperation({value:5,text:'data'});
    const outcome=track(op.result());f.resultReady=true;f.failTake='before';await f.drain();
    assert.equal(outcome.state,'rejected');assert.equal(outcome.value.code,'BridgeFailure');
    assert.equal(f.observations.size,1);assert.equal(f.timers.size,0);
    await rejected(f.runtime.close(),'BridgeFailure');
  },
  async synchronous_native_error_uses_exported_error_type() {
    const f=fixture(),e=f.engine();f.failSync=error('InvalidArgument');
    let caught;
    try {e.roundtrip({value:0,text:''});} catch(reason) {caught=reason;}
    assert.ok(caught instanceof f.BoundaryError,'raw native Error must use the exported SDK error type');
    assert.equal(caught.code,'InvalidArgument');
  },
  async user_getter_error_is_preserved_by_identity() {
    const f=fixture(),e=f.engine();const own=error('InvalidArgument');
    let caught;
    try {e.roundtrip({get value(){throw own;},text:''});} catch(reason) {caught=reason;}
    assert.equal(caught,own,'SDK must not normalize user property errors even if they contain code');
  },
  async getter_reentry_cannot_admit_after_close() {
    const f=fixture(),e=f.engine();
    assert.throws(()=>e.startOperation({get value(){e.requestClose();return 1;},text:''}),e=>e.code==='Closing');
    assert.equal([...f.resources.values()].filter(x=>x.kind==='o').length,0);
  },
};
(async()=>{
  const cases=[];
  for(const[name,test]of Object.entries(tests)){
    try{await test();cases.push({name,passed:true});console.log('PASS',name);}
    catch(e){cases.push({name,passed:false,error:String(e),stack:e.stack});console.log('FAIL',name,String(e));}
  }
  const output=path.join(fs.mkdtempSync(path.join(require('node:os').tmpdir(), 'madb-c07-facade-check-')), 'receipt.json');
  fs.writeFileSync(output,JSON.stringify({scope:'Independent host JS facade checks with simulated native API; no NAPI/OHOS/target proof',node:process.version,source:{path:sourcePath,sha256:sha(source)},frame:{path:framePath,sha256:sha(frameSource)},test:{path:__filename,sha256:sha(fs.readFileSync(__filename))},layout:{path:layoutPath,sha256:sha(layoutBytes)},legacy_native:legacy,cases,passed:cases.every(x=>x.passed)},null,2)+'\n');
  console.log('receipt',output);if(cases.some(x=>!x.passed))process.exitCode=1;
})().catch(e=>{console.error(e);process.exitCode=1;});
