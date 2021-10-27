use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_controllers::Claim;
pub use cw_controllers::ClaimsResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Schedule};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub staking_token: String,
    pub terraland_token: String,
    pub unbonding_period: u64,
    pub burn_address: String,
    pub instant_claim_percentage_loss: u64,
    pub distribution_schedule: Vec<Schedule>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct NewConfig {
    pub owner: Option<String>,
    pub unbonding_period: Option<u64>,
    pub burn_address: Option<String>,
    pub instant_claim_percentage_loss: Option<u64>,
    pub distribution_schedule: Option<Vec<Schedule>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Update config parameters
    UpdateConfig ( NewConfig ),
    /// Unbond will start the unbonding process for the given number of tokens.
    /// The sender immediately loses weight from these tokens, and can claim them
    /// back to his wallet after `unbonding_period`
    Unbond { tokens: Uint128 },
    /// Claim is used to claim your native tokens that you previously "unbonded"
    /// after the contract-defined waiting period (eg. 1 week)
    Claim {},
    /// Claim without waiting period, but with percentage fee
    InstantClaim {},
    /// Withdraw reward
    Withdraw {},

    /// This accepts a properly-encoded ReceiveMsg from a cw20 contract
    Receive(Cw20ReceiveMsg),

    /// Withdraw ust from smart contract by owner
    UstWithdraw {
        recipient: String,
    },
    /// Withdraw tokens from smart contract by owner
    TokenWithdraw {
        token: String,
        recipient: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    /// Only valid cw20 message is to bond the tokens
    Bond {},
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Return config
    Config {},

    /// Return state
    State {},

    /// Return staker info
    Member { address: String },

    /// Return stakers
    ListMembers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberResponseItem {
    pub stake: Uint128,
    pub reward: Uint128,
    pub reward_index: Decimal,
    pub withdrawn: Uint128,
    pub claims: Vec<Claim>,
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
