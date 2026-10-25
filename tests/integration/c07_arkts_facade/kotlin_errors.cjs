// Execute the actual generated facade with a native-port double. No target claim.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const {stripTypeScriptTypes} = require('node:module');
const source = fs.readFileSync(process.argv[2], 'utf8');
const frame = fs.readFileSync(process.argv[3], 'utf8');
const frameJs = stripTypeScriptTypes(frame.replace(/\bexport /g,'').replace(/enum Frame \{([\s\S]*?)\}/,
  (_whole, values) => 'const Frame = {' + values.replace(/ = /g, ': ') + '};'), {mode:'strip'});
const js = stripTypeScriptTypes(source.replace(/^import .*\n/gm, '')
  .replace('@MADB_EXPECTED_IDENTITY@', '"host-regression"')
  .replace(/\bexport /g, ''), {mode:'strip'});
let mode = 'ok', calls = 0, publicationClosed = false, received;
const nativeFailure = Object.assign(new Error('native failure'), {code:'Closing'});
const retained = Object.fromEntries(['peek','takeStatus','connect','abort'].map(name =>
  [name, () => {calls++; throw nativeFailure;} ]));
const native = new Proxy({}, {get(_target, name) {
  return (...args) => {
    calls++;
    if (name === 'runtimeOpen') return 'kn:1';
    if (mode === 'error') throw nativeFailure;
    if (name === 'saveCalls') return retained;
    if (name === 'pollRuntime') {const frame = new Array(110).fill(0); frame[0] = 70501; return frame;}
    if (name === 'requestClose') {publicationClosed = true; return;}
    if (name === 'testPublication') {
      received = args[0];
      if (publicationClosed) throw nativeFailure;
      if (received === null || typeof received !== 'object' || Array.isArray(received))
        throw Object.assign(new Error('record type'), {code:'ERR_RECORD_TYPE'});
      if (!Number.isInteger(received.value) || received.value < -2147483648 || received.value > 2147483647)
        throw Object.assign(new Error('record range'), {code:'ERR_RECORD_RANGE'});
      if (typeof received.text !== 'string')
        throw Object.assign(new Error('text type'), {code:'ERR_RECORD_TYPE'});
    }
    return [];
  };
}});
const context = vm.createContext({native, setTimeout:() => 1, clearTimeout:() => {}});
vm.runInContext(frameJs + '\n' + js + '\nthis.api = {BoundaryError, Session, RuntimeHandle, EngineHandle, OperationHandle, SubscriptionHandle, ResourceRecord, ' +
  'testObserve,testInspect,testSaveCalls,testObserveNapi,testRawFrame,testLastAdmissionAudit,testPollAdmission,' +
  'testArmPartialOpenFailure,testArmWork,testReleaseWork,testHoldPublication,testReleasePublication,testMemoryFrame,' +
  'testControlFrame,testArmDelivery,testDeliveryFrame,testReleaseDelivery,testStagePublication,testFinishPublication,' +
  'testArmCloneFailure,testArmTransferFailure,testEngineHandle,testOperationHandle,testSubscriptionHandle};', context);
const a = context.api;
const session = new a.Session();
const runtime = new a.RuntimeHandle(session);
const engine = new a.EngineHandle(session, new a.ResourceRecord('engine', 'engine', 0));
const operation = new a.OperationHandle(session, new a.ResourceRecord('operation', 'engine', 1));
const subscription = new a.SubscriptionHandle(session, new a.ResourceRecord('subscription', 'engine', 2));
const invocations = [
  ['testObserve',[runtime]], ['testInspect',['kn:1']], ['testSaveCalls',[runtime]],
  ['testObserveNapi',[]], ['testRawFrame',[]], ['testLastAdmissionAudit',[]], ['testPollAdmission',[]],
  ['testArmPartialOpenFailure',[1]], ['testArmWork',[engine,1]], ['testReleaseWork',[engine]],
  ['testHoldPublication',[engine]], ['testReleasePublication',[engine]], ['testMemoryFrame',[]],
  ['testControlFrame',[runtime]], ['testArmDelivery',[subscription]], ['testDeliveryFrame',[subscription]],
  ['testReleaseDelivery',[subscription]], ['testStagePublication',[engine,0,{value:1,text:'x'}]],
  ['testFinishPublication',[engine,true]], ['testArmCloneFailure',[engine,1,true]],
  ['testArmTransferFailure',[runtime,1]], ['testEngineHandle',[engine,0]],
  ['testOperationHandle',[operation,0]], ['testSubscriptionHandle',[subscription,0]],
];
const isBoundary = error => error instanceof a.BoundaryError && error.code === 'Closing';
mode = 'error';
for (const [name,args] of invocations) assert.throws(() => a[name](...args), isBoundary, name);
mode = 'ok';
const saved = a.testSaveCalls(runtime);
for (const name of ['peek','takeStatus','connect','abort'])
  assert.throws(() => saved[name]('kn:1','1'), isBoundary, 'SavedCalls.' + name);

