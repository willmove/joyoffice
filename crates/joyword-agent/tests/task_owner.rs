//! Local workers only: no network, document handle or real credentials.
use joyword_agent::*;
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

struct Held {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    outcome: u8,
}
impl Provider for Held {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: false,
            tools: false,
        }
    }
    fn calls(&self) -> u64 {
        0
    }
    fn complete(
        &self,
        request: &CompletionRequest<'_>,
    ) -> Result<CompletionResponse, ProviderError> {
        self.entered.send(()).unwrap();
        // Test-only escape hatch: every blocked worker is released and reaped.
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        match self.outcome {
            1 => Err(ProviderError::Offline),
            2 => panic!("synthetic worker failure"),
            3 => {
                assert!(request.cancel.is_cancelled());
                Err(ProviderError::Cancelled)
            }
            _ => Ok(CompletionResponse {
                text: "synthetic late result".into(),
            }),
        }
    }
}
fn setup(outcome: u8) -> (AgentTaskOwner, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered, observed) = mpsc::channel();
    let (release, receive) = mpsc::channel();
    let mut config = AiConfig::default();
    config.user_enabled = true;
    config.provider = ProviderSelection::named("fake").unwrap();
    let runtime = AgentRuntime::new(
        config,
        AdminPolicy::unmanaged(),
        Box::new(Held {
            entered,
            release: Mutex::new(receive),
            outcome,
        }),
        Box::new(MemoryCredentialStore::new()),
    );
    (AgentTaskOwner::new(runtime).unwrap(), observed, release)
}
fn reap(owner: &mut AgentTaskOwner) -> TaskPoll {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = owner.poll(Instant::now()).unwrap();
        if !matches!(status, TaskPoll::Running | TaskPoll::Stopping { .. }) {
            return status;
        }
        assert!(Instant::now() < deadline, "worker did not exit");
        std::thread::yield_now();
    }
}

#[test]
fn timeout_retains_worker_and_suppresses_all_late_outcomes() {
    for outcome in 0..4 {
        let (mut owner, entered, release) = setup(outcome);
        owner.start("synthetic prompt".into()).unwrap();
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        let now = Instant::now();
        owner.request_stop(now, Duration::from_secs(2)).unwrap();
        assert!(matches!(
            owner.poll(now).unwrap(),
            TaskPoll::Stopping { timed_out: false }
        ));
        owner
            .request_stop(now + Duration::from_secs(1), Duration::from_secs(100))
            .unwrap();
        assert!(matches!(
            owner.poll(now + Duration::from_secs(2)).unwrap(),
            TaskPoll::Stopping { timed_out: true }
        ));
        assert!(!owner.ready_to_close());
        owner = match owner.try_close() {
            Err(retained) => retained,
            Ok(()) => panic!("live worker was closed"),
        };
        assert_eq!(owner.enable(), Err(TaskError::Busy));
        assert_eq!(owner.start("must not send".into()), Err(TaskError::Busy));
        // Timeout did not discard the task: even another poll must stay Stopping.
        assert!(matches!(
            owner.poll(now + Duration::from_secs(20)).unwrap(),
            TaskPoll::Stopping { timed_out: true }
        ));
        release.send(()).unwrap();
        assert!(matches!(reap(&mut owner), TaskPoll::Disabled));
        assert!(owner.ready_to_close());
        assert!(matches!(
            owner.poll(Instant::now()).unwrap(),
            TaskPoll::Disabled
        ));
        owner.enable().unwrap();
        assert!(matches!(
            owner.poll(Instant::now()).unwrap(),
            TaskPoll::Idle
        ));
        owner.request_stop(Instant::now(), Duration::ZERO).unwrap();
        assert!(matches!(
            owner.poll(Instant::now()).unwrap(),
            TaskPoll::Disabled
        ));
        assert!(owner.ready_to_close());
        assert!(owner.try_close().is_ok());
    }
}

#[test]
fn normal_completion_failure_and_panic_are_reaped_without_blocking_poll() {
    for outcome in 0..3 {
        let (mut owner, entered, release) = setup(outcome);
        owner.start("synthetic".into()).unwrap();
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(
            owner.poll(Instant::now()).unwrap(),
            TaskPoll::Running
        ));
        assert_eq!(owner.start("overlap".into()), Err(TaskError::Busy));
        release.send(()).unwrap();
        match (outcome, reap(&mut owner)) {
            (0, TaskPoll::Completed(Ok(response))) => {
                assert_eq!(response.text, "synthetic late result")
            }
            (
                1,
                TaskPoll::Completed(Err(TaskError::Runtime(AgentError::Provider(
                    ProviderError::Offline,
                )))),
            ) => {}
            (2, TaskPoll::Completed(Err(TaskError::WorkerPanicked))) => {}
            _ => panic!("unexpected completion"),
        }
        assert!(matches!(
            owner.poll(Instant::now()).unwrap(),
            TaskPoll::Idle
        ));
        owner.request_stop(Instant::now(), Duration::ZERO).unwrap();
        assert!(matches!(
            owner.poll(Instant::now()).unwrap(),
            TaskPoll::Disabled
        ));
        assert!(owner.ready_to_close());
    }
}

#[test]
fn immediate_stop_covers_worker_not_yet_admitted() {
    let runtime = AgentRuntime::boot_disabled();
    let mut owner = AgentTaskOwner::new(runtime).unwrap();
    assert!(owner.ready_to_close());
    assert_eq!(owner.start("disabled".into()), Err(TaskError::Busy));
    owner.enable().unwrap();
    owner.start("synthetic".into()).unwrap();
    owner.request_stop(Instant::now(), Duration::ZERO).unwrap();
    assert!(matches!(reap(&mut owner), TaskPoll::Disabled));
    assert!(owner.ready_to_close());
}
