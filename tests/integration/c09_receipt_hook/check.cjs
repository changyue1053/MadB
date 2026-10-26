'use strict';
// Executes generated facade logic. Native frames and scheduling below are explicit
// host inputs, never evidence of an actual native ACK, HAR build or target run.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const {stripTypeScriptTypes} = require('node:module');
const {readFacade} = require('../../../adapters/generators/arkts/read_facade.cjs');
const [sourcePath, framePath, route, compiler] = process.argv.slice(2);
const ts = require(compiler);
const source = fs.readFileSync(sourcePath, 'utf8');
const actual = source.replace('@MADB_EXPECTED_IDENTITY@', '"host-only"');
const frameSource = fs.readFileSync(framePath, 'utf8');
const frameJs = stripTypeScriptTypes(frameSource.replace(/\bexport /g, '')
  .replace(/enum Frame \{([\s\S]*?)\}/, (_, values) => 'const Frame = {' + values.replace(/ = /g, ': ') + '};'), {mode:'strip'});
const js = stripTypeScriptTypes(actual.replace(/^import .*\n/gm, '').replace(/\bexport /g, ''), {mode:'strip'});
const nativeError = code => Object.assign(new Error(code), {code});
function fixture() {
  const timers = new Map();
  const state = {physical:false, receipt:0, occupied:false, failPoll:false, timer:0};
  function frame() {
    if (state.failPoll) throw nativeError('PollUnavailable');
    const values = new Array(route === 'kotlin' ? 110 : 54).fill(0);
    if (route === 'kotlin') {
      values[0] = 70501; values[1] = state.physical ? 4 : 2; values[7] = 1;
    } else { values[0] = 1; values[1] = state.physical ? 3 : 1; }
    return values;
  }
  const native = {
    runtimeOpen() {
      if (state.occupied) throw nativeError(route === 'kotlin' ? 'AlreadyInitialized' : 'RuntimeAlreadyOpen');
      state.occupied = true; return 'r1';
    },
    requestRuntimeClose() {},
    pollRuntime: frame,
    consumeRuntimeReceipt() {
      assert.equal(state.physical, true); state.receipt++; state.occupied = false; return frame();
    },
  };
  const sandbox = {native, Promise,
    setTimeout(fn) {const key = ++state.timer; timers.set(key, fn); return key;},
    clearTimeout(key) {timers.delete(key);}};
  vm.createContext(sandbox);
  vm.runInContext(frameJs + '\n' + js + '\nthis.api = {openRuntime, testBeforeRuntimeReceipt, BoundaryError, Error};', sandbox);
  const api = sandbox.api;
  return {state, api, runtime:api.openRuntime(), timers,
    async tick() {
      const item = timers.entries().next();
      if (!item.done) {timers.delete(item.value[0]); item.value[1]();}
      await Promise.resolve();
    }};
}
function track(promise) {
  const outcome = {status:'pending'};
  promise.then(() => {outcome.status = 'resolved';}, error => {outcome.status = 'rejected'; outcome.error = error;});
  return outcome;
}
function rejectsCall(fn, code, api) {
  assert.throws(fn, error => error instanceof api.BoundaryError && error.code === code);
}
const tests = {
  async real_cutpoint_order_once_and_reentry() {
    const f = fixture(); let calls = 0, closing;
    f.api.testBeforeRuntimeReceipt(f.runtime, () => {
      calls++; assert.equal(f.state.physical, true); assert.equal(f.state.receipt, 0);
      assert.equal(f.runtime.close(), closing);
      rejectsCall(() => f.api.testBeforeRuntimeReceipt(f.runtime, () => {}), 'Closing', f.api);
      rejectsCall(() => f.api.openRuntime(), route === 'kotlin' ? 'AlreadyInitialized' : 'RuntimeAlreadyOpen', f.api);
    });
    closing = f.runtime.close(); const outcome = track(closing);
    await f.tick(); assert.equal(calls, 0); assert.equal(outcome.status, 'pending');
    f.state.physical = true; await f.tick(); await closing;
    assert.equal(calls, 1); assert.equal(f.state.receipt, 1); assert.equal(f.timers.size, 0);
    // Host-only retention inspection, not a public consumer API or native proof.
    assert.equal(f.runtime.session.testReceiptProbe, undefined);
    rejectsCall(() => f.api.testBeforeRuntimeReceipt(f.runtime, () => {}), 'Closed', f.api);
    await f.runtime.close(); await f.tick(); assert.equal(calls, 1); assert.equal(f.state.receipt, 1);
    const next = f.api.openRuntime(); await Promise.resolve(); assert.ok(next);
  },
  async registration_rejects_without_replacing_owner() {
    const f = fixture(); let calls = 0;
    rejectsCall(() => f.api.testBeforeRuntimeReceipt({}, () => {}), 'InvalidArgument', f.api);
    const other = fixture();
    rejectsCall(() => f.api.testBeforeRuntimeReceipt(other.runtime, () => {}), 'InvalidArgument', f.api);
    for (const invalid of [undefined, null, 1, 'callback', {}]) {
      rejectsCall(() => f.api.testBeforeRuntimeReceipt(f.runtime, invalid), 'InvalidArgument', f.api);
    }
    f.api.testBeforeRuntimeReceipt(f.runtime, () => {calls++;});
    rejectsCall(() => f.api.testBeforeRuntimeReceipt(f.runtime, () => {throw Error('replacement');}), 'InvalidArgument', f.api);
    const closed = f.runtime.close();
    rejectsCall(() => f.api.testBeforeRuntimeReceipt(f.runtime, () => {}), 'Closing', f.api);
    f.state.physical = true; await f.tick(); await closed; assert.equal(calls, 1);
  },
  async callback_throw_preserves_failure_and_unconsumed_receipt() {
    const f = fixture(), expected = new f.api.BoundaryError('ProbeFailure'); let calls = 0;
    f.api.testBeforeRuntimeReceipt(f.runtime, () => {calls++; throw expected;});
    const closed = f.runtime.close(), outcome = track(closed);
    f.state.physical = true; await f.tick();
    assert.equal(outcome.status, 'rejected'); assert.equal(outcome.error, expected);
    assert.equal(f.state.receipt, 0); assert.equal(f.timers.size, 0); assert.equal(calls, 1);
    assert.equal(f.runtime.close(), closed);
    assert.throws(() => f.api.testBeforeRuntimeReceipt(f.runtime, () => {}), error => error === expected);
    await f.tick(); assert.equal(calls, 1); assert.equal(f.state.receipt, 0);
    assert.equal(f.runtime.session.testReceiptProbe, undefined);
  },
  async error_properties_cannot_escape_or_replace_the_original_failure() {
    const f = fixture(), expected = new f.api.Error('original'); let propertyReads = 0;
    for (const name of ['code', 'message', 'then']) {
      Object.defineProperty(expected, name, {get() {propertyReads++; throw Error('secondary');}});
    }
    f.api.testBeforeRuntimeReceipt(f.runtime, () => {throw expected;});
    const closing = f.runtime.close(), outcome = track(closing);
    f.state.physical = true; await f.tick();
    assert.equal(outcome.status, 'rejected'); assert.equal(outcome.error, expected);
    assert.equal(propertyReads, 0); assert.equal(f.runtime.close(), closing);
    assert.equal(f.state.receipt, 0); assert.equal(f.timers.size, 0);
    assert.equal(f.runtime.session.testReceiptProbe, undefined);
    await f.tick(); assert.equal(propertyReads, 0); assert.equal(f.state.receipt, 0);
  },
  async non_error_throws_and_classification_traps_reject_without_sentinel_collision() {
    let traps = 0, properties = 0;
    const trapped = new Proxy({}, {
      getPrototypeOf() {traps++; throw Error('classification failure');},
      get() {properties++; throw Error('must not inspect thrown properties');},
    });
    for (const thrown of [undefined, null, false, 0, NaN, '', 1n, Symbol('throw'), {}, [], trapped]) {
      const f = fixture(); let calls = 0;
      f.api.testBeforeRuntimeReceipt(f.runtime, () => {calls++; throw thrown;});
      const closing = f.runtime.close(), outcome = track(closing);
      f.state.physical = true; await f.tick();
      assert.equal(outcome.status, 'rejected'); assert.equal(outcome.error.code, 'CallbackFailure');
      assert.ok(outcome.error instanceof f.api.BoundaryError);
      assert.equal(f.runtime.close(), closing); assert.equal(calls, 1);
      assert.equal(f.state.receipt, 0); assert.equal(f.timers.size, 0);
      assert.equal(f.runtime.session.testReceiptProbe, undefined);
      await f.tick(); assert.equal(calls, 1); assert.equal(f.state.receipt, 0);
    }
    assert.equal(traps, 1); assert.equal(properties, 0);
  },
  async promise_thenable_scalar_returns_reject_without_assimilation() {
    let thenReads = 0;
    const thenable = {get then() {thenReads++; throw Error('must not inspect then');}};
    for (const callback of [async () => {}, () => Promise.resolve(), () => thenable, () => 1, () => null]) {
      const f = fixture(); f.api.testBeforeRuntimeReceipt(f.runtime, callback);
      const outcome = track(f.runtime.close()); f.state.physical = true; await f.tick();
      assert.equal(outcome.status, 'rejected'); assert.equal(outcome.error.code, 'InvalidArgument');
      assert.ok(outcome.error instanceof f.api.BoundaryError);
      assert.equal(f.state.receipt, 0); assert.equal(f.timers.size, 0);
      assert.equal(f.runtime.session.testReceiptProbe, undefined);
    }
    assert.equal(thenReads, 0);
  },
  async preceding_failure_does_not_invoke_probe() {
    const f = fixture(); let calls = 0;
    f.api.testBeforeRuntimeReceipt(f.runtime, () => {calls++;});
    const outcome = track(f.runtime.close()); f.state.failPoll = true; await f.tick();
    assert.equal(outcome.status, 'rejected'); assert.equal(outcome.error.code, 'PollUnavailable');
    f.state.failPoll = false; f.state.physical = true; await f.tick();
    assert.equal(calls, 0); assert.equal(f.state.receipt, 0); assert.equal(f.timers.size, 0);
    assert.equal(f.runtime.session.testReceiptProbe, undefined);
  },
  async unarmed_close_preserves_receipt() {
    const f = fixture(), closed = f.runtime.close(); f.state.physical = true;
    await f.tick(); await closed; assert.equal(f.state.receipt, 1);
  },
  declaration_and_role_policy_are_exact() {
    const mode = route === 'kotlin' ? 'source-kotlin-test' : 'source-test';
    const descriptor = readFacade(ts, Buffer.from(actual), 'Boundary.ets', mode);
    const helper = descriptor.functions.find(f => f.name === 'testBeforeRuntimeReceipt');
    assert.equal(helper.params[1].type.tag, 'callback'); assert.equal(helper.params[1].type.returns.tag, 'void');
    const policyPath = path.join(__dirname, '../../../adapters/generators/arkts/profiles',
      route === 'kotlin' ? 'snapshot-kotlin-test-v1.json' : 'snapshot-test-v1.json');
    const policy = JSON.parse(fs.readFileSync(policyPath));
    const signature = policy.public_surface.functions.find(f => f.name === helper.name);
    const normalized = {...helper, params:helper.params.map(({location, ...param}) => param)};
    delete normalized.location; assert.deepEqual(normalized, signature);
    const slot = policy.calls[helper.name].slots['parameter.probe'];
    assert.equal(slot.synchronous_callback, true); assert.equal(slot.reentrant, true);
    assert.equal(slot.context_owner, 'caller'); assert.equal(slot.keep_alive, 'source-lease');
    const bad = actual.replace('export function testBeforeRuntimeReceipt(runtime: Runtime, probe: () => void)',
      'export function testBeforeRuntimeReceipt(runtime: Runtime, probe: () => Promise<void>)');
    assert.notEqual(bad, actual);
    assert.throws(() => readFacade(ts, Buffer.from(bad), 'Boundary.ets', mode), /ARKTS_DECL_CALLBACK_RESULT: callback must return void/);
  },
};
(async () => {
  for (const [name, test] of Object.entries(tests)) {await test(); console.log(route + ': passed ' + name);}
})().catch(error => {console.error(error); process.exitCode = 1;});
