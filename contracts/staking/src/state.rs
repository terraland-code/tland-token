use cosmwasm_std::{Uint128, Addr};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw0::Duration;
use cw_controllers::Claims;
use cw_storage_plus::{Item, Map, SnapshotMap, Strategy};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Config {
    pub staking_token: String,
    pub fcqn_token: String,
    pub unbonding_period: Duration,
    pub burn_address: String,
    pub instant_claim_percentage_loss: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const TOTAL: Item<u64> = Item::new("total");
pub const CLAIMS: Claims = Claims::new("claims");

pub const MEMBERS: SnapshotMap<&Addr, u64> = SnapshotMap::new(
    cw4::MEMBERS_KEY,
    cw4::MEMBERS_CHECKPOINTS,
    cw4::MEMBERS_CHANGELOG,
    Strategy::EveryBlock,
);

pub const STAKE: Map<&Addr, Uint128> = Map::new("stake");

