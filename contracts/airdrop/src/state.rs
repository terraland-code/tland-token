use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub terraland_token: Addr,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MemberAirdrop {
    pub amount: Uint128,
    pub claimed: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const MEMBERS: Map<&Addr, MemberAirdrop> = Map::new("members");
