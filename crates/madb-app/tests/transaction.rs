use madb_app::transaction::{CommitControl, IndexSnapshot, Layer};
use madb_app::{Digest, process::CancelDecision};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn cancellation_wins_without_running_publish() {
    let control = CommitControl::default();
    let permit = control.begin().unwrap();
    assert_eq!(control.cancel(), CancelDecision::Requested);
    assert_eq!(control.cancel(), CancelDecision::AlreadyRequested);
    let error = permit
        .publish(|| panic!("cancelled publication must not run"))
        .unwrap_err();
    assert_eq!(error.findings[0].code, "STORE002");
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}
#[test]
fn successful_publication_wins_before_later_cancel() {
    let control = CommitControl::default();
    let started = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let published = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let commit = scope.spawn(|| {
            control.begin().unwrap().publish(|| {
                started.wait();
                release.wait();
                published.store(true, Ordering::SeqCst);
                Ok(())
            })
        });
        started.wait();
        // Publication owns the admission mutex; cancellation blocks until it is
        // accepted, then must report AlreadyFinished rather than request cancel.
        let cancel = scope.spawn(|| control.cancel());
        release.wait();
        commit.join().unwrap().unwrap();
        assert_eq!(cancel.join().unwrap(), CancelDecision::AlreadyFinished);
    });
    assert!(published.load(Ordering::SeqCst));
}
#[test]
fn controls_are_single_use_even_after_abandoned_or_failed_commit() {
    let control = CommitControl::default();
    drop(control.begin().unwrap());
    assert!(control.begin().is_err());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}
#[test]
fn source_and_build_advance_invalidate_descendants_and_detect_overflow() {
    let s = Digest::of_bytes(b"source");
    let b = Digest::of_bytes(b"build");
    let c = Digest::of_bytes(b"candidate");
    let index = IndexSnapshot::default()
        .advance(Layer::Source, s.clone())
        .unwrap()
        .advance(Layer::Build, b.clone())
        .unwrap()
        .advance(Layer::Candidate, c)
        .unwrap();
    assert_eq!(index.revision, 3);
    let build = index.advance(Layer::Build, b).unwrap();
    assert_eq!(build.source, Some(s.clone()));
    assert!(build.candidate.is_none());
    let source = index.advance(Layer::Source, s).unwrap();
    assert!(source.build.is_none() && source.candidate.is_none());
    let overflow = IndexSnapshot {
        revision: u64::MAX,
        ..index
    };
    assert_eq!(
        overflow
            .advance(Layer::Source, Digest::of_bytes(b"x"))
            .unwrap_err()
            .findings[0]
            .code,
        "STORE007"
    );
}
