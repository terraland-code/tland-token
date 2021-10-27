use std::ops::{Div, Mul};

use cosmwasm_std::{Addr, BankMsg, Binary, Decimal, Deps, DepsMut, Env, from_slice, MessageInfo, Order, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg, WasmQuery};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw0::{Duration, maybe_addr, must_pay};
use cw20::{Balance, BalanceResponse, Cw20CoinVerified, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg};
use cw2::set_contract_version;
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MemberListResponse, MemberListResponseItem, MemberResponse, MemberResponseItem, NewConfig, QueryMsg, ReceiveMsg};
use crate::state::{CLAIMS, Config, CONFIG, MemberInfo, MEMBERS, Schedule, State, STATE};

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
        owner: deps.api.addr_validate(&msg.owner)?,
        staking_token: deps.api.addr_validate(&msg.staking_token)?,
        terraland_token: deps.api.addr_validate(&msg.terraland_token)?,
        unbonding_period: msg.unbonding_period,
        burn_address: deps.api.addr_validate(&msg.burn_address)?,
        instant_claim_percentage_loss: msg.instant_claim_percentage_loss,
        distribution_schedule: msg.distribution_schedule,
    };

    let state = State {
        total_stake: Default::default(),
        last_updated: 0,
        global_reward_index: Default::default(),
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;

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
        ExecuteMsg::UpdateConfig(new_config) => execute_update_config(deps, env, info, new_config),
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::Unbond { tokens: amount } => execute_unbond(deps, env, info, amount),
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
        ExecuteMsg::InstantClaim {} => execute_instant_claim(deps, env, info),
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),
        ExecuteMsg::UstWithdraw { recipient } =>
            execute_ust_withdraw(deps, env, info, recipient),
        ExecuteMsg::TokenWithdraw { token, recipient } =>
            execute_token_withdraw(deps, env, info, token, recipient),
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_config: NewConfig,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    let api = deps.api;

    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        if let Some(addr) = new_config.owner {
            exists.owner = api.addr_validate(&addr)?;
        }
        if let Some(addr) = new_config.burn_address {
            exists.burn_address = api.addr_validate(&addr)?;
        }
        if let Some(period) = new_config.unbonding_period {
            exists.unbonding_period = period;
        }
        if let Some(percentage) = new_config.instant_claim_percentage_loss {
            exists.instant_claim_percentage_loss = percentage;
        }
        if let Some(schedule) = new_config.distribution_schedule {
            exists.distribution_schedule = schedule;
        }
        Ok(exists)
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
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

    let mut state = STATE.load(deps.storage)?;
    let mut member_info = MEMBERS.may_load(deps.storage, &sender)?
        .unwrap_or(Default::default());

    // compute reward and updates member info with new rewards
    update_member_reward(&state, &cfg, env.block.time.seconds(), &mut member_info);

    // update member stake
    member_info.stake += amount;

    // update state with new stake and global_reward_index
    state.total_stake += amount;
    state.last_updated = env.block.time.seconds();
    state.global_reward_index = member_info.reward_index;

    // save new member info and state in storage
    MEMBERS.save(deps.storage, &sender, &member_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "bond")
        .add_attribute("amount", amount)
        .add_attribute("sender", sender))
}

fn update_member_reward(state: &State, cfg: &Config, time: u64, member_info: &mut MemberInfo) -> () {
    let global_reward_index = compute_reward_index(&cfg, &state, time);

    let reward = compute_member_reward(&member_info, global_reward_index);

    member_info.reward_index = global_reward_index;
    member_info.pending_reward = reward;
}

fn compute_reward_index(cfg: &Config, state: &State, time: u64) -> Decimal {
    // if there is first stake, the reward index is 0
    if state.last_updated == 0 {
        return Decimal::zero();
    }

    // if we are outside distribution schedule then panic
    let current_schedule = find_distribution_schedule(&cfg, time).unwrap();

    // compute distributed amount per second for current schedule
    let distributed_amount_per_sec = current_schedule.amount
        .div(Uint128::from(current_schedule.end_time - current_schedule.start_time));

    // distributed amount per second multiplied by time elapsed since last update
    let distributed_amount = distributed_amount_per_sec
        .mul(Uint128::from(time - state.last_updated));

    // global reward index is increased by distributed amount per staked token
    let res = state.global_reward_index
        + Decimal::from_ratio(distributed_amount, state.total_stake);

    return res;
}

