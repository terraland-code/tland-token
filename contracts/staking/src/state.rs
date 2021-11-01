use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_controllers::Claims;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Config {
    pub owner: Addr,
    pub staking_token: Addr,
    pub terraland_token: Addr,
    pub unbonding_period: u64,
    pub burn_address: Addr,
    pub instant_claim_percentage_loss: u64,
    pub distribution_schedule: Vec<Schedule>,
    pub fee_config: Vec<FeeConfig>,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Schedule {
    pub amount: Uint128,
    pub start_time: u64,
    pub end_time: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct FeeConfig {
    pub fee: Uint128,
    pub operation: String,
    pub denom: String,
}

#[derive(Default, Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberInfo {
    pub stake: Uint128,
    pub pending_reward: Uint128,
    pub reward_index: Decimal,
    pub withdrawn: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct State {
    pub total_stake: Uint128,
    pub last_updated: u64,
    pub global_reward_index: Decimal,
    pub num_of_members: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const MEMBERS: Map<&Addr, MemberInfo> = Map::new("members");
pub const STATE: Item<State> = Item::new("state");
pub const CLAIMS: Claims = Claims::new("claims");
