use aislide_core::generation::CancellationToken;
use std::{sync::{Arc, Mutex}, time::{Duration, Instant}};

struct ActiveRequest {
    window: String,
    operation: String,
    cancellation: CancellationToken,
}

#[derive(Default, Clone)]
pub struct RequestGate(Arc<Mutex<GateState>>);

#[derive(Default)]
struct GateState { active: Option<ActiveRequest>, pending: Vec<(String, String, Instant)> }

fn validate_operation(operation:&str)->Result<(),String> {
    if operation.is_empty() || operation.len()>128 || !operation.bytes().all(|byte|byte.is_ascii_alphanumeric() || byte==b'-') {return Err("Invalid operation ID".into());}
    Ok(())
}

pub struct RequestGuard {
    state: RequestGate,
    cancellation: CancellationToken,
}

impl RequestGate {
    pub fn begin(&self, window: &str, operation: &str) -> Result<RequestGuard, String> {
        validate_operation(operation)?;
        let mut current = self.0.lock().map_err(|_| "Core request state unavailable")?;
        if current.active.is_some() { return Err("Core is busy".into()); }
        let cancellation = CancellationToken::new();
        current.pending.retain(|(_,_,created)|created.elapsed()<Duration::from_secs(30));
        if let Some(index)=current.pending.iter().position(|(owner,id,_)|owner==window && id==operation) {current.pending.remove(index);cancellation.cancel();}
        current.active = Some(ActiveRequest { window: window.into(), operation: operation.into(), cancellation: cancellation.clone() });
        Ok(RequestGuard { state: self.clone(), cancellation })
    }

    pub fn cancel(&self, window: &str, operation: &str) -> Result<bool, String> {
        validate_operation(operation)?;
        let mut current = self.0.lock().map_err(|_| "Core request state unavailable")?;
        if let Some(active) = current.active.as_ref().filter(|active| active.window == window && active.operation == operation) {
            active.cancellation.cancel();
            return Ok(true);
        }
        if current.active.is_some() {return Ok(false);}
        current.pending.retain(|(_,_,created)|created.elapsed()<Duration::from_secs(30));
        if current.pending.iter().any(|(owner,id,_)|owner==window && id==operation) {return Ok(true);}
        if current.pending.len()>=64 {return Err("Too many pending cancellations".into());}
        current.pending.push((window.into(),operation.into(),Instant::now()));
        Ok(true)
    }
}

impl RequestGuard {
    pub fn cancellation(&self) -> CancellationToken { self.cancellation.clone() }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut current) = self.state.0.lock() { current.active = None; }
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

    #[test]
    fn cancellation_arriving_before_registration_is_bounded_and_window_scoped() {
        let gate=RequestGate::default();
        assert!(gate.cancel("main","early-operation").unwrap());
        let other=gate.begin("other","early-operation").unwrap();
        assert!(!other.cancellation().is_cancelled());drop(other);
        let running=gate.begin("main","early-operation").unwrap();
        assert!(running.cancellation().is_cancelled());drop(running);
        let reused=gate.begin("main","early-operation").unwrap();
        assert!(!reused.cancellation().is_cancelled());drop(reused);
        for index in 0..64 {assert!(gate.cancel("main",&format!("pending-{index}")).unwrap());}
        assert!(gate.cancel("main","too-many").is_err());
        assert!(gate.cancel("main","").is_err());
    }
}