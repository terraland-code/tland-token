use std::cmp;
use std::collections::HashMap;
use std::ops::Div;

use cosmwasm_std::{Addr, Binary, Deps, DepsMut, Env, from_slice, MessageInfo, Response, StdError, StdResult, Storage, SubMsg, to_binary, Uint128, WasmMsg};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw0::Duration;
use cw20::{Balance, Cw20CoinVerified, Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw2::set_contract_version;
use cw_storage_plus::U8Key;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg,
                 ReceiveMsg, RewardResponse, StakedResponse, TotalResponse, WithdrawnResponse};
use crate::state::{CLAIMS, Config, CONFIG, Epoch, EPOCHS, EPOCHS_WEIGHT, MEMBERS_WEIGHT, STAKE, Stake, TOTAL, WITHDRAWN};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:fcq-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const WEEK: u64 = 7 * 24 * 3600;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut i: u8 = 0;
    for schedule in msg.distribution_schedule.clone().into_iter() {
        let weeks = (schedule.end_time - schedule.start_time) / WEEK;
        let amount = schedule.amount.div(Uint128::new(weeks as u128));
        for n in 0..weeks {
            i += 1;
            EPOCHS.save(deps.storage, U8Key::from(i as u8), &Epoch {
                amount,
                start_time: schedule.start_time + n * WEEK,
                end_time: schedule.start_time + (n + 1) * WEEK,
            })?;
        }
    }

    let config = Config {
        staking_token: msg.staking_token,
        terraland_token: msg.terraland_token,
        unbonding_period: msg.unbonding_period,
        burn_address: msg.burn_address,
        instant_claim_percentage_loss: msg.instant_claim_percentage_loss,
        start_time: msg.distribution_schedule.first().unwrap().start_time,
        end_time: msg.distribution_schedule.last().unwrap().end_time,
    };
    CONFIG.save(deps.storage, &config)?;
    TOTAL.save(deps.storage, &Stake { amount: Uint128::new(0), time: 0 })?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::Unbond { tokens: amount } => execute_unbond(deps, env, info, amount),
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
        ExecuteMsg::InstantClaim {} => execute_instant_claim(deps, env, info),
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),
    }
}

pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // info.sender is the address of the cw20 contract (that re-sent this message).
    // wrapper.sender is the address of the user that requested the cw20 contract to send this.
    // This cannot be fully trusted (the cw20 contract can fake it), so only use it for actions
    // in the address's favor (like paying/bonding tokens, not withdrawls)
    let msg: ReceiveMsg = from_slice(&wrapper.msg)?;
    let balance = Balance::Cw20(Cw20CoinVerified {
        address: info.sender,
        amount: wrapper.amount,
    });
    let api = deps.api;
    match msg {
        ReceiveMsg::Bond {} => {
            execute_bond(deps, env, balance, api.addr_validate(&wrapper.sender)?)
        }
    }
}

pub fn execute_bond(
    deps: DepsMut,
    env: Env,
    amount: Balance,
    sender: Addr,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if env.block.time.seconds() < cfg.start_time || env.block.time.seconds() > cfg.end_time {
        return Err(ContractError::StakingClosed {});
    }

    // ensure the sent token was proper
    let amount = match &amount {
        Balance::Cw20(token) => {
            if token.address == cfg.staking_token {
                Ok(token.amount)
            } else {
                Err(ContractError::InvalidToken(token.address.to_string()))
            }
        }
        _ => Err(ContractError::MissedToken {})
    }?;

    let old_stake = STAKE.may_load(deps.storage, &sender)?;
    STAKE.update(deps.storage, &sender, |stake| -> StdResult<_> {
        let mut val = amount;
        if stake.is_some() {
            val += stake.unwrap().amount
        }
        Ok(Stake { amount: val, time: env.block.time.seconds() })
    })?;

    // update total stake
    let old_total = TOTAL.load(deps.storage)?;
    TOTAL.update(deps.storage, |mut total| -> StdResult<_> {
        total.amount += amount;
        total.time = env.block.time.seconds();
        Ok(total)
    })?;

    update_weight(
        deps.storage,
        &sender,
        old_stake,
        old_total,
        env.block.time.seconds(),
        &cfg,
    )?;

    Ok(Response::new()
        .add_attribute("action", "bond")
        .add_attribute("amount", amount)
        .add_attribute("sender", sender))
}

fn update_weight(
    storage: &mut dyn Storage,
    sender: &Addr,
    stake: Option<Stake>,
    total: Stake,
    time: u64,
    cfg: &Config,
) -> StdResult<()> {
    if stake.is_some() {
        let member_diffs = calc_weight_diffs(storage, stake.unwrap(), cfg.start_time, time)?;

        for (epoch_id, weight_diff) in member_diffs {
            MEMBERS_WEIGHT.update(storage, (U8Key::from(epoch_id), &sender), |weight| -> StdResult<_> {
                Ok(weight.unwrap_or_default() + weight_diff)
            })?;
        }
    }

    let epoch_diffs = calc_weight_diffs(storage, total, cfg.start_time, time)?;

    for (epoch_id, weight_diff) in epoch_diffs {
        EPOCHS_WEIGHT.update(storage, U8Key::from(epoch_id as u8), |weight| -> StdResult<_> {
            Ok(weight.unwrap_or_default() + weight_diff)
        })?;
    }

    Ok(())
}

