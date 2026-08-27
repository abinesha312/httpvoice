use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CallStatus {
    Queued,
    Ringing,
    #[serde(rename = "in-progress")]
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    pub id: Uuid,
    pub to: String,
    pub from: String,
    pub url: Option<String>,
    pub status: CallStatus,
}

#[derive(Debug)]
pub struct CallStore {
    calls: RwLock<HashMap<Uuid, Call>>,
}

impl CallStore {
    pub fn new() -> Self {
        Self {
            calls: RwLock::new(HashMap::new()),
        }
    }

    pub fn create(&self, to: String, from: String, url: Option<String>) -> Call {
        let call = Call {
            id: Uuid::new_v4(),
            to,
            from,
            url,
            status: CallStatus::Queued,
        };

        self.calls.write().insert(call.id, call.clone());
        call
    }

    pub fn get(&self, id: &Uuid) -> Option<Call> {
        self.calls.read().get(id).cloned()
    }

    pub fn update_status(&self, id: &Uuid, status: CallStatus) -> bool {
        let mut calls = self.calls.write();
        if let Some(call) = calls.get_mut(id) {
            call.status = status;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_store() {
        let store = CallStore::new();
        let call = store.create(
            "sip:user@example.com".to_string(),
            "sip:from@example.com".to_string(),
            None,
        );

        assert_eq!(call.status, CallStatus::Queued);

        let retrieved = store.get(&call.id).unwrap();
        assert_eq!(retrieved.id, call.id);

        assert!(store.update_status(&call.id, CallStatus::Ringing));
        let updated = store.get(&call.id).unwrap();
        assert_eq!(updated.status, CallStatus::Ringing);
    }
}
