use std::collections::VecDeque;
use std::sync::Mutex;

use super::{DockerBackend, DockerBackendError, DockerFuture, RawContainer};

pub struct FakeDockerBackend {
    outcomes: Mutex<VecDeque<Result<Vec<RawContainer>, DockerBackendError>>>,
    logs: Mutex<VecDeque<Result<Vec<Vec<u8>>, DockerBackendError>>>,
    calls: Mutex<usize>,
}

impl FakeDockerBackend {
    pub fn new(outcomes: Vec<Result<Vec<RawContainer>, DockerBackendError>>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes.into()),
            logs: Mutex::new(VecDeque::new()),
            calls: Mutex::new(0),
        }
    }

    pub fn with_logs(outcomes: Vec<Result<Vec<Vec<u8>>, DockerBackendError>>) -> Self {
        Self {
            outcomes: Mutex::new(VecDeque::new()),
            logs: Mutex::new(outcomes.into()),
            calls: Mutex::new(0),
        }
    }

    pub fn calls(&self) -> usize {
        *self.calls.lock().unwrap_or_else(|error| error.into_inner())
    }
}

impl DockerBackend for FakeDockerBackend {
    fn list_all(&self) -> DockerFuture<'_, Result<Vec<RawContainer>, DockerBackendError>> {
        Box::pin(async move {
            *self.calls.lock().unwrap_or_else(|error| error.into_inner()) += 1;
            self.outcomes
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new()))
        })
    }

    fn logs_follow<'a>(
        &'a self,
        _container_id: &'a str,
    ) -> DockerFuture<'a, Result<Vec<Vec<u8>>, DockerBackendError>> {
        Box::pin(async move {
            self.logs
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .pop_front()
                .unwrap_or_else(|| Ok(Vec::new()))
        })
    }
}
