use serde::{Deserialize, Serialize};

/// Knowledge a step produced that later steps must respect ("use JWT for the reset token").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)] // `decision` is what the decision says
pub struct Decision {
    pub id: String,
    pub title: String,
    pub decision: String,
    pub rationale: String,
    pub source_node_id: String,
    pub created_at: u64,
}
