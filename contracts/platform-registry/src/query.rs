use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PlatformRegistryQueryMsg {
    /// Returns whether the address is associated with registered user.
    IsRegistered { address: String },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct IsRegisteredResponse {
    pub is_registered: bool,
}