fn calc_weight_diffs(
    storage: &dyn Storage,
    stake: Stake,
    start_time: u64,
    time: u64,
) -> StdResult<HashMap<u8, u128>> {
    if stake.amount.is_zero() {
        return Ok(HashMap::new());
    }

    let start_epoch_id = (stake.time - start_time) / WEEK + 1;
    let end_epoch_id = (time - start_time) / WEEK + 1;

    let mut weight_diffs = HashMap::new();
    for epoch_id in start_epoch_id..=end_epoch_id {
        let epoch = EPOCHS.load(storage, U8Key::from(epoch_id as u8))?;
        let weight_diff = calc_weight_diff(&epoch, &stake, time)?;
        weight_diffs.insert(epoch_id as u8, weight_diff);
    }

    Ok(weight_diffs)
}

fn calc_weight_diff(epoch: &Epoch, stake: &Stake, until_time: u64) -> StdResult<u128> {
    let start = cmp::max(epoch.start_time, stake.time);
    let end = cmp::min(epoch.end_time, until_time);

    if start < end {
        return Ok(Uint128::new(0).u128());
    }

    let res = stake.amount.checked_mul(Uint128::from(end - start))?;

    Ok(res.u128())
}

pub fn execute_unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let old_stake = STAKE.may_load(deps.storage, &info.sender)?;
    let old_total = TOTAL.load(deps.storage)?;

    // reduce the sender's stake - aborting if insufficient
    STAKE.update(deps.storage, &info.sender, |stake| -> StdResult<_> {
        let val = stake.unwrap().amount.checked_sub(amount)?;
        Ok(Stake { amount: val, time: env.block.time.seconds() })
    })?;

    // reduce the total stake - aborting if insufficient
    TOTAL.update(deps.storage, |total| -> StdResult<_> {
        let val = total.amount.checked_sub(amount)?;
        Ok(Stake { amount: val, time: env.block.time.seconds() })
    })?;

    // provide them a claim
    let cfg = CONFIG.load(deps.storage)?;
    CLAIMS.create_claim(
        deps.storage,
        &info.sender,
        amount,
        Duration::Time(cfg.unbonding_period).after(&env.block),
    )?;

    update_weight(
        deps.storage,
        &info.sender,
        old_stake,
        old_total,
        env.block.time.seconds(),
        &cfg,
    )?;

    Ok(Response::new()
        .add_attribute("action", "unbond")
        .add_attribute("amount", amount)
        .add_attribute("sender", info.sender))
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // get amount of tokens to release
    let release = CLAIMS.claim_tokens(deps.storage, &info.sender, &env.block, None)?;
    if release.is_zero() {
        return Err(ContractError::NothingToClaim {});
    }

    // create message to transfer staking tokens
    let config = CONFIG.load(deps.storage)?;
    let transfer = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.clone().into(),
        amount: release,
    };
    let message = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.staking_token.clone().into(),
        msg: to_binary(&transfer)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(message)
        .add_attribute("action", "claim")
        .add_attribute("tokens", coin_to_string(release, config.staking_token.as_str()))
        .add_attribute("sender", info.sender))
}

pub fn execute_instant_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Create block after unbonding_period to be able to release all claims
    let mut block = env.block.clone();
    block.time = block.time.plus_seconds(config.unbonding_period);

    // get amount of tokens to release
    let mut release = CLAIMS.claim_tokens(deps.storage, &info.sender, &block, None)?;
    if release.is_zero() {
        return Err(ContractError::NothingToClaim {});
    }

    // calculate fee for instant claim
    let fee = release
        .checked_mul(Uint128::from(config.instant_claim_percentage_loss))
        .map_err(StdError::overflow)?
        .div(Uint128::new(100));
    release = release.checked_sub(fee)
        .map_err(StdError::overflow)?;

    // create message to release staking tokens to owner
    let transfer = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.clone().into(),
        amount: release,
    };
    let message1 = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.staking_token.clone().into(),
        msg: to_binary(&transfer)?,
        funds: vec![],
    });

    // create message to transfer fee to burn address
    let transfer_fee = Cw20ExecuteMsg::Transfer {
        recipient: config.burn_address.clone().into(),
        amount: fee,
    };
    let message2 = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.staking_token.clone().into(),
        msg: to_binary(&transfer_fee)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessages([message1, message2])
        .add_attribute("action", "instant_claim")
        .add_attribute("tokens", coin_to_string(release, config.staking_token.as_str()))
        .add_attribute("fee", coin_to_string(fee, config.staking_token.as_str()))
        .add_attribute("sender", info.sender))
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let amount = calc_reward(deps.storage, &info.sender, env.block.time.seconds())?;

    // update withdrawal
    WITHDRAWN.update(deps.storage, &info.sender, |withdrawal| -> StdResult<_> {
        Ok(withdrawal.unwrap_or_default().checked_add(amount)?)
    })?;

    // create message to transfer reward in fcqn tokens
    let config = CONFIG.load(deps.storage)?;
    let transfer = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.clone().into(),
        amount,
    };
    let message = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.terraland_token.clone(),
        msg: to_binary(&transfer)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(message)
        .add_attribute("action", "withdraw")
        .add_attribute("tokens", coin_to_string(amount, config.terraland_token.as_str()))
        .add_attribute("sender", info.sender))
}

