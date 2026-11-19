#[derive(Debug, PartialEq)]
pub enum State {
    Clean { revision: u64 },
    Pending { revision: u64, due: f64 },
    Saving { revision: u64 },
    Failed { revision: u64, error: String },
}

pub enum Event {
    Edited {
        revision: u64,
        now: f64,
    },
    Timer(f64),
    Completed {
        revision: u64,
        result: Result<(), String>,
    },
    Flush,
}

#[derive(Debug, PartialEq)]
pub enum Effect {
    Write(u64),
    WakeAt(f64),
}

pub const DEBOUNCE: f64 = 0.6;

impl State {
    pub fn revision(&self) -> u64 {
        match self {
            Self::Clean { revision }
            | Self::Pending { revision, .. }
            | Self::Saving { revision }
            | Self::Failed { revision, .. } => *revision,
        }
    }

    pub fn unacknowledged(&self) -> bool {
        !matches!(self, Self::Clean { .. })
    }
}

pub fn step(state: &mut State, event: Event) -> Vec<Effect> {
    step_after(state, event, DEBOUNCE)
}

pub fn step_after(state: &mut State, event: Event, delay: f64) -> Vec<Effect> {
    match event {
        Event::Edited { revision, now } if revision > state.revision() => {
            let due = now
                + if delay.is_finite() {
                    delay.clamp(0.1, 10.)
                } else {
                    DEBOUNCE
                };
            *state = State::Pending { revision, due };
            vec![Effect::WakeAt(due)]
        }
        Event::Timer(now) => match state {
            State::Pending { revision, due } if now >= *due => {
                let revision = *revision;
                *state = State::Saving { revision };
                vec![Effect::Write(revision)]
            }
            State::Pending { due, .. } => vec![Effect::WakeAt(*due)],
            _ => vec![],
        },
        Event::Flush if state.unacknowledged() => {
            let revision = state.revision();
            *state = State::Saving { revision };
            vec![Effect::Write(revision)]
        }
        Event::Completed { revision, result } if revision == state.revision() => {
            *state = match result {
                Ok(()) => State::Clean { revision },
                Err(error) => State::Failed { revision, error },
            };
            vec![]
        }
        _ => vec![],
    }
}
