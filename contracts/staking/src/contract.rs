use std::ops::Div;

use cosmwasm_std::{Addr, Binary, BlockInfo, Deps, DepsMut, Env, from_slice, MessageInfo, Order, Response, StdError, StdResult, Storage, SubMsg, Timestamp, to_binary, Uint128, WasmMsg};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw0::{Duration, maybe_addr};
use cw20::{Balance, Cw20CoinVerified, Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw2::set_contract_version;
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, Member, MemberListResponse, MemberResponse, QueryMsg,
                 ReceiveMsg, RewardResponse, StakedResponse, TotalResponse, WithdrawnResponse};
use crate::state::{CLAIMS, Config, CONFIG, MEMBERS, Snapshot, STAKE, TOTAL, WITHDRAWN};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:fcq-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        staking_token: msg.staking_token,
        fcqn_token: msg.fcqn_token,
        unbonding_period: msg.unbonding_period,
        burn_address: msg.burn_address,
        instant_claim_percentage_loss: msg.instant_claim_percentage_loss,
    };
    CONFIG.save(deps.storage, &config)?;
    TOTAL.save(deps.storage, &Uint128::new(0))?;

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

    // update the sender's stake
    let new_stake = STAKE.update(deps.storage, &sender, |stake| -> StdResult<_> {
        Ok(stake.unwrap_or_default().checked_sub(amount)?)
    })?;

    update_membership(
        deps.storage,
        sender.clone(),
        new_stake,
        env.block,
    )?;

    Ok(Response::new()
        .add_attribute("action", "bond")
        .add_attribute("amount", amount)
        .add_attribute("sender", sender))
}

fn update_membership(
    storage: &mut dyn Storage,
    sender: Addr,
    stake: Uint128,
    block: BlockInfo,
) -> StdResult<Option<u64>> {
    let snapshot = MEMBERS.may_load(storage, &sender)?;
    let new_weight = calc_weight(snapshot.clone(), block.time);
    let new_snapshot = Snapshot {
        stake,
        weight: new_weight,
        time: block.time,
    };

    // record change of weight
    MEMBERS.save(storage, &sender, &new_snapshot, block.height)?;

    // update total
    let mut snapshot_stake = Uint128::new(0);
    if snapshot.is_some() {
        snapshot_stake = snapshot.unwrap().stake
    }
    TOTAL.update(storage, |total| -> StdResult<_> {
        Ok(total + stake - snapshot_stake)
    })?;

    Ok(Option::from(new_weight))
}

fn calc_weight(snapshot: Option<Snapshot>, now: Timestamp) -> u64 {
    if snapshot.is_none() {
        return 0;
    }
    let snap = snapshot.unwrap();

    let time_diff = now.seconds() - snap.time.seconds();

    // weight = number of tokens * staked hours
    let mut w = (snap.stake.u128() / (1_000_000)) as u64;
    w = w * (time_diff / 3600);

    return snap.weight + w;
}

pub fn execute_unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // reduce the sender's stake - aborting if insufficient
    let new_stake = STAKE.update(deps.storage, &info.sender, |stake| -> StdResult<_> {
        Ok(stake.unwrap_or_default().checked_sub(amount)?)
    })?;

    // provide them a claim
    let cfg = CONFIG.load(deps.storage)?;
    CLAIMS.create_claim(
        deps.storage,
        &info.sender,
        amount,
        cfg.unbonding_period.after(&env.block),
    )?;

    update_membership(
        deps.storage,
        info.sender.clone(),
        new_stake,
        env.block,
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
    match config.unbonding_period {
        Duration::Height(v) => { block.height = block.height + v; }
        Duration::Time(v) => { block.time = block.time.plus_seconds(v); }
    };

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
    let amount = calc_reward(deps.storage, &info.sender, env.block.time)?;

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
        contract_addr: config.fcqn_token.clone(),
        msg: to_binary(&transfer)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(message)
        .add_attribute("action", "withdraw")
        .add_attribute("tokens", coin_to_string(amount, config.fcqn_token.as_str()))
        .add_attribute("sender", info.sender))
}

fn calc_reward(
    storage: &dyn Storage,
    addr: &Addr,
    now: Timestamp,
) -> StdResult<Uint128> {
    let snapshot = MEMBERS.may_load(storage, addr)?;

    let weight = calc_weight(snapshot, now);

    let reward_per_weight = 1000;
    let reward = Uint128::new((weight * reward_per_weight) as u128);

    let withdrawal = WITHDRAWN.may_load(storage, addr)?.unwrap_or_default();
    Ok(reward.checked_sub(withdrawal)?)
}

#[inline]
fn coin_to_string(amount: Uint128, denom: &str) -> String {
    format!("{} {}", amount, denom)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Member {
            address,
            at_height: height,
        } => to_binary(&query_member(deps, address, height)?),
        QueryMsg::ListMembers { start_after, limit } => {
            to_binary(&list_members(deps, start_after, limit)?)
        }
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
    Ok(TotalResponse { total })
}

pub fn query_staked(deps: Deps, addr: String) -> StdResult<StakedResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let stake = STAKE.may_load(deps.storage, &addr)?.unwrap_or_default();
    let denom = CONFIG.load(deps.storage)?.staking_token;
    Ok(StakedResponse { stake, denom })
}

fn query_member(deps: Deps, addr: String, height: Option<u64>) -> StdResult<MemberResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let snapshot = match height {
        Some(h) => MEMBERS.may_load_at_height(deps.storage, &addr, h),
        None => MEMBERS.may_load(deps.storage, &addr),
    }?;
    Ok(MemberResponse { snapshot })
}

fn query_reward(deps: Deps, env: Env, addr: String) -> StdResult<RewardResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let reward = calc_reward(deps.storage, &addr, env.block.time)?;
    Ok(RewardResponse { reward })
}

fn query_withdrawn(deps: Deps, addr: String) -> StdResult<WithdrawnResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let withdrawn = WITHDRAWN.may_load(deps.storage, &addr)?.unwrap_or_default();
    Ok(WithdrawnResponse { withdrawn })
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

fn list_members(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<MemberListResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.map(|addr| Bound::exclusive(addr.as_ref()));

    let members: StdResult<Vec<_>> = MEMBERS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (key, snapshot) = item?;
            Ok(Member {
                address: String::from_utf8(key)?,
                snapshot,
            })
        })
        .collect();

    Ok(MemberListResponse { members: members? })
}

#[cfg(test)]
mod tests {}