fn find_distribution_schedule(cfg: &Config, time: u64) -> Option<Schedule> {
    for schedule in cfg.distribution_schedule.iter() {
        if time >= schedule.start_time && time < schedule.end_time {
            return Some(schedule.clone());
        }
    }
    None
}

fn compute_member_reward(member_info: &MemberInfo, global_reward_index: Decimal) -> Uint128 {
    let pending_reward = member_info.stake * global_reward_index
        - member_info.stake * member_info.reward_index;

    return member_info.pending_reward + pending_reward;
}

pub fn execute_unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // sender has to pay fee to unbond
    must_pay_fee(&info)?;

    // provide them a claim
    let cfg = CONFIG.load(deps.storage)?;
    CLAIMS.create_claim(
        deps.storage,
        &info.sender,
        amount,
        Duration::Time(cfg.unbonding_period).after(&env.block),
    )?;

    let mut state = STATE.load(deps.storage)?;
    let mut member_info = MEMBERS.may_load(deps.storage, &info.sender)?
        .unwrap_or(Default::default());

    // compute reward and updates member info with new rewards
    update_member_reward(&state, &cfg, env.block.time.seconds(), &mut member_info);

    // update member stake
    member_info.stake = member_info.stake.checked_sub(amount).map_err(StdError::overflow)?;

    // update state with new stake and global_reward_index
    state.total_stake -= amount;
    state.last_updated = env.block.time.seconds();
    state.global_reward_index = member_info.reward_index;

    // save new member info and state in storage
    MEMBERS.save(deps.storage, &info.sender, &member_info)?;
    STATE.save(deps.storage, &state)?;

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
    // sender has to pay fee to claim
    must_pay_fee(&info)?;

    // get amount of tokens to release
    let release = CLAIMS.claim_tokens(deps.storage, &info.sender, &env.block, None)?;
    if release.is_zero() {
        return Err(ContractError::NothingToClaim {});
    }

    // create message to transfer staking tokens
    let config = CONFIG.load(deps.storage)?;
    let message = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.staking_token.clone().into(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.clone().into(),
            amount: release,
        })?,
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
    // sender has to pay fee to instant_claim
    must_pay_fee(&info)?;

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
    let message1 = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.staking_token.clone().into(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.clone().into(),
            amount: release,
        })?,
        funds: vec![],
    });

    // create message to transfer fee to burn address
    let message2 = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.staking_token.clone().into(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: config.burn_address.clone().into(),
            amount: fee,
        })?,
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
    let state = STATE.load(deps.storage)?;
    let cfg = CONFIG.load(deps.storage)?;
    let mut member_info = MEMBERS.may_load(deps.storage, &info.sender)?
        .unwrap_or(Default::default());

    // calculate member reward until current block
    update_member_reward(&state, &cfg, env.block.time.seconds(), &mut member_info);

    // amount to withdraw is difference between the reward and the withdraw amount
    let amount = member_info.pending_reward.checked_sub(member_info.withdrawn)
        .map_err(StdError::overflow)?;

    if amount.is_zero() {
        return Err(ContractError::NothingToWithdraw {});
    }

    // update withdrawal
    MEMBERS.update(deps.storage, &info.sender, |member_info| -> StdResult<_> {
        let mut info = member_info.unwrap();
        info.withdrawn += amount;
        Ok(info)
    })?;

    // create message to transfer reward in terraland tokens
    let config = CONFIG.load(deps.storage)?;
    let message = SubMsg::new(WasmMsg::Execute {
        contract_addr: config.terraland_token.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.clone().into(),
            amount,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(message)
        .add_attribute("action", "withdraw")
        .add_attribute("tokens", coin_to_string(amount, config.terraland_token.as_str()))
        .add_attribute("sender", info.sender))
}

pub fn execute_ust_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    // get ust balance
    let ust_balance = deps.querier.query_balance(&env.contract.address, "uust")?;

    // create message to transfer ust
    let message = SubMsg::new(BankMsg::Send {
        to_address: String::from(deps.api.addr_validate(&recipient)?),
        amount: vec![ust_balance],
    });

    Ok(Response::new()
        .add_submessage(message)
        .add_attribute("action", "ust_withdraw")
        .add_attribute("sender", info.sender))
}

