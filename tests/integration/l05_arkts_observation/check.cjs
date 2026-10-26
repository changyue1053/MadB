// Executes the actual optional sources with real host Promise scheduling.
// The controlled Engine is an explicit substitute, never native/ArkUI proof.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');
const {stripTypeScriptTypes} = require('node:module');
const root = path.resolve(__dirname, '../../..');
const sdk = path.join(root, 'sdk/arkts/observation');
const sources = ['StateObservation.ets', 'ArkUIStateModel.ets'].map(name => ({
  name, text: fs.readFileSync(path.join(sdk, name), 'utf8')
}));
const erased = sources.map(source => stripTypeScriptTypes(source.text
  .replace(/^import .*\n/gm, '')
  .replace(/^@ObservedV2\n/gm, '')
  .replace(/@Trace /g, '')
  .replace(/\bexport /g, ''), {mode: 'strip'})).join('\n');
const {StateObservation, ArkUIStateModel} = vm.runInNewContext(erased +
  '\n({ StateObservation, ArkUIStateModel });', {Promise, Error});

function barrier() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return {promise, resolve, reject};
}

class ControlledEngine {
  records = [];
  onRegister;
  registrationFailure;
  registrationRollbacks = 0;

  observeState(listener) {
    if (this.registrationFailure) throw this.registrationFailure;
    const record = {listener, stops: 0, closes: 0, attempts: [], onStop: undefined, onClose: undefined};
    const subscription = {
      requestStop: () => { record.stops++; if (record.onStop) record.onStop(); },
      close: () => {
        record.closes++;
        if (record.onClose) record.onClose();
        const attempt = barrier();
        record.attempts.push(attempt);
        return attempt.promise;
      }
    };
    this.records.push(record);
    try { if (this.onRegister) this.onRegister(listener, record); }
    catch (error) { this.registrationRollbacks++; throw error; }
    return subscription;
  }

  emit(index, value = {value: index, text: 'snapshot'}) { this.records[index].listener(value); }
  observeEvents() { throw new Error('state extension must not subscribe to events'); }
  requestClose() { throw new Error('page must not stop a shared Engine'); }
  close() { throw new Error('page must not close a shared Engine'); }
}

const cases = [];
const test = (name, run) => cases.push({name, run});
const busy = action => assert.throws(action, error => error.code === 'Busy');
const tick = () => Promise.resolve();

test('invalid listeners reject before acquiring a subscription', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  for (const listener of [undefined, null, 1, {}]) {
    assert.throws(() => owner.attach(engine, listener), error => error.code === 'InvalidArgument');
    assert.equal(owner.phase, 'idle');
    assert.equal(engine.records.length, 0);
  }
  await owner.close();
});

test('constructor/getters are inert; attach registers exactly once; values stay ordinary', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  assert.equal(owner.phase, 'idle');
  assert.equal(owner.closeFailure, undefined);
  assert.equal(engine.records.length, 0);
  let received;
  owner.attach(engine, value => { received = value; });
  assert.equal(received, undefined, 'the adapter must not invent a replay');
  busy(() => owner.attach(engine, () => {}));
  assert.equal(engine.records.length, 1);
  const snapshot = {value: 7, text: 'owned snapshot'};
  engine.emit(0, snapshot);
  assert.equal(received, snapshot, 'no second DTO bridge is introduced');
  const closing = owner.close();
  await tick(); engine.records[0].attempts[0].resolve(); await closing;
});

test('stop gates synchronously; close is a shared pending join; restart waits for ACK', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  let calls = 0;
  owner.attach(engine, () => { calls++; });
  owner.requestStop();
  assert.equal(engine.records[0].stops, 1);
  engine.emit(0); assert.equal(calls, 0);
  busy(() => owner.attach(engine, () => {}));
  const closing = owner.close();
  assert.equal(closing, owner.close());
  assert.equal(owner.phase, 'closing');
  let returned = false;
  closing.then(() => { returned = true; });
  await tick(); await tick();
  assert.equal(returned, false, 'a stop request is not a drain ACK');
  assert.equal(engine.records[0].closes, 1);
  busy(() => owner.attach(engine, () => {}));
  engine.records[0].attempts[0].resolve(); await closing;
  assert.equal(owner.phase, 'idle');
  await owner.close();
  owner.attach(engine, () => { calls++; });
  engine.emit(0); assert.equal(calls, 0, 'old epoch cannot enter the new receiver');
  engine.emit(1); assert.equal(calls, 1);
  const again = owner.close(); await tick(); engine.records[1].attempts[0].resolve(); await again;
});

test('an entered listener can stop; the current frame returns but nested/late delivery is gated', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  let entries = 0, exits = 0;
  owner.attach(engine, () => {
    entries++; owner.requestStop(); engine.emit(0); exits++;
  });
  engine.emit(0); engine.emit(0);
  assert.equal(entries, 1); assert.equal(exits, 1);
  const closing = owner.close(); await tick(); engine.records[0].attempts[0].resolve(); await closing;
});

