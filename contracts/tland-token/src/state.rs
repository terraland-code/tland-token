use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SwapFeeConfig {
    pub fee_admin: Addr,
    /// The percent fee amount from every token swap to any other
    pub enable_swap_fee: bool,
    /// The percent fee amount from every token swap to any other
    pub swap_percent_fee: Decimal,
    /// The fee receiver address
    pub fee_receiver: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const SWAP_FEE_CONFIG: Item<SwapFeeConfig> = Item::new("swap_fee_config");