fn calc_reward(
    storage: &dyn Storage,
    addr: &Addr,
    time: u64,
) -> StdResult<Uint128> {
    let cfg = CONFIG.load(storage)?;

    let last_stake = STAKE.may_load(storage, &addr)?;
    if last_stake.is_none() {
        return Ok(Uint128::new(0));
    }
    let last_total = TOTAL.load(storage)?;

    // calculate weight_diffs for epochs since last stake until time
    let member_diffs = calc_weight_diffs(storage, last_stake.unwrap(), cfg.start_time, time)?;
    let epoch_diffs = calc_weight_diffs(storage, last_total, cfg.start_time, time)?;

    // calculate current epoch
    let current_epoch_id = (time - cfg.start_time) / WEEK + 1;

    // calculate reward for every epoch and sum
    let mut reward = Uint128::new(0);
    for epoch_id in 1..=current_epoch_id {
        let epoch = EPOCHS.load(storage, U8Key::new(epoch_id as u8))?;
        let mut epoch_weight = EPOCHS_WEIGHT.may_load(storage, U8Key::new(epoch_id as u8))?.unwrap_or_default();
        let mut member_weight = MEMBERS_WEIGHT.may_load(storage, (U8Key::new(epoch_id as u8), &addr))?.unwrap_or_default();

        epoch_weight += epoch_diffs.get(&(epoch_id as u8)).unwrap_or(&(0 as u128));
        member_weight += member_diffs.get(&(epoch_id as u8)).unwrap_or(&(0 as u128));

        let mut amount = epoch.amount;

        // if current epoch then only part of epoch amount is available for distribution
        if epoch_id == current_epoch_id {
            // amount multiplied by percentage of epoch elapsed time
            amount = amount
                .checked_mul(Uint128::from(time - epoch.start_time))
                .map_err(StdError::overflow)?
                .div(Uint128::from(epoch.end_time - epoch.start_time));
        }

        // member reward is proportional to member weight
        let member_reward = amount
            .checked_mul(Uint128::from(member_weight))
            .map_err(StdError::overflow)?
            .div(Uint128::from(epoch_weight));

        reward += member_reward;
    }

    Ok(reward)
}

#[inline]
fn coin_to_string(amount: Uint128, denom: &str) -> String {
    format!("{} {}", amount, denom)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Total {} => to_binary(&query_total(deps)?),
        QueryMsg::Claims { address } => {
            to_binary(&CLAIMS.query_claims(deps, &deps.api.addr_validate(&address)?)?)
        }
        QueryMsg::Staked { address } => to_binary(&query_staked(deps, address)?),
        QueryMsg::Reward { address } => to_binary(&query_reward(deps, env, address)?),
        QueryMsg::Withdrawn { address } => to_binary(&query_withdrawn(deps, address)?),
    }
}

fn query_total(deps: Deps) -> StdResult<TotalResponse> {
    let total = TOTAL.load(deps.storage)?;
    Ok(TotalResponse { total: total.amount })
}

pub fn query_staked(deps: Deps, addr: String) -> StdResult<StakedResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let denom = CONFIG.load(deps.storage)?.staking_token;
    let stake = STAKE.may_load(deps.storage, &addr)?;
    if let Some(v) = stake {
        Ok(StakedResponse { stake: v.amount, denom })
    } else {
        Ok(StakedResponse { stake: Uint128::new(0), denom })
    }
}

fn query_reward(deps: Deps, env: Env, addr: String) -> StdResult<RewardResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let reward = calc_reward(deps.storage, &addr, env.block.time.seconds())?;
    Ok(RewardResponse { reward })
}

fn query_withdrawn(deps: Deps, addr: String) -> StdResult<WithdrawnResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let withdrawn = WITHDRAWN.may_load(deps.storage, &addr)?.unwrap_or_default();
    Ok(WithdrawnResponse { withdrawn })
}

#[cfg(test)]
mod tests {}