test('stop during synchronous registration is applied to the returned Subscription', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  let calls = 0;
  engine.onRegister = listener => { listener({value: 1, text: 'during registration'}); };
  owner.attach(engine, () => { calls++; owner.requestStop(); });
  assert.equal(owner.phase, 'stopping');
  assert.equal(engine.records[0].stops, 1);
  engine.emit(0); assert.equal(calls, 1);
  const closing = owner.close(); await tick(); engine.records[0].attempts[0].resolve(); await closing;
});

test('close during synchronous registration retains and drains exactly that Subscription', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  let closing;
  engine.onRegister = listener => { listener({value: 2, text: 'synchronous'}); };
  owner.attach(engine, () => {
    closing = owner.close(); assert.equal(closing, owner.close());
    busy(() => owner.attach(engine, () => {}));
  });
  assert.equal(owner.phase, 'closing');
  assert.equal(engine.records[0].stops, 1);
  await tick(); assert.equal(engine.records[0].closes, 1);
  engine.records[0].attempts[0].resolve(); await closing;
  assert.equal(owner.phase, 'idle');
});

test('stop and close reentrancy join the already-registered close attempt', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  owner.attach(engine, () => {});
  let fromStop, fromClose;
  engine.records[0].onStop = () => { fromStop = owner.close(); };
  engine.records[0].onClose = () => { fromClose = owner.close(); };
  const closing = owner.close();
  assert.equal(fromStop, closing);
  await tick(); assert.equal(fromClose, closing);
  assert.equal(engine.records[0].closes, 1);
  engine.records[0].attempts[0].resolve(); await closing;
});

test('drain rejection retains the same Subscription and error identity; retry never resubscribes', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  let deliveries = 0;
  owner.attach(engine, () => { deliveries++; });
  const failure = new Error('close observer exhausted');
  const first = owner.close();
  const rejected = assert.rejects(first, error => error === failure);
  await tick(); engine.records[0].attempts[0].reject(failure); await rejected;
  assert.equal(owner.phase, 'closeFailed'); assert.equal(owner.closeFailure, failure);
  busy(() => owner.attach(engine, () => {}));
  engine.emit(0); assert.equal(deliveries, 0);
  const retry = owner.close(); assert.notEqual(retry, first); assert.equal(retry, owner.close());
  await tick();
  assert.equal(engine.records.length, 1); assert.equal(engine.records[0].closes, 2);
  engine.records[0].attempts[1].resolve(); await retry;
  assert.equal(owner.phase, 'idle'); assert.equal(owner.closeFailure, undefined);
});

test('synchronous stop failure gates delivery and becomes a retryable close rejection', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  let calls = 0;
  owner.attach(engine, () => { calls++; });
  const failure = new Error('requestStop failed');
  engine.records[0].onStop = () => { throw failure; };
  assert.throws(() => owner.requestStop(), error => error === failure);
  engine.emit(0); assert.equal(calls, 0);
  await assert.rejects(owner.close(), error => error === failure);
  assert.equal(owner.closeFailure, failure); assert.equal(owner.phase, 'closeFailed');
  assert.equal(engine.records[0].closes, 0);
  busy(() => owner.attach(engine, () => {}));
  engine.records[0].onStop = undefined;
  const retry = owner.close(); await tick(); engine.records[0].attempts[0].resolve(); await retry;
});

test('synchronous close throw is a retained asynchronous rejection', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation(); owner.attach(engine, () => {});
  const failure = new Error('close threw');
  engine.records[0].onClose = () => { throw failure; };
  await assert.rejects(owner.close(), error => error === failure);
  assert.equal(owner.phase, 'closeFailed');
  engine.records[0].onClose = undefined;
  const retry = owner.close(); await tick(); engine.records[0].attempts[0].resolve(); await retry;
});

test('listener exception identity survives; failed delivery cannot reenter later', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  const failure = new Error('business listener');
  let calls = 0;
  owner.attach(engine, () => { calls++; throw failure; });
  assert.throws(() => engine.emit(0), error => error === failure);
  engine.emit(0); assert.equal(calls, 1); assert.equal(owner.phase, 'stopping');
  const closing = owner.close(); await tick(); engine.records[0].attempts[0].resolve(); await closing;
});

test('synchronous registration listener failure remains unchanged, including an undefined throw', async () => {
  for (const failure of [new Error('synchronous listener'), undefined]) {
    const engine = new ControlledEngine();
    const owner = new StateObservation();
    let closing, thrown = false;
    engine.onRegister = listener => { listener({value: 0, text: 'synchronous'}); };
    try {
      owner.attach(engine, () => { closing = owner.close(); throw failure; });
    } catch (error) { thrown = true; assert.equal(error, failure); }
    assert.equal(thrown, true);
    await closing.then(() => { assert.fail('failed registration cannot confirm close'); },
      error => { assert.equal(error, failure); });
    assert.equal(engine.registrationRollbacks, 1); assert.equal(owner.phase, 'idle');
  }
});

