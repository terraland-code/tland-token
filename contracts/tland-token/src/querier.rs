use std::ops::Sub;
use cosmwasm_std::{Coin, Decimal, DepsMut, StdError, Uint128};
use cw20_base::ContractError;
use terra_cosmwasm::TerraQuerier;

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

pub fn compute_tax(deps: DepsMut, coin: &Coin) -> Result<Uint128, ContractError> {
    let amount = coin.amount;
    let denom = coin.denom.clone();

    if denom == "uluna" {
        Ok(Uint128::zero())
    } else {
        let terra_querier = TerraQuerier::new(&deps.querier);
        let tax_rate: Decimal = (terra_querier.query_tax_rate()?).rate;
        let tax_cap: Uint128 = (terra_querier.query_tax_cap(denom)?).cap;
        Ok(std::cmp::min(
            amount.checked_sub(amount.multiply_ratio(
                DECIMAL_FRACTION,
                DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
            )).map_err(StdError::overflow)?,
            tax_cap,
        ))
    }
}

pub fn deduct_tax(deps: DepsMut, coin: Coin) -> Result<Coin, ContractError> {
    Ok(Coin {
        denom: coin.denom.clone(),
        amount: coin.amount.sub(compute_tax(deps, &coin)?),
    })
}
