use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw0::Duration;
use cw_controllers::Claims;
use cw_storage_plus::{Item, Map, SnapshotMap, Strategy};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Config {
    pub staking_token: String,
    pub fcqn_token: String,
    pub unbonding_period: Duration,
    pub burn_address: String,
    pub instant_claim_percentage_loss: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Snapshot {
    pub stake: Uint128,
    pub weight: u64,
    pub time: Timestamp,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const TOTAL: Item<Uint128> = Item::new("total");
pub const CLAIMS: Claims = Claims::new("claims");

pub const MEMBERS: SnapshotMap<&Addr, Snapshot> = SnapshotMap::new(
    cw4::MEMBERS_KEY,
    cw4::MEMBERS_CHECKPOINTS,
    cw4::MEMBERS_CHANGELOG,
    Strategy::EveryBlock,
);

pub const STAKE: Map<&Addr, Uint128> = Map::new("stake");
pub const WITHDRAWN: Map<&Addr, Uint128> = Map::new("withdrawn");
