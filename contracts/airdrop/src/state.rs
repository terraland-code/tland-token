use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub terraland_token: Addr,
    pub mission_smart_contracts: MissionSmartContracts,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MissionSmartContracts {
    pub lp_staking: Option<Addr>,
    pub tland_staking: Option<Addr>,
    pub property_shareholders: Option<Addr>,
    pub platform_users: Option<Addr>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Member {
    pub amount: Uint128,
    pub claimed: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const MEMBERS: Map<&Addr, Member> = Map::new("members");
