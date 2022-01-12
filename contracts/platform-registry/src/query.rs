use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PlatformRegistryQueryMsg {
    /// Returns whether the address is registered and has estate shares
    AddressBaseInfo {
        address: String
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct AddressBaseInfoResponse {
    pub is_registered: bool,
    pub is_property_buyer: bool,
}
