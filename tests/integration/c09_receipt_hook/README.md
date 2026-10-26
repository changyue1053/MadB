# C09 test companion receipt cutpoint

Run with the pinned Node/Python versions and the actual locked TypeScript API:

```sh
python3 tests/integration/c09_receipt_hook/check.py --typescript /absolute/locked/typescript.js
```

The check derives the C/C++/Rust companion from the strict ordinary baseline and
the Kotlin companion from its actual managed generator. Nine cases per facade
execute the generated logic after type erasure: ACK-before-receipt order,
one-shot/reentrant close, brand/type/duplicate/closing/closed admission, thrown
callback identity, Promise/thenable/scalar return rejection without assimilation,
failure before invocation and unarmed ordinary close. Error property getters
must never run or replace the original Error; non-Error throws, including
undefined and a Proxy classification trap, reject with existing CallbackFailure.
The same close Promise rejects, the receipt remains unconsumed, the callback is
not repeated and no timer restarts. The host-only retention assertion inspects
the actual erased Session probe slot; independent consumers use only public API.
The declaration check also
matches the actual helper against the two explicit test role policies and rejects
a Promise-returning public callback declaration with its specific diagnostic.

The native frame and timer implementation are a controlled host model. These
checks cannot establish native cleanup, an ArkTS compiler result, independent HAR
consumption, target execution or C09/C07 acceptance. New frozen test candidates
and independently checked target cases are required. Ordinary templates, native
method inventories and the original C07 reports are unchanged.
