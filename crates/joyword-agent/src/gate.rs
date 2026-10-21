//! Kill switch and generation gate (AI.md §4).
//!
//! [`AiGate::commit_disabled`] is the linearization point: under one mutex it
//! sets the phase to [`AiPhase::Disabled`] and increments the generation.
//! [`AiGate::begin_stop`] publishes [`AiPhase::Stopping`] first, so new
//! requests and not-yet-admitted commits fail before that increment.
//! A provider cannot clear either flag.

use std::fmt;
use std::sync::Mutex;

/// Monotonic permission generation. Tickets from an older value are stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Permission captured when a request was allowed to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Admission {
    generation: Generation,
}

impl Admission {
    pub const fn generation(self) -> Generation {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiPhase {
    Disabled,
    EnabledIdle,
    Running,
    Stopping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitDecision {
    /// The check ran while this generation was still allowed to commit.
    /// T400 does not apply a document change; T402 must re-check immediately
    /// before its transaction.
    Allowed,
    Rejected,
    Duplicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateError {
    Disabled,
    Stopping,
    Stale,
    ManagedDisabled,
    GenerationExhausted,
    Poisoned,
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            GateError::Disabled => "ai is disabled",
            GateError::Stopping => "ai is stopping",
            GateError::Stale => "ai result is stale",
            GateError::ManagedDisabled => "an administrator policy disables ai",
            GateError::GenerationExhausted => "ai generation counter is exhausted",
            GateError::Poisoned => "ai gate lock was poisoned",
        })
    }
}

impl std::error::Error for GateError {}

struct Inner {
    phase: AiPhase,
    generation: u64,
    user_enabled: bool,
    policy_forced_off: bool,
    in_flight: u64,
}

/// Process-wide AI permission. Cloning is deliberately not implemented; share
/// with [`std::sync::Arc`] when tasks race the kill switch.
pub struct AiGate {
    inner: Mutex<Inner>,
}

impl AiGate {
    /// First launch: disabled, generation 1, user preference off.
    pub fn disabled() -> Self {
        Self::from_flags(false, false)
    }