pub fn execute_token_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
    recipient: String,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    // get token balance for this contract
    let token_addr = deps.api.addr_validate(&token)?;
    let query = WasmQuery::Smart {
        contract_addr: token_addr.to_string(),
        msg: to_binary(&Cw20QueryMsg::Balance {
            address: env.contract.address.to_string(),
        })?,
    }.into();
    let res: BalanceResponse = deps.querier.query(&query)?;

    // create message to transfer tokens
    let message = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: String::from(deps.api.addr_validate(&recipient)?),
            amount: res.balance,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(message)
        .add_attribute("action", "token_withdraw")
        .add_attribute("sender", info.sender))
}

fn must_pay_fee(info: &MessageInfo) -> Result<(), ContractError> {
    // check if 1 UST was send
    let amount = must_pay(info, "uust")?;
    if amount != Uint128::new(1000000) {
        return Err(ContractError::InvalidFeeAmount {});
    }
    Ok(())
}

#[inline]
fn coin_to_string(amount: Uint128, denom: &str) -> String {
    format!("{} {}", amount, denom)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Member { address } => to_binary(&query_member(deps, env, address)?),
        QueryMsg::ListMembers { start_after, limit } =>
            to_binary(&query_member_list(deps, env, start_after, limit)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    Ok(CONFIG.load(deps.storage)?)
}

fn query_state(deps: Deps) -> StdResult<State> {
    Ok(STATE.load(deps.storage)?)
}

fn query_member(deps: Deps, env: Env, addr: String) -> StdResult<MemberResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let member_info = MEMBERS.may_load(deps.storage, &addr)?;

    if let Some(info) = member_info {
        let cfg = CONFIG.load(deps.storage)?;
        let state = STATE.load(deps.storage)?;
        let global_reward_index = compute_reward_index(&cfg, &state, env.block.time.seconds());
        let reward = compute_member_reward(&info, global_reward_index);

        return Ok(MemberResponse {
            member: Some(MemberResponseItem {
                stake: info.stake,
                reward,
                reward_index: info.reward_index,
                withdrawn: info.withdrawn,
                claims: CLAIMS.query_claims(deps, &addr)?.claims,
            }),
        });
    }

    Ok(MemberResponse { member: None })
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

fn query_member_list(
    deps: Deps,
    env: Env,
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
            let (key, info) = item?;
            let address = deps.api.addr_validate(&String::from_utf8(key)?)?;

            let cfg = CONFIG.load(deps.storage)?;
            let state = STATE.load(deps.storage)?;
            let global_reward_index = compute_reward_index(&cfg, &state, env.block.time.seconds());
            let reward = compute_member_reward(&info, global_reward_index);

            Ok(MemberListResponseItem {
                address: address.to_string(),
                info: MemberResponseItem {
                    stake: info.stake,
                    reward,
                    reward_index: info.reward_index,
                    withdrawn: info.withdrawn,
                    claims: CLAIMS.query_claims(deps, &address)?.claims,
                },
            })
        })
        .collect();

    Ok(MemberListResponse { members: members? })
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::from_slice;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    use super::*;

    const INIT_ADMIN: &str = "admin";
    const USER1: &str = "somebody";
    const USER2: &str = "else";
    const USER3: &str = "funny";
    const UNBONDING_PERIOD: u64 = 600;
    const BURN_ADDRESS: &str = "burn1234567890";
    const TERRALAND_TOKEN_ADDRESS: &str = "tland1234567890";
    const STAKING_TOKEN_ADDRESS: &str = "staking1234567890";
    const WEEK: u64 = 604800;

    fn default_instantiate(
        deps: DepsMut,
    ) {
        let msg = InstantiateMsg {
            owner: INIT_ADMIN.into(),
            staking_token: STAKING_TOKEN_ADDRESS.into(),
            terraland_token: TERRALAND_TOKEN_ADDRESS.into(),
            unbonding_period: UNBONDING_PERIOD,
            burn_address: BURN_ADDRESS.into(),
            instant_claim_percentage_loss: 0,
            distribution_schedule: Vec::from([
                Schedule {
                    amount: Uint128::new(150_000_000_000),
                    start_time: mock_env().block.time.seconds(),
                    end_time: mock_env().block.time.seconds() + WEEK,
                }]),
        };
        let info = mock_info("creator", &[]);
        instantiate(deps, mock_env(), info, msg).unwrap();
    }

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        default_instantiate(deps.as_mut());

        // it worked, let's query the state
        let res = query_config(deps.as_ref()).unwrap();
        assert_eq!(INIT_ADMIN, res.owner.as_str());

        let res = query_state(deps.as_ref()).unwrap();
        assert_eq!(0, res.total_stake.u128());

        let res = query_member(deps.as_ref(), env, USER1.into()).unwrap();
        assert_eq!(None, res.member)
    }

    fn get_env(height_delta: u64) -> Env {
        let mut env = mock_env();
        env.block.height += height_delta;
        env.block.time = env.block.time.plus_seconds(height_delta * 6);
        return env;
    }

    fn get_member(deps: Deps, addr: String) -> Option<MemberResponseItem> {
        let raw = query(deps, mock_env(), QueryMsg::Member { address: addr }).unwrap();
        let res: MemberResponse = from_slice(&raw).unwrap();
        return res.member;
    }

    // this tests the member queries
    fn assert_users(
        deps: Deps,
        user1: Option<MemberResponseItem>,
        user2: Option<MemberResponseItem>,
        user3: Option<MemberResponseItem>,
    ) {
        let member1 = get_member(deps, USER1.into());
        assert_eq!(member1, user1);

        let member2 = get_member(deps, USER2.into());
        assert_eq!(member2, user2);

        let member3 = get_member(deps, USER3.into());
        assert_eq!(member3, user3);
    }

    fn bond_cw20(mut deps: DepsMut, user1: u128, user2: u128, user3: u128, height_delta: u64) {
        let env = get_env(height_delta);

        for (addr, stake) in &[(USER1, user1), (USER2, user2), (USER3, user3)] {
            if *stake != 0 {
                let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
                    sender: addr.to_string(),
                    amount: Uint128::new(*stake),
                    msg: to_binary(&ReceiveMsg::Bond {}).unwrap(),
                });
                let info = mock_info(STAKING_TOKEN_ADDRESS, &[]);
                execute(deps.branch(), env.clone(), info, msg).unwrap();
            }
        }
    }

    // this tests the member queries
    fn assert_stake(deps: Deps, user1_stake: u128, user2_stake: u128, user3_stake: u128, height_delta: u64) {
        let env = get_env(height_delta);

        let res1 = query_member(deps, env.clone(), USER1.into()).unwrap();
        assert_eq!(res1.member.unwrap().stake, user1_stake.into());

        let res2 = query_member(deps, env.clone(), USER2.into()).unwrap();
        assert_eq!(res2.member.unwrap().stake, user2_stake.into());

        let res3 = query_member(deps, env.clone(), USER3.into()).unwrap();
        assert_eq!(res3.member.unwrap().stake, user3_stake.into());
    }

    fn assert_rewards(deps: Deps, user1_reward: u128, user2_reward: u128, user3_reward: u128, height_delta: u64) {
        let env = get_env(height_delta);

        let res1 = query_member(deps, env.clone(), USER1.into()).unwrap();
        assert_eq!(res1.member.unwrap().reward, user1_reward.into());

        let res2 = query_member(deps, env.clone(), USER2.into()).unwrap();
        assert_eq!(res2.member.unwrap().reward, user2_reward.into());

        let res3 = query_member(deps, env.clone(), USER3.into()).unwrap();
        assert_eq!(res3.member.unwrap().reward, user3_reward.into());
    }

    #[test]
    fn cw20_token_bond() {
        let mut deps = mock_dependencies(&[]);
        default_instantiate(deps.as_mut());

        // Assert original staking members
        assert_users(deps.as_ref(), None, None, None);

        bond_cw20(deps.as_mut(), 12_000, 7_500, 500, 1);

        assert_stake(deps.as_ref(), 12_000, 7_500, 500, 1);

        // unbond_cw20(deps.as_mut(), 6_000, 1_500, 0, 2)
    }
}
