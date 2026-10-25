# akinterop source-patch attribution

Both patches contain changes and context from [CPF-KMP-CMP/akinterop at commit 75ba50c77afda2845a3a6c51cdcddb03d4fd0ab8](https://gitcode.com/CPF-KMP-CMP/akinterop/tree/75ba50c77afda2845a3a6c51cdcddb03d4fd0ab8). The unmodified upstream [Apache-2.0 license](patches/LICENSE-Apache-2.0) and [NOTICE](patches/NOTICE-upstream) are included once and referenced by both source pins. The NOTICE attributes AKInterop to Huawei Device Co., Ltd. and portions derived from Tencent-TDS/KuiklyBase-components/knoi to Tencent Inc. Existing source notices are preserved.

MadB's r2 changes affect the ten files in [managed-source.json](managed-source.json): bounded main-env reference/notification ownership and native error-output repair. Its patch and source-pin bytes remain unchanged.

MadB's r3 changes affect the thirteen files in [managed-source-r3.json](managed-source-r3.json). r3 includes r2's changes and adds the opt-in internal coordinated-shutdown experiment: generated-entry leases and gates, explicit hook removal, interface creator reference accounting, reference/metadata release and native TSFN finalizer/context acknowledgment. r3 adds an internal helper ABI and has its own matched build and target evidence. It does not claim actual env cleanup-hook invocation or a complete C07 SDK profile.

These patches are local modifications, not an upstream release or endorsement. Each source pin records original/patched hashes, exact patch bytes and notice hashes; the README states the supported subsets and remaining validation.

This source package contains patches, source pins and notices only. A future binary/SDK package must include the notices and dependency closure appropriate to its actual shipped sources and runtime. This file does not certify a release.
