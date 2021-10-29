use cosmwasm_std::{BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult, SubMsg, to_binary, Uint128, WasmMsg, WasmQuery};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw0::{maybe_addr, must_pay};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw2::set_contract_version;
use cw_storage_plus::Bound;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MemberListResponse, MemberListResponseItem, MemberResponse, MemberResponseItem, QueryMsg, RegisterMemberItem};
use crate::state::{CONFIG, Config, FeeConfig, Member, MEMBERS, Vesting};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:airdrop";
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
        terraland_token: deps.api.addr_validate(&msg.terraland_token)?,
        name: msg.name,
        fee_config: msg.fee_config,
        vesting: msg.vesting,
    };

    CONFIG.save(deps.storage, &config)?;

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
        ExecuteMsg::UpdateConfig { owner, name, fee_config, vesting } =>
            execute_update_config(deps, env, info, owner, name, fee_config, vesting),
        ExecuteMsg::RegisterMembers(members) =>
            execute_register_members(deps, env, info, members),
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
        ExecuteMsg::UstWithdraw { recipient, amount } =>
            execute_ust_withdraw(deps, env, info, recipient, amount),
        ExecuteMsg::TokenWithdraw { token, recipient } =>
            execute_token_withdraw(deps, env, info, token, recipient),
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: Option<String>,
    new_name: Option<String>,
    new_fee_config: Option<Vec<FeeConfig>>,
    new_vesting: Option<Vesting>,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    let api = deps.api;

    CONFIG.update(deps.storage, |mut existing_config| -> StdResult<_> {
        // update new owner if set
        if let Some(addr) = new_owner {
            existing_config.owner = api.addr_validate(&addr)?;
        }
        if let Some(name) = new_name {
            existing_config.name = name;
        }
        if let Some(fee_config) = new_fee_config {
            existing_config.fee_config = fee_config;
        }
        if let Some(vesting) = new_vesting {
            existing_config.vesting = vesting;
        }
        Ok(existing_config)
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub fn execute_register_members(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    members: Vec<RegisterMemberItem>,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    // save all members with valid address in storage
    for m in members.iter() {
        let address = deps.api.addr_validate(&m.address)?;
        let val = Member {
            amount: m.amount,
            claimed: m.claimed.unwrap_or_default(),
        };
        MEMBERS.save(deps.storage, &address, &val)?;
    }

    Ok(Response::new()
        .add_attribute("action", "register_member")
        .add_attribute("sender", info.sender))
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // sender has to pay fee to claim
    must_pay_fee(&info, &cfg, "claim".to_string())?;

    let member = MEMBERS.may_load(deps.storage, &info.sender)?;

    let amount = match member {
        Some(mut member) => {
            // compute amount available to claim
            let available_to_claim = compute_available_amount(&member, &cfg, env.block.time.seconds());
            // update member claimed amount
            member.claimed += available_to_claim;
            MEMBERS.save(deps.storage, &info.sender, &member)?;
            Ok(available_to_claim)
        }
        None => Err(ContractError::MemberNotFound {})
    }?;

    if amount.is_zero() {
        return Err(ContractError::NothingToClaim {});
    }

    // create message to transfer terraland tokens
    let message = SubMsg::new(WasmMsg::Execute {
        contract_addr: cfg.terraland_token.clone().into(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.clone().into(),
            amount,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_submessage(message)
        .add_attribute("action", "claim")
        .add_attribute("tokens", format!("{} {}", amount, cfg.terraland_token.as_str()))
        .add_attribute("sender", info.sender))
}

fn compute_available_amount(member: &Member, cfg: &Config, time: u64) -> Uint128 {
    // calculate released amount for the member
    let released_amount = compute_released_amount(&member, &cfg, time);
    // available amount to claim is decreased by already claimed tokens
    return released_amount - member.claimed;
}

fn compute_released_amount(member: &Member, cfg: &Config, time: u64) -> Uint128 {
    // before vesting start released amount is 0
    if time < cfg.vesting.start_time {
        return Uint128::zero();
    }

    // after vesting end released full amount
    if time > cfg.vesting.end_time {
        return member.amount;
    }

    // initial amount is released at the beginning of vesting
    let initial_amount = member.amount * Uint128::from(cfg.vesting.initial_percentage) / Uint128::new(100);

    // during the cliff the initial_amount is released
    if time < cfg.vesting.cliff_end_time {
        return initial_amount;
    }

    const DAY: u64 = 24 * 3600;
    let total_days = (cfg.vesting.end_time - cfg.vesting.cliff_end_time) / DAY;
    let days_passed = (time - cfg.vesting.cliff_end_time) / DAY;

    // after cliff ends smart contract release initial_amount + rest daily
    return (member.amount - initial_amount) * Uint128::from(days_passed) / Uint128::from(total_days) + initial_amount;
}

pub fn execute_ust_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    // create message to transfer ust
    let message = SubMsg::new(BankMsg::Send {
        to_address: String::from(deps.api.addr_validate(&recipient)?),
        amount: vec![Coin { denom: "uusd".to_string(), amount }],
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

fn must_pay_fee(info: &MessageInfo, cfg: &Config, operation: String) -> Result<(), ContractError> {
    let mut denom = "".to_string();
    let mut fee_amount = Uint128::zero();

    for fee_config in cfg.fee_config.iter() {
        if fee_config.operation == operation {
            fee_amount = fee_config.fee;
            denom = fee_config.denom.clone();
        }
    }

    if fee_amount == Uint128::zero() {
        return Ok(());
    }

    // check if exact fee amount was send
    let amount = must_pay(info, denom.as_str())?;
    if amount != fee_amount {
        return Err(ContractError::InvalidFeeAmount {});
    }

    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Member { address } =>
            to_binary(&query_member(deps, address, env.block.time.seconds())?),
        QueryMsg::ListMembers { start_after, limit } =>
            to_binary(&query_member_list(deps, start_after, limit, env.block.time.seconds())?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    Ok(CONFIG.load(deps.storage)?)
}

pub fn query_member(deps: Deps, addr: String, time: u64) -> StdResult<MemberResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let cfg = CONFIG.load(deps.storage)?;
    let member = MEMBERS.may_load(deps.storage, &addr)?;

    let res: Option<MemberResponseItem> = match member {
        Some(m) => Some(MemberResponseItem {
            amount: m.amount,
            available_to_claim: compute_available_amount(&m, &cfg, time),
            claimed: m.claimed,
        }),
        None => None,
    };

    Ok(MemberResponse { member: res })
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

fn query_member_list(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
    time: u64,
) -> StdResult<MemberListResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.map(|addr| Bound::exclusive(addr.as_ref()));
    let cfg = CONFIG.load(deps.storage)?;

    let members: StdResult<Vec<_>> = MEMBERS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (key, m) = item?;

            let addr = deps.api.addr_validate(&String::from_utf8(key)?)?;

            Ok(MemberListResponseItem {
                address: addr.to_string(),
                info: MemberResponseItem {
                    amount: m.amount,
                    available_to_claim: compute_available_amount(&m, &cfg, time),
                    claimed: m.claimed,
                },
            })
        })
        .collect();

    Ok(MemberListResponse { members: members? })
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{Coin, Deps, DepsMut, Env, Uint128};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    use crate::contract::{execute, instantiate, query_config, query_member};
    use crate::msg::{ExecuteMsg, InstantiateMsg, MemberResponseItem, RegisterMemberItem};
    use crate::state::{FeeConfig, Vesting};

    const INIT_ADMIN: &str = "admin";
    const USER1: &str = "somebody";
    const USER2: &str = "else";
    const TERRALAND_TOKEN_ADDRESS: &str = "tland1234567890";
    const NAME: &str = "VESTING";
    const WEEK: u64 = 604800;
    const FEE: Uint128 = Uint128::new(1000000);
    const FEE_DENOM: &str = "uusd";

    fn default_instantiate(
        deps: DepsMut,
        env: Env,
    ) {
        let msg = InstantiateMsg {
            owner: INIT_ADMIN.into(),
            terraland_token: TERRALAND_TOKEN_ADDRESS.into(),
            name: "VESTING".to_string(),
            fee_config: Vec::from([FeeConfig{
                fee: FEE,
                operation: "claim".to_string(),
                denom: FEE_DENOM.to_string(),
            }]),
            vesting: Vesting {
                start_time: env.block.time.seconds(),
                end_time: env.block.time.seconds() + 10 * WEEK,
                initial_percentage: 10,
                cliff_end_time: env.block.time.seconds() + WEEK,
            },
        };
        let info = mock_info("creator", &[]);
        instantiate(deps, env, info, msg).unwrap();
    }

    #[test]
    fn proper_instantiation() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        default_instantiate(deps.as_mut(), mock_env());

        // it worked, let's query the state
        let res = query_config(deps.as_ref()).unwrap();
        assert_eq!(INIT_ADMIN, res.owner.as_str());
        assert_eq!(NAME, res.name.as_str());

        let res = query_member(deps.as_ref(), USER1.into(), env.block.time.seconds()).unwrap();
        assert_eq!(None, res.member)
    }

    fn get_env(height_delta: u64) -> Env {
        let mut env = mock_env();
        env.block.height += height_delta;
        env.block.time = env.block.time.plus_seconds(height_delta * 6);
        return env;
    }

    fn assert_users(
        deps: Deps,
        env: Env,
        user1: Option<MemberResponseItem>,
        user2: Option<MemberResponseItem>,
    ) {
        let member1 = query_member(deps, USER1.into(), env.block.time.seconds()).unwrap();
        assert_eq!(member1.member, user1);

        let member2 = query_member(deps, USER2.into(), env.block.time.seconds()).unwrap();
        assert_eq!(member2.member, user2);
    }

    fn register_members(mut deps: DepsMut, user1: u128, user2: u128) {
        let env = mock_env();

        for (addr, amount) in &[(USER1, user1), (USER2, user2)] {
            if *amount != 0 {
                let msg = ExecuteMsg::RegisterMembers(Vec::from([
                    RegisterMemberItem {
                        address: addr.to_string(),
                        amount: Uint128::new(*amount),
                        claimed: None,
                    }]));
                let info = mock_info(INIT_ADMIN, &[]);
                execute(deps.branch(), env.clone(), info, msg).unwrap();
            }
        }
    }

    fn assert_available_to_claim(deps: Deps, user1_stake: u128, user2_stake: u128, height_delta: u64) {
        let env = get_env(height_delta);

        let res1 = query_member(deps,  USER1.into(), env.block.time.seconds()).unwrap();
        assert_eq!(res1.member.unwrap().available_to_claim, user1_stake.into());

        let res2 = query_member(deps,  USER2.into(), env.block.time.seconds()).unwrap();
        assert_eq!(res2.member.unwrap().available_to_claim, user2_stake.into());
    }

    fn assert_claimed(deps: Deps, user1_stake: u128, user2_stake: u128, height_delta: u64) {
        let env = get_env(height_delta);

        let res1 = query_member(deps,  USER1.into(), env.block.time.seconds()).unwrap();
        assert_eq!(res1.member.unwrap().claimed, user1_stake.into());

        let res2 = query_member(deps,  USER2.into(), env.block.time.seconds()).unwrap();
        assert_eq!(res2.member.unwrap().claimed, user2_stake.into());
    }

    fn assert_amount(deps: Deps, user1_stake: u128, user2_stake: u128, height_delta: u64) {
        let env = get_env(height_delta);

        let res1 = query_member(deps,  USER1.into(), env.block.time.seconds()).unwrap();
        assert_eq!(res1.member.unwrap().amount, user1_stake.into());

        let res2 = query_member(deps,  USER2.into(), env.block.time.seconds()).unwrap();
        assert_eq!(res2.member.unwrap().amount, user2_stake.into());
    }

    #[test]
    fn register_members_and_claim() {
        let mut deps = mock_dependencies(&[]);
        default_instantiate(deps.as_mut(), mock_env());

        // Assert original staking members
        assert_users(deps.as_ref(), mock_env(), None, None);

        // Register 2 members with amounts for vesting
        register_members(deps.as_mut(), 1_000_000, 5_000_000);

        assert_amount(deps.as_ref(), 1_000_000, 5_000_000, 0);
        assert_claimed(deps.as_ref(), 0, 0, 0);
        assert_available_to_claim(deps.as_ref(), 100_000, 500_000, 0);

        // available_to_claim keeps const until clif ends
        assert_available_to_claim(deps.as_ref(), 100_000, 500_000, 100800);
        // the daily increase happens
        assert_available_to_claim(deps.as_ref(), 114_285, 571_428, 115200);
        assert_available_to_claim(deps.as_ref(), 114_285, 571_428, 115200);
        assert_available_to_claim(deps.as_ref(), 128_571, 642_857, 129600);
        // ... until end of vesting
        assert_available_to_claim(deps.as_ref(), 1_000_000, 5_000_000, 1008000);
        assert_available_to_claim(deps.as_ref(), 1_000_000, 5_000_000, 10080000);

        // claim
        let env = get_env(100800);
        let msg = ExecuteMsg::Claim{};
        let info = mock_info(USER1, &[Coin{ denom: FEE_DENOM.to_string(), amount: FEE}]);
        execute(deps.as_mut().branch(), env.clone(), info, msg).unwrap();

        // check available_to_claim and claimed
        assert_available_to_claim(deps.as_ref(), 0, 500_000, 100800);
        assert_claimed(deps.as_ref(), 100_000, 0, 100800);
    }
}