    pub fn from_flags(user_enabled: bool, policy_forced_off: bool) -> Self {
        let enabled = user_enabled && !policy_forced_off;
        Self {
            inner: Mutex::new(Inner {
                phase: if enabled {
                    AiPhase::EnabledIdle
                } else {
                    AiPhase::Disabled
                },
                generation: 1,
                user_enabled,
                policy_forced_off,
                in_flight: 0,
            }),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, GateError> {
        self.inner.lock().map_err(|_| GateError::Poisoned)
    }

    pub fn phase(&self) -> Result<AiPhase, GateError> {
        Ok(self.lock()?.phase)
    }

    pub fn generation(&self) -> Result<Generation, GateError> {
        Ok(Generation(self.lock()?.generation))
    }

    pub fn user_enabled(&self) -> Result<bool, GateError> {
        Ok(self.lock()?.user_enabled)
    }

    pub fn policy_forced_off(&self) -> Result<bool, GateError> {
        Ok(self.lock()?.policy_forced_off)
    }

    /// User toggle. A forced-off policy rejects enabling. Disabling runs the
    /// kill switch. Re-enabling does not restore an old generation's tasks.
    pub fn set_user_enabled(&self, enabled: bool) -> Result<Generation, GateError> {
        if enabled {
            let mut guard = self.lock()?;
            if guard.policy_forced_off {
                return Err(GateError::ManagedDisabled);
            }
            guard.user_enabled = true;
            if guard.phase == AiPhase::Disabled {
                guard.phase = AiPhase::EnabledIdle;
            }
            return Ok(Generation(guard.generation));
        }
        self.kill_switch()
    }

    /// Admin force-disable. The user's stored preference is left intact and
    /// becomes effective again only after the policy lifts, still on a new
    /// generation so old tickets stay stale.
    pub fn set_policy_forced_off(&self, forced: bool) -> Result<Generation, GateError> {
        let mut guard = self.lock()?;
        guard.policy_forced_off = forced;
        if forced {
            guard.phase = AiPhase::Disabled;
            if guard.generation == u64::MAX {
                return Err(GateError::GenerationExhausted);
            }
            // Always bump, including when already disabled, so a ticket taken
            // before the policy landed cannot commit.
            guard.generation += 1;
            return Ok(Generation(guard.generation));
        }
        if guard.user_enabled && guard.phase == AiPhase::Disabled {
            guard.phase = AiPhase::EnabledIdle;
        }
        Ok(Generation(guard.generation))
    }

    /// Start one provider request. Fails closed unless the phase is idle or
    /// already running. The returned ticket is the only handle that can later
    /// admit a result or a commit.
    pub fn begin_request(&self) -> Result<Admission, GateError> {
        let mut guard = self.lock()?;
        if guard.policy_forced_off {
            return Err(GateError::ManagedDisabled);
        }
        match guard.phase {
            AiPhase::EnabledIdle | AiPhase::Running => {
                guard.in_flight = guard.in_flight.saturating_add(1);
                guard.phase = AiPhase::Running;
                Ok(Admission {
                    generation: Generation(guard.generation),
                })
            }
            AiPhase::Stopping => Err(GateError::Stopping),
            AiPhase::Disabled => Err(GateError::Disabled),
        }
    }

    /// Drop one in-flight count. A stale or stopped gate never returns to idle
    /// because of this call.
    pub fn finish_request(&self, ticket: Admission) -> Result<(), GateError> {
        let mut guard = self.lock()?;
        guard.in_flight = guard.in_flight.saturating_sub(1);
        if guard.generation != ticket.generation.0 {
            return Err(GateError::Stale);
        }
        if guard.phase == AiPhase::Running && guard.in_flight == 0 {
            guard.phase = AiPhase::EnabledIdle;
        }
        Ok(())
    }

    /// Publish Stopping. New requests fail. Generation is unchanged until
    /// [`Self::commit_disabled`].
    pub fn begin_stop(&self) -> Result<(), GateError> {
        let mut guard = self.lock()?;
        match guard.phase {
            AiPhase::Disabled => Err(GateError::Disabled),
            AiPhase::Stopping => Ok(()),
            AiPhase::EnabledIdle | AiPhase::Running => {
                guard.phase = AiPhase::Stopping;
                Ok(())
            }
        }
    }

    /// Linearization point: disabled and generation + 1, one critical section.
    pub fn commit_disabled(&self) -> Result<Generation, GateError> {
        let mut guard = self.lock()?;
        guard.user_enabled = false;
        guard.phase = AiPhase::Disabled;
        if guard.generation == u64::MAX {
            return Err(GateError::GenerationExhausted);
        }
        guard.generation += 1;
        Ok(Generation(guard.generation))
    }

    /// One-button close. Already-disabled is a no-op so the generation does
    /// not move. Otherwise this is [`Self::begin_stop`] plus
    /// [`Self::commit_disabled`] under a single lock acquisition.
    pub fn kill_switch(&self) -> Result<Generation, GateError> {
        let mut guard = self.lock()?;
        if guard.phase == AiPhase::Disabled && !guard.user_enabled {
            return Ok(Generation(guard.generation));
        }
        guard.user_enabled = false;
        guard.phase = AiPhase::Disabled;
        if guard.generation == u64::MAX {
            return Err(GateError::GenerationExhausted);
        }
        guard.generation += 1;
        Ok(Generation(guard.generation))
    }

    pub fn decide_result(&self, ticket: Admission) -> Result<(), GateError> {
        let guard = self.lock()?;
        if guard.generation != ticket.generation.0 {
            return Err(GateError::Stale);
        }
        match guard.phase {
            AiPhase::EnabledIdle | AiPhase::Running => Ok(()),
            AiPhase::Stopping => Err(GateError::Stopping),
            AiPhase::Disabled => Err(GateError::Disabled),
        }
    }

    /// Commit hook for a later transaction layer. This does not edit a document.
    /// Stopping and Disabled both reject, so a commit racing the kill switch
    /// either observed the open generation or it did not.
    pub fn decide_commit(&self, ticket: Admission) -> CommitDecision {
        match self.decide_result(ticket) {
            Ok(()) => CommitDecision::Allowed,
            Err(_) => CommitDecision::Rejected,
        }
    }
}
