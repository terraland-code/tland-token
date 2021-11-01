use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub terraland_token: Addr,
    pub name: String,
    pub fee_config: Vec<FeeConfig>,
    pub vesting: Vesting,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Vesting {
    pub start_time: u64,
    pub end_time: u64,
    pub initial_percentage: u64,
    pub cliff_end_time: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct FeeConfig {
    pub fee: Uint128,
    pub operation: String,
    pub denom: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Member {
    pub amount: Uint128,
    pub claimed: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct State {
    pub num_of_members: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const MEMBERS: Map<&Addr, Member> = Map::new("members");
pub const STATE: Item<State> = Item::new("state");
