use aislide_core::generation::CancellationToken;
use std::{collections::VecDeque, sync::{Arc, Mutex}, time::{Duration, Instant}};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lane { Foreground, Preview }

impl Lane {
    fn for_request(request: &serde_json::Value) -> Self {
        match request.get("op").and_then(serde_json::Value::as_str) {
            Some("compute_chart_presentation" | "render_element_preview") => Self::Preview,
            _ => Self::Foreground,
        }
    }

    fn index(self) -> usize { match self { Self::Foreground => 0, Self::Preview => 1 } }
}

struct ActiveRequest {
    window: String,
    operation: String,
    cancellation: CancellationToken,
    owner: Arc<()>,
}

#[derive(Default, Clone)]
pub struct RequestGate(Arc<Mutex<GateState>>);

#[derive(Default)]
struct GateState {
    active: [Option<ActiveRequest>; 2],
    pending: Vec<(String, String, Instant)>,
    completed: VecDeque<(String, String, Instant)>,
}

fn validate_operation(operation:&str)->Result<(),String> {
    if operation.is_empty() || operation.len()>128 || !operation.bytes().all(|byte|byte.is_ascii_alphanumeric() || byte==b'-') {return Err("Invalid operation ID".into());}
    Ok(())
}

pub struct RequestGuard {
    state: RequestGate,
    cancellation: CancellationToken,
    lane: Lane,
    owner: Arc<()>,
}

impl RequestGate {
    pub fn begin(&self, window: &str, operation: &str) -> Result<RequestGuard, String> {
        self.begin_lane(window, operation, Lane::Foreground)
    }

    pub fn begin_request(&self, window: &str, operation: &str, request: &serde_json::Value) -> Result<RequestGuard, String> {
        self.begin_lane(window, operation, Lane::for_request(request))
    }

    fn begin_lane(&self, window: &str, operation: &str, lane: Lane) -> Result<RequestGuard, String> {
        validate_operation(operation)?;
        let mut current = self.0.lock().map_err(|_| "Core request state unavailable")?;
        if current.active[lane.index()].is_some() { return Err("Core is busy".into()); }
        if current.active.iter().flatten().any(|active| active.window == window && active.operation == operation) {
            return Err("Operation ID already active".into());
        }
        let cancellation = CancellationToken::new();
        current.pending.retain(|(_,_,created)|created.elapsed()<Duration::from_secs(30));
        current.completed.retain(|(owner,id,created)| created.elapsed()<Duration::from_secs(30) && !(owner==window && id==operation));
        if let Some(index)=current.pending.iter().position(|(owner,id,_)|owner==window && id==operation) {current.pending.remove(index);cancellation.cancel();}
        let owner = Arc::new(());
        current.active[lane.index()] = Some(ActiveRequest { window: window.into(), operation: operation.into(), cancellation: cancellation.clone(), owner: owner.clone() });
        Ok(RequestGuard { state: self.clone(), cancellation, lane, owner })
    }

    pub fn cancel(&self, window: &str, operation: &str) -> Result<bool, String> {
        validate_operation(operation)?;
        let mut current = self.0.lock().map_err(|_| "Core request state unavailable")?;
        if let Some(active) = current.active.iter().flatten().find(|active| active.window == window && active.operation == operation) {
            active.cancellation.cancel();
            return Ok(true);
        }
        if current.active.iter().flatten().any(|active| active.operation == operation) { return Ok(false); }
        current.completed.retain(|(_,_,created)|created.elapsed()<Duration::from_secs(30));
        if current.completed.iter().any(|(owner,id,_)| owner==window && id==operation) { return Ok(false); }
        current.pending.retain(|(_,_,created)|created.elapsed()<Duration::from_secs(30));
        if current.pending.iter().any(|(owner,id,_)|owner==window && id==operation) {return Ok(true);}
        if current.pending.len()>=64 {return Err("Too many pending cancellations".into());}
        current.pending.push((window.into(),operation.into(),Instant::now()));
        Ok(true)
    }
}

