use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::state::FeeConfig;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub terraland_token: String,
    pub fee_config: Vec<FeeConfig>,
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
        owner: Option<String>,
        fee_config: Option<Vec<FeeConfig>>,
        mission_smart_contracts: Option<InstantiateMissionSmartContracts>,
    },
    Claim {},
    RegisterMembers (
        Vec<RegisterMemberItem>
    ),
    UstWithdraw {
        recipient: String,
        amount: Uint128
    },
    TokenWithdraw {
        token: String,
        recipient: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    Member {
        address: String
    },
    ListMembers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct RegisterMemberItem {
    pub address: String,
    pub amount: Uint128,
    pub claimed: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberResponseItem {
    pub amount: Uint128,
    pub available_to_claim: Uint128,
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
pub struct MemberListResponseItem {
    pub address: String,
    pub info: MemberResponseItem,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberResponse {
    pub member: Option<MemberResponseItem>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberListResponse {
    pub members: Vec<MemberListResponseItem>,
}
