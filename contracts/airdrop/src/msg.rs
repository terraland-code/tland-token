use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub terraland_token: String,
    pub mission_smart_contracts: Option<InstantiateMissionSmartContracts>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMissionSmartContracts {
    pub lp_staking: Option<String>,
    pub tland_staking: Option<String>,
    pub platform_registry: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        new_owner: Option<String>,
        mission_smart_contracts: Option<InstantiateMissionSmartContracts>,
    },
    Claim {},
    RegisterMembers {
        members: Vec<NewMember>
    },
    UstWithdraw {
        recipient: String,
    },
    TokenWithdraw {
        token: String,
        recipient: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Member {
        address: String
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct NewMember {
    pub address: String,
    pub amount: Uint128,
    pub claimed: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberStats {
    pub amount: Uint128,
    pub claimed: Uint128,
    pub passed_missions: Missions,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Missions {
    pub is_in_lp_staking: bool,
    pub is_in_tland_staking: bool,
    pub is_registered_on_platform: bool,
    pub is_property_shareholder: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberResponse {
    pub stats: Option<MemberStats>,
}
