use aislide_core::generation::CancellationToken;
use std::sync::{Arc, Mutex};

struct ActiveRequest {
    window: String,
    operation: String,
    cancellation: CancellationToken,
}

#[derive(Default, Clone)]
pub struct RequestGate(Arc<Mutex<Option<ActiveRequest>>>);

pub struct RequestGuard {
    state: RequestGate,
    cancellation: CancellationToken,
}

impl RequestGate {
    pub fn begin(&self, window: &str, operation: &str) -> Result<RequestGuard, String> {
        if operation.is_empty() || operation.len() > 128 || !operation.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') {
            return Err("Invalid operation ID".into());
        }
        let mut current = self.0.lock().map_err(|_| "Core request state unavailable")?;
        if current.is_some() { return Err("Core is busy".into()); }
        let cancellation = CancellationToken::new();
        *current = Some(ActiveRequest { window: window.into(), operation: operation.into(), cancellation: cancellation.clone() });
        Ok(RequestGuard { state: self.clone(), cancellation })
    }

    pub fn cancel(&self, window: &str, operation: &str) -> Result<bool, String> {
        let current = self.0.lock().map_err(|_| "Core request state unavailable")?;
        if let Some(active) = current.as_ref().filter(|active| active.window == window && active.operation == operation) {
            active.cancellation.cancel();
            return Ok(true);
        }
        Ok(false)
    }
}

impl RequestGuard {
    pub fn cancellation(&self) -> CancellationToken { self.cancellation.clone() }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut current) = self.state.0.lock() { *current = None; }
    }
}

#[cfg(test)]
mod tests {
    use super::RequestGate;

    #[test]
    fn cancellation_is_scoped_to_window_and_operation() {
        let gate = RequestGate::default();
        let running = gate.begin("main", "operation-one").unwrap();
        assert!(gate.begin("main", "operation-two").is_err());
        assert!(!gate.cancel("other-window", "operation-one").unwrap());
        assert!(!gate.cancel("main", "wrong-operation").unwrap());
        assert!(!running.cancellation().is_cancelled());
        assert!(gate.cancel("main", "operation-one").unwrap());
        assert!(running.cancellation().is_cancelled());
        assert!(gate.begin("main", "operation-two").is_err());
        drop(running);
        assert!(gate.begin("main", "operation-two").is_ok());
    }

    #[test]
    fn operation_ids_are_bounded_and_nonempty() {
        let gate = RequestGate::default();
        assert!(gate.begin("main", "").is_err());
        assert!(gate.begin("main", &"a".repeat(129)).is_err());
        assert!(gate.begin("main", "valid-id").is_ok());
    }
}