test('failed registration restores admission; reentrant close receives the original failure', async () => {
  const engine = new ControlledEngine();
  const owner = new StateObservation();
  const failure = new Error('registration failure');
  engine.registrationFailure = failure;
  assert.throws(() => owner.attach(engine, () => {}), error => error === failure);
  assert.equal(owner.phase, 'idle');
  engine.registrationFailure = undefined;
  let closing;
  engine.onRegister = listener => { listener({value: 1, text: 'then registration fails'}); throw failure; };
  assert.throws(() => owner.attach(engine, () => { closing = owner.close(); }), error => error === failure);
  busy(() => owner.attach(engine, () => {}));
  await assert.rejects(closing, error => error === failure);
  assert.equal(engine.registrationRollbacks, 1); assert.equal(owner.phase, 'idle');
  engine.emit(0);
  engine.onRegister = undefined;
  owner.attach(engine, () => {});
  const done = owner.close(); await tick(); engine.records[1].attempts[0].resolve(); await done;
});

test('ArkUI projection resets only on successful admission and preserves synchronous first state', async () => {
  // Decorators are erased in this host test. Notification behavior needs ArkUI.
  const engine = new ControlledEngine();
  const model = new ArkUIStateModel();
  assert.equal(model.snapshot, undefined); assert.equal(engine.records.length, 0);
  model.attach(engine);
  const first = {value: 10, text: 'last confirmed'};
  engine.emit(0, first); assert.equal(model.snapshot, first);
  busy(() => model.attach(engine)); assert.equal(model.snapshot, first);
  const closing = model.close(); await tick(); engine.records[0].attempts[0].resolve(); await closing;
  assert.equal(model.snapshot, first, 'stop retains the last confirmed local state');
  engine.registrationFailure = new Error('not admitted');
  assert.throws(() => model.attach(engine)); assert.equal(model.snapshot, first);
  engine.registrationFailure = undefined;
  model.attach(engine); assert.equal(model.snapshot, undefined);
  const again = model.close(); await tick(); engine.records[1].attempts[0].resolve(); await again;
  const synchronous = {value: 11, text: 'actual first state'};
  engine.onRegister = listener => { listener(synchronous); };
  model.attach(engine); assert.equal(model.snapshot, synchronous);
  model.requestStop(); engine.emit(2, {value: 12, text: 'late'}); assert.equal(model.snapshot, synchronous);
  const final = model.close(); await tick(); engine.records[2].attempts[0].resolve(); await final;
});

test('ArkUI publishes synchronous snapshots only after successful registration', async () => {
  const engine = new ControlledEngine();
  const model = new ArkUIStateModel();
  const previous = {value: 1, text: 'previous confirmed session'};
  model.attach(engine); engine.emit(0, previous);
  const firstClose = model.close(); await tick(); engine.records[0].attempts[0].resolve(); await firstClose;
  const failure = new Error('registration failed after callback');
  const pending = {value: 2, text: 'uncommitted registration delivery'};
  engine.onRegister = listener => {
    listener(pending);
    assert.equal(model.snapshot, previous, 'do not publish before registration succeeds');
    throw failure;
  };
  assert.throws(() => model.attach(engine), error => error === failure);
  assert.equal(model.snapshot, previous); assert.equal(model.phase, 'idle');
  assert.equal(engine.registrationRollbacks, 1);
  let closing;
  const confirmed = {value: 3, text: 'confirmed despite stop during registration'};
  engine.onRegister = listener => {
    listener(pending); listener(confirmed);
    model.requestStop(); closing = model.close();
    assert.equal(model.snapshot, previous);
  };
  model.attach(engine);
  assert.equal(model.snapshot, confirmed); assert.equal(model.phase, 'closing');
  busy(() => model.attach(engine)); assert.equal(model.snapshot, confirmed);
  await tick(); engine.records[2].attempts[0].resolve(); await closing;
  assert.equal(model.snapshot, confirmed);
});

let activeCase = 'test startup';
const deadline = setTimeout(() => {
  console.error('L05 host test did not complete: ' + activeCase);
  process.exitCode = 1;
}, 10000);
(async () => {
  const passed = [];
  for (const entry of cases) { activeCase = entry.name; await entry.run(); passed.push(entry.name); }
  console.log(JSON.stringify({scope: 'L05 host lifecycle; controlled Engine; ArkUI decorators erased',
    certifies_c07: false, certifies_l05: false, node: process.version,
    sources: sources.map(source => ({path: 'sdk/arkts/observation/' + source.name,
      sha256: crypto.createHash('sha256').update(source.text).digest('hex')})),
    passed: passed.length, cases: passed}, null, 2));
})().catch(error => { console.error(error); process.exitCode = 1; }).finally(() => { clearTimeout(deadline); });