impl RequestGuard {
    pub fn cancellation(&self) -> CancellationToken { self.cancellation.clone() }
    pub async fn execute_owned(self, request: serde_json::Value) -> Result<serde_json::Value, String> {
        tauri::async_runtime::spawn(async move { self.execute(request).await }).await
            .map_err(|_| "Core worker failed".to_string())?
    }

    async fn execute(&self, request: serde_json::Value) -> Result<serde_json::Value, String> {
        if self.cancellation.is_cancelled() { return Err("Operation cancelled".into()); }
        if self.lane == Lane::Preview && Lane::for_request(&request) != Lane::Preview {
            return Err("Only allowlisted read-only operations may use the preview lane".into());
        }
        aislide_core::preflight::value(&request, &aislide_core::limits::LARGE, Some(&self.cancellation)).map_err(|error| error.to_string())?;
        aislide_core::preflight::serialized_bytes(&request, aislide_core::limits::LARGE.request_bytes, "native JSON request").map_err(|error| error.to_string())?;
        aislide_core::protocol::execute_request_async(request, self.cancellation()).await.map_err(|error| error.to_string())
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut current) = self.state.0.lock() {
            if current.active[self.lane.index()].as_ref().is_some_and(|active| Arc::ptr_eq(&active.owner, &self.owner)) {
                if let Some(active) = current.active[self.lane.index()].take() {
                    if current.completed.len() == 64 { current.completed.pop_front(); }
                    current.completed.push_back((active.window, active.operation, Instant::now()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RequestGate;

    #[test]
    fn preview_is_bounded_and_independent_of_foreground() {
        let gate = RequestGate::default();
        let preview = serde_json::json!({"op":"render_element_preview"});
        let first = gate.begin_request("main", "preview-one", &preview).unwrap();
        let edit = gate.begin_request("main", "edit-one", &serde_json::json!({"op":"apply_transaction"})).unwrap();
        assert!(gate.begin_request("main", "preview-two", &preview).is_err());
        assert!(gate.begin("main", "edit-two").is_err());
        assert!(gate.cancel("main", "preview-one").unwrap());
        assert!(!edit.cancellation().is_cancelled());
        assert!(gate.begin_request("main", "preview-two", &preview).is_err());
        drop(first);
        assert!(gate.begin_request("main", "preview-two", &preview).is_ok());
        drop(edit);
        let edit = gate.begin("main", "edit-three").unwrap();
        assert!(gate.begin_request("main", "chart", &serde_json::json!({"op":"compute_chart_presentation"})).is_ok());
        for operation in ["apply_transaction", "generate", "export_presentation", "render_slide_preview", "Render_element_preview"] {
            assert!(gate.begin_request("main", "claim", &serde_json::json!({"op":operation,"preview":true,"lane":"preview"})).is_err());
        }
        drop(edit);
    }

    #[test]
    fn full_occupancy_retains_window_scoped_one_use_pending_cancellation() {
        for request in [serde_json::json!({"op":"sample"}), serde_json::json!({"op":"render_element_preview"})] {
            let gate = RequestGate::default();
            let edit = gate.begin("main", "edit").unwrap();
            let preview = gate.begin_request("main", "preview", &serde_json::json!({"op":"render_element_preview"})).unwrap();
            assert!(!gate.cancel("other", "edit").unwrap());
            assert!(!gate.cancel("other", "preview").unwrap());
            assert!(gate.cancel("main", "third").unwrap());
            assert!(gate.begin_request("main", "third", &request).is_err());
            assert_eq!(gate.0.lock().unwrap().pending.len(), 1);
            assert!(!edit.cancellation().is_cancelled());
            assert!(!preview.cancellation().is_cancelled());
            let remaining = if super::Lane::for_request(&request) == super::Lane::Preview {
                drop(preview);
                edit
            } else {
                drop(edit);
                preview
            };
            let other = gate.begin_request("other", "third", &request).unwrap();
            assert!(!other.cancellation().is_cancelled());
            drop(other);
            let late = gate.begin_request("main", "third", &request).unwrap();
            assert!(late.cancellation().is_cancelled());
            assert!(gate.0.lock().unwrap().pending.is_empty());
            assert!(!remaining.cancellation().is_cancelled());
            drop(late);
            assert!(!gate.cancel("main", "third").unwrap());
            let reused = gate.begin_request("main", "third", &request).unwrap();
            assert!(!reused.cancellation().is_cancelled());
            assert!(!remaining.cancellation().is_cancelled());
        }
    }

    #[test]
    fn full_occupancy_pending_cancellations_keep_capacity_and_expire() {
        let gate = RequestGate::default();
        let edit = gate.begin("main", "edit").unwrap();
        let preview = gate.begin_request("main", "preview", &serde_json::json!({"op":"render_element_preview"})).unwrap();
        for index in 0..64 { assert!(gate.cancel("main", &format!("pending-{index}")).unwrap()); }
        assert!(gate.cancel("main", "pending-0").unwrap());
        assert_eq!(gate.0.lock().unwrap().pending.len(), 64);
        assert_eq!(gate.cancel("main", "overflow").unwrap_err(), "Too many pending cancellations");
        for operation in [String::new(), "a".repeat(129), "invalid/id".into()] {
            assert_eq!(gate.cancel("main", &operation).unwrap_err(), "Invalid operation ID");
        }
        assert!(!gate.cancel("other", "edit").unwrap());
        assert!(!gate.cancel("other", "preview").unwrap());
        let expired = std::time::Instant::now() - std::time::Duration::from_secs(31);
        gate.0.lock().unwrap().pending[0].2 = expired;
        assert!(gate.cancel("main", "overflow").unwrap());
        {
            let mut current = gate.0.lock().unwrap();
            assert_eq!(current.pending.len(), 64);
            assert!(!current.pending.iter().any(|(_, operation, _)| operation == "pending-0"));
            for (_, _, created) in &mut current.pending { *created = expired; }
        }
        assert!(gate.cancel("main", "fresh").unwrap());
        assert_eq!(gate.0.lock().unwrap().pending.len(), 1);
        assert!(!edit.cancellation().is_cancelled());
        assert!(!preview.cancellation().is_cancelled());
        assert!(gate.cancel("main", "edit").unwrap());
        assert!(edit.cancellation().is_cancelled());
        assert!(!preview.cancellation().is_cancelled());
    }

    #[test]
    fn expired_pending_and_completed_cancellations_are_collected() {
        let gate = RequestGate::default();
        assert!(gate.cancel("main", "expired").unwrap());
        gate.0.lock().unwrap().pending[0].2 = std::time::Instant::now() - std::time::Duration::from_secs(31);
        let running = gate.begin("main", "expired").unwrap();
        assert!(!running.cancellation().is_cancelled());
        assert!(gate.0.lock().unwrap().pending.is_empty());
        drop(running);
        assert!(!gate.cancel("main", "expired").unwrap());
        gate.0.lock().unwrap().completed[0].2 = std::time::Instant::now() - std::time::Duration::from_secs(31);
        assert!(gate.cancel("main", "expired").unwrap());
        assert!(gate.0.lock().unwrap().completed.is_empty());
        let late = gate.begin("main", "expired").unwrap();
        assert!(late.cancellation().is_cancelled());
        assert!(gate.0.lock().unwrap().pending.is_empty());
    }

    #[test]
    fn preview_cancellation_has_one_owner_and_does_not_leak_late_records() {
        let gate = RequestGate::default();
        let preview = serde_json::json!({"op":"compute_chart_presentation"});
        let edit = gate.begin("main", "edit").unwrap();
        assert!(gate.cancel("main", "early-preview").unwrap());
        let running = gate.begin_request("main", "early-preview", &preview).unwrap();
        assert!(running.cancellation().is_cancelled());
        assert!(!edit.cancellation().is_cancelled());
        assert!(!gate.cancel("other", "early-preview").unwrap());
        assert!(gate.begin_request("main", "edit", &preview).is_err());
        drop(running);
        assert!(!gate.cancel("main", "early-preview").unwrap());
        drop(edit);
        for index in 0..100 {
            let id = format!("preview-{index}");
            drop(gate.begin_request("main", &id, &preview).unwrap());
            assert!(!gate.cancel("main", &id).unwrap());
        }
        assert!(gate.0.lock().unwrap().pending.is_empty());
        assert_eq!(gate.0.lock().unwrap().completed.len(), 64);
    }

    #[test]
    fn preview_guard_rejects_replaced_mutation_payload() {
        tauri::async_runtime::block_on(async {
            let gate = RequestGate::default();
            let guard = gate.begin_request("main", "preview", &serde_json::json!({"op":"render_element_preview"})).unwrap();
            let error = guard.execute(serde_json::json!({"op":"create_presentation","id":"forbidden","title":"Mutation","preview":true})).await.unwrap_err();
            assert!(error.contains("allowlisted read-only"));
        });
    }

    #[test]
    fn cancelled_worker_retains_preview_slot_until_worker_exit() {
        let gate = RequestGate::default();
        let preview = serde_json::json!({"op":"render_element_preview"});
        let guard = gate.begin_request("main", "worker", &preview).unwrap();
        let (entered, started) = std::sync::mpsc::channel();
        let (release, finished) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            entered.send(()).unwrap();
            finished.recv().unwrap();
            assert!(guard.cancellation().is_cancelled());
            drop(guard);
        });
        started.recv().unwrap();
        assert!(gate.cancel("main", "worker").unwrap());
        assert!(gate.begin_request("main", "second", &preview).is_err());
        assert!(gate.begin("main", "save").is_ok());
        release.send(()).unwrap();
        worker.join().unwrap();
        assert!(gate.begin_request("main", "second", &preview).is_ok());
    }

    #[test]
    fn stale_guard_cannot_release_replacement_owner() {
        let gate = RequestGate::default();
        let old = gate.begin("main", "old").unwrap();
        gate.0.lock().unwrap().active[0].take();
        let replacement = gate.begin("main", "replacement").unwrap();
        drop(old);
        assert!(gate.begin("main", "third").is_err());
        assert!(!replacement.cancellation().is_cancelled());
        drop(replacement);
        assert!(gate.begin("main", "third").is_ok());
    }

    #[test]
    fn native_capacity_uses_core_profiles_and_preserves_cancellation() {
        tauri::async_runtime::block_on(async {
            let gate = RequestGate::default();
            let guard = gate.begin("g38", "synthetic").unwrap();
            let document = guard.execute(serde_json::json!({"op":"create_presentation","id":"native-capacity","title":"Synthetic"})).await.unwrap();
            let mut deck = document["deck"].clone();
            let slide = deck["slides"][0].clone();
            deck["slides"] = serde_json::json!((0..256).map(|index| { let mut copy = slide.clone(); copy["id"] = serde_json::json!(format!("s{index}")); copy }).collect::<Vec<_>>());
            assert!(guard.execute(serde_json::json!({"op":"validate","deck":deck})).await.is_ok());
            assert!(guard.execute(serde_json::json!({"op":"validate","capacity_profile":"legacy","deck":deck})).await.is_err());
            assert!(guard.execute(serde_json::json!({"op":"validate","capacity_profile":"standard","deck":deck})).await.is_err());
            assert!(guard.execute(serde_json::json!({"op":"sample","capacity_profile":"legacy","extra":"a".repeat(aislide_core::limits::LEGACY.request_bytes)})).await.unwrap_err().contains("JSON request"));
            gate.cancel("g38", "synthetic").unwrap();
            assert!(guard.execute(serde_json::json!({"op":"sample"})).await.unwrap_err().contains("cancelled"));
        });
    }

    #[test]
    fn cancellation_is_scoped_to_window_and_operation() {
        let gate = RequestGate::default();
        let running = gate.begin("main", "operation-one").unwrap();
        assert!(gate.begin("main", "operation-two").is_err());
        assert!(!gate.cancel("other-window", "operation-one").unwrap());
        assert!(gate.cancel("main", "not-yet-registered-preview").unwrap());
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