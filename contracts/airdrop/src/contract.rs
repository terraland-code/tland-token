use std::ops::Div;

use cosmwasm_std::{Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Order, QuerierWrapper, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg, WasmQuery};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw0::{maybe_addr, must_pay};
use cw2::{get_contract_version, set_contract_version};
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw_storage_plus::Bound;

use platform_registry::{AddressBaseInfoResponse, PlatformRegistryQueryMsg};
use staking::msg::MemberResponse as StakingMemberResponse;
use staking::msg::QueryMsg as StakingQueryMsg;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMissionSmartContracts, InstantiateMsg, MemberListResponse, MemberListResponseItem, MemberResponse, MemberResponseItem, MigrateMsg, Missions, QueryMsg, RegisterMemberItem};
use crate::state::{CONFIG, Config, FeeConfig, Member, MEMBERS, MissionSmartContracts, STATE, State};

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
        fee_config: msg.fee_config,
        mission_smart_contracts: mission_smart_contracts_from(&deps, msg.mission_smart_contracts)?,
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &State { num_of_members: 0 })?;

    Ok(Response::default())
}

fn mission_smart_contracts_from(deps: &DepsMut, m: Option<InstantiateMissionSmartContracts>) -> StdResult<MissionSmartContracts> {
    let res = match m {
        Some(m) => MissionSmartContracts {
            lp_staking: option_addr_validate(&deps, &m.lp_staking)?,
            tland_staking: option_addr_validate(&deps, &m.tland_staking)?,
            platform_registry: option_addr_validate(&deps, &m.platform_registry)?,
        },
        None => MissionSmartContracts {
            lp_staking: None,
            tland_staking: None,
            platform_registry: None,
        },
    };
    Ok(res)
}

fn option_addr_validate(deps: &DepsMut, value: &Option<String>) -> StdResult<Option<Addr>> {
    let v = match value {
        Some(str) => Some(deps.api.addr_validate(&str)?),
        None => None,
    };
    Ok(v)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::CannotMigrate {
            previous_contract: version.contract,
        });
    }
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
        ExecuteMsg::UpdateConfig { owner, fee_config, mission_smart_contracts } =>
            execute_update_config(deps, env, info, owner, fee_config, mission_smart_contracts),
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
    new_fee_config: Option<Vec<FeeConfig>>,
    new_mission_smart_contracts: Option<InstantiateMissionSmartContracts>,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    let api = deps.api;
    let new_mission_sc = mission_smart_contracts_from(&deps, new_mission_smart_contracts)?;

    CONFIG.update(deps.storage, |mut existing_config| -> StdResult<_> {
        // update new owner if set
        if let Some(addr) = new_owner {
            existing_config.owner = api.addr_validate(&addr)?;
        }
        if let Some(fee_config) = new_fee_config {
            existing_config.fee_config = fee_config;
        }
        // update new lp_staking address if set
        if new_mission_sc.lp_staking.is_some() {
            existing_config.mission_smart_contracts.lp_staking = new_mission_sc.lp_staking
        }
        // update new tland_staking address if set
        if new_mission_sc.tland_staking.is_some() {
            existing_config.mission_smart_contracts.tland_staking = new_mission_sc.tland_staking
        }
        // update new platform_registry address if set
        if new_mission_sc.platform_registry.is_some() {
            existing_config.mission_smart_contracts.platform_registry = new_mission_sc.platform_registry
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
    let mut new_members: u64 = 0;
    for m in members.iter() {
        let address = deps.api.addr_validate(&m.address)?;
        let val = Member {
            amount: m.amount,
            claimed: m.claimed,
        };
        if !MEMBERS.has(deps.storage, &address) {
            new_members += 1;
        }
        MEMBERS.save(deps.storage, &address, &val)?;
    }

    STATE.update(deps.storage, |mut existing_state| -> StdResult<_> {
        existing_state.num_of_members += new_members;
        Ok(existing_state)
    })?;

    Ok(Response::new()
        .add_attribute("action", "register_member")
        .add_attribute("sender", info.sender))
}

pub fn execute_claim(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    // sender has to pay 1 UST to claim
    must_pay_fee(&info, &cfg, "claim".to_string())?;

    let member = MEMBERS.may_load(deps.storage, &info.sender)?;

    let amount = match member {
        Some(mut member) => {
            // check missions passed by the sender
            let missions = check_missions(&deps.querier, &cfg, &info.sender)?;
            // calculate amount to claim based on passed missions
            let available_to_claim = calc_claim_amount(&missions, &member)?;
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

fn calc_claim_amount(missions: &Missions, member: &Member) -> StdResult<Uint128> {
    let passed_missions_num = calc_missions_passed(&missions);
    let max_passed_missions = Uint128::new(5);

    // amount earned equals amount multiplied by percentage of passed missions
    let amount_earned = member.amount
        .checked_mul(Uint128::from(passed_missions_num))
        .map_err(StdError::overflow)?
        .div(max_passed_missions);

    // claim amount is amount_earned minus already claimed
    Ok(amount_earned
        .checked_sub(member.claimed)
        .map_err(StdError::overflow)?)
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
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Member { address } => to_binary(&query_member(deps, address)?),
        QueryMsg::ListMembers { start_after, limit } =>
            to_binary(&query_member_list(deps, start_after, limit)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    Ok(CONFIG.load(deps.storage)?)
}

pub fn query_state(deps: Deps) -> StdResult<State> {
    Ok(STATE.load(deps.storage)?)
}

pub fn query_member(deps: Deps, addr: String) -> StdResult<MemberResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let cfg = CONFIG.load(deps.storage)?;
    let member = MEMBERS.may_load(deps.storage, &addr)?;

    let res: Option<MemberResponseItem> = match member {
        Some(m) => {
            let passed_missions = check_missions(&deps.querier, &cfg, &addr)?;
            let available_to_claim = calc_claim_amount(&passed_missions, &m)?;

            Some(MemberResponseItem {
                amount: m.amount,
                available_to_claim,
                claimed: m.claimed,
                passed_missions,
            })
        }
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
) -> StdResult<MemberListResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.map(|addr| Bound::exclusive(addr.as_ref()));

    let members: StdResult<Vec<_>> = MEMBERS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (key, m) = item?;

            let addr = deps.api.addr_validate(&String::from_utf8(key)?)?;
            let cfg = CONFIG.load(deps.storage)?;

            let passed_missions = check_missions(&deps.querier, &cfg, &addr)?;
            let available_to_claim = calc_claim_amount(&passed_missions, &m)?;

            Ok(MemberListResponseItem {
                address: addr.to_string(),
                info: MemberResponseItem {
                    amount: m.amount,
                    available_to_claim,
                    claimed: m.claimed,
                    passed_missions,
                },
            })
        })
        .collect();

    Ok(MemberListResponse { members: members? })
}


fn check_missions(querier: &QuerierWrapper, cfg: &Config, addr: &Addr) -> StdResult<Missions> {
    let mut missions = Missions {
        is_in_lp_staking: false,
        is_in_tland_staking: false,
        is_registered_on_platform: false,
        is_property_shareholder: false,
    };

    if let Some(contract_addr) = cfg.mission_smart_contracts.lp_staking.clone() {
        let query = WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&StakingQueryMsg::Member {
                address: addr.to_string(),
            })?,
        }.into();
        let res: StakingMemberResponse = querier.query(&query)?;
        if res.member.is_some() {
            missions.is_in_lp_staking = true;
        }
    }

    if let Some(contract_addr) = cfg.mission_smart_contracts.tland_staking.clone() {
        let query = WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&StakingQueryMsg::Member {
                address: addr.to_string(),
            })?,
        }.into();
        let res: StakingMemberResponse = querier.query(&query)?;
        if res.member.is_some() {
            missions.is_in_tland_staking = true;
        }
    }

    if let Some(contract_addr) = cfg.mission_smart_contracts.platform_registry.clone() {
        let query = WasmQuery::Smart {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&PlatformRegistryQueryMsg::AddressBaseInfo {
                address: addr.to_string(),
            })?,
        }.into();
        let res: AddressBaseInfoResponse = querier.query(&query)?;
        if res.is_registered {
            missions.is_registered_on_platform = true;
        }
        if res.is_property_shareholder {
            missions.is_property_shareholder = true;
        }
    }

    Ok(missions)
}

fn calc_missions_passed(missions: &Missions) -> u32 {
    // one mission is always passed
    let mut passed = 1;

    if missions.is_in_lp_staking {
        passed += 1;
    }
    if missions.is_in_tland_staking {
        passed += 1;
    }
    if missions.is_registered_on_platform {
        passed += 1;
    }
    if missions.is_property_shareholder {
        passed += 1;
    }

    return passed;
}

#[cfg(test)]
mod tests {}
