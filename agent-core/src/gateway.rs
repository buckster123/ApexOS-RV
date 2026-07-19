//! Gateway control frames — the two non-`Event` message shapes on the `/ws`
//! contract (upstream CLAUDE.md §agentd WebSocket protocol): the gateway's
//! `session_init` push and the client's optional `hello`. Everything else on
//! that wire is a raw `apexos-protocol` `Event`.

use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Gateway → client, pushed on connect (and again after a `hello`).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GatewayControl {
    SessionInit {
        session_id: u64,
        /// Replayed history — opaque to the metal node in v2.
        #[serde(default)]
        history: serde_json::Value,
    },
}

/// Client → gateway session control.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientControl {
    Hello {
        #[serde(skip_serializing_if = "Option::is_none")]
        resume_session: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        new: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        persona: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_init_parses_the_documented_shape() {
        let s = r#"{"type": "session_init", "session_id": 42, "history": []}"#;
        let GatewayControl::SessionInit { session_id, history } =
            serde_json::from_str(s).unwrap();
        assert_eq!(session_id, 42);
        assert!(history.as_array().is_some_and(|a| a.is_empty()));
    }

    #[test]
    fn hello_frames_serialize_the_documented_shapes() {
        let new = ClientControl::Hello {
            resume_session: None,
            new: Some(true),
            agent_id: None,
            persona: None,
        };
        assert_eq!(
            serde_json::to_value(&new).unwrap(),
            serde_json::json!({"type": "hello", "new": true})
        );
        let resume = ClientControl::Hello {
            resume_session: Some(42),
            new: None,
            agent_id: None,
            persona: None,
        };
        assert_eq!(
            serde_json::to_value(&resume).unwrap(),
            serde_json::json!({"type": "hello", "resume_session": 42})
        );
    }
}