let valueReads = 0, textReads = 0, codeReads = 0;
const userFailure = new Error('user getter');
Object.defineProperty(userFailure, 'code', {get() {codeReads++; return 'Closing';}});
function throwingSnapshot(field) {
  return {get value() {valueReads++; if(field === 'value') throw userFailure; return 1;},
          get text() {textReads++; if(field === 'text') throw userFailure; return 'x';}};
}
for (const field of ['value','text']) {
  valueReads = textReads = codeReads = 0; const before = calls;
  assert.throws(() => a.testStagePublication(engine,0,throwingSnapshot(field)), error => error === userFailure);
  assert.equal(valueReads,1); assert.equal(textReads,field === 'text' ? 1 : 0);
  assert.equal(codeReads,0); assert.equal(calls,before);
}
valueReads = textReads = 0;
assert.throws(() => a.testStagePublication({},0,throwingSnapshot('value')),
  error => error instanceof a.BoundaryError && error.code === 'InvalidArgument');
assert.equal(valueReads,0); assert.equal(textReads,0);
const getters = {get value() {valueReads++; return 7;},get text() {textReads++; return 'own';}};
a.testStagePublication(engine,0,getters);
assert.equal(valueReads,1); assert.equal(textReads,1); assert.notEqual(received,getters);
for (const name of ['value','text']) {
  const descriptor = Object.getOwnPropertyDescriptor(received,name);
  assert.ok(Object.hasOwn(descriptor,'value')); assert.equal(descriptor.get,undefined);
}
for (const invalid of [null,undefined,1,'text',true,[],Object.assign([],{value:1,text:'array'})]) {
  assert.throws(() => a.testStagePublication(engine,0,invalid),
    error => error instanceof a.BoundaryError && error.code === 'ERR_RECORD_TYPE');
  assert.equal(received,invalid,'container rejection still uses original native codec input');
}
for (const invalid of [{value:0.5,text:'range'},{value:2147483648,text:'range'},{value:1,text:17}]) {
  assert.throws(() => a.testStagePublication(engine,0,invalid),
    error => error instanceof a.BoundaryError && ['ERR_RECORD_TYPE','ERR_RECORD_RANGE'].includes(error.code));
  assert.equal(received.value,invalid.value); assert.equal(received.text,invalid.text);
}
valueReads = textReads = 0;
const reentrant = {get value() {valueReads++; engine.requestClose(); return 1;},
  get text() {textReads++; return 'closed';}};
assert.throws(() => a.testStagePublication(engine,0,reentrant), isBoundary);
assert.equal(valueReads,1); assert.equal(textReads,1);
codeReads = 0;
session.testSettleProbe(() => {throw userFailure;}, false);
assert.throws(() => session.afterSettler(true), error => error === userFailure);
assert.equal(codeReads,0); assert.equal(session.testSettleInspect()[4],1);
console.log(JSON.stringify({native_helpers:invocations.length,saved_calls:4,getter_identity:true,
  brand_before_getters:true,getters_once:true,reentrant_close:true,settler_identity:true,container_validation:true,field_validation:true,
  scope:'actual generated JS facade with native-port double; no KSP/NAPI/target claim'}));
