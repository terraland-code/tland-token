use cosmwasm_std::Uint128;
use cw0::Duration;
use cw20::Cw20ReceiveMsg;
pub use cw_controllers::ClaimsResponse;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub staking_token: String,
    pub fcqn_token: String,
    pub unbonding_period: Duration,
    pub burn_address: String,
    pub instant_claim_percentage_loss: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
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
    /// Claims shows the tokens in process of unbonding for this address
    Claims {
        address: String,
    },
    /// Show the number of tokens currently staked by this address.
    Staked {
        address: String,
    },
    /// Show the number of reward to withdraw
    Reward {
        address: String,
    },

    /// Return TotalWeightResponse
    TotalWeight {},
    /// Returns MembersListResponse
    ListMembers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns MemberResponse
    Member {
        addr: String,
        at_height: Option<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct StakedResponse {
    pub stake: Uint128,
    pub denom: String,
}
