use cosmwasm_std::{Addr, Uint128};
use cw_controllers::Claims;
use cw_storage_plus::{Item, Map, U8Key};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Config {
    pub staking_token: String,
    pub terraland_token: String,
    pub unbonding_period: u64,
    pub burn_address: String,
    pub instant_claim_percentage_loss: u64,
    pub start_time: u64,
    pub end_time: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Stake {
    pub amount: Uint128,
    pub time: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const TOTAL: Item<Stake> = Item::new("total");
pub const CLAIMS: Claims = Claims::new("claims");

pub const STAKE: Map<&Addr, Stake> = Map::new("stake");
pub const WITHDRAWN: Map<&Addr, Uint128> = Map::new("withdrawn");

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Epoch {
    pub amount: Uint128,
    pub start_time: u64,
    pub end_time: u64,
}

pub const EPOCHS: Map<U8Key, Epoch> = Map::new("epochs");

// Weight map for member for epoch
pub const MEMBERS_WEIGHT: Map<(U8Key, &Addr), u128> = Map::new("members");
// Weight map for epoch
pub const EPOCHS_WEIGHT: Map<U8Key, u128> = Map::new("epochs_weight");



