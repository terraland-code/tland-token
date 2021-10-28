use std::ops::Div;

use cosmwasm_std::{Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, QuerierWrapper, Response, StdError, StdResult, SubMsg, to_binary, Uint128, WasmMsg, WasmQuery};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw0::must_pay;
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};
use cw2::set_contract_version;

use platform_registry::{PlatformRegistryQueryMsg, AddressBaseInfoResponse};
use staking::msg::MemberResponse as StakingMemberResponse;
use staking::msg::QueryMsg as StakingQueryMsg;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMissionSmartContracts, InstantiateMsg, MemberResponse, MemberStats, Missions, NewMember, QueryMsg};
use crate::state::{CONFIG, Config, Member, MEMBERS, MissionSmartContracts};

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
        mission_smart_contracts: mission_smart_contracts_from(&deps, msg.mission_smart_contracts)?,
    };

    CONFIG.save(deps.storage, &config)?;

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
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { new_owner, mission_smart_contracts } =>
            execute_update_config(deps, env, info, new_owner, mission_smart_contracts),
        ExecuteMsg::RegisterMembers { members } =>
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
    new_mission_smart_contracts: Option<InstantiateMissionSmartContracts>,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    let owner = option_addr_validate(&deps, &new_owner)?;
    let mission_sc = mission_smart_contracts_from(&deps, new_mission_smart_contracts)?;

    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        // update new owner if set
        if let Some(addr) = owner {
            exists.owner = addr
        }
        // update new lp_staking address if set
        if mission_sc.lp_staking.is_some() {
            exists.mission_smart_contracts.lp_staking = mission_sc.lp_staking
        }
        // update new tland_staking address if set
        if mission_sc.tland_staking.is_some() {
            exists.mission_smart_contracts.tland_staking = mission_sc.tland_staking
        }
        // update new platform_registry address if set
        if mission_sc.platform_registry.is_some() {
            exists.mission_smart_contracts.platform_registry = mission_sc.platform_registry
        }

        Ok(exists)
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub fn execute_register_members(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    members: Vec<NewMember>,
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
            claimed: m.claimed,
        };
        MEMBERS.save(deps.storage, &address, &val)?;
    }

    Ok(Response::new()
        .add_attribute("action", "register_member")
        .add_attribute("sender", info.sender))
}

pub fn execute_claim(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // sender has to pay 1 UST to claim
    must_pay_fee(&info)?;

    let cfg = CONFIG.load(deps.storage)?;
    let member = MEMBERS.may_load(deps.storage, &info.sender)?;

    let amount = match member {
        Some(m) => {
            // check missions passed by the sender
            let missions = check_missions(&deps.querier, &cfg, &info.sender)?;
            Ok(calc_claim_amount( &missions, &m)?)
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

fn calc_claim_amount(missions : &Missions, member: &Member) -> StdResult<Uint128> {
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
        amount: vec![Coin{ denom: "uusd".to_string(), amount }],
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
    let amount = must_pay(info, "uusd")?;
    if amount != Uint128::new(1000000) {
        return Err(ContractError::InvalidFeeAmount {});
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Member { address } => to_binary(&query_member(deps, address)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    Ok( CONFIG.load(deps.storage)?)
}

pub fn query_member(deps: Deps, addr: String) -> StdResult<MemberResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    let addr = deps.api.addr_validate(&addr)?;
    let member = MEMBERS.may_load(deps.storage, &addr)?;
    let stats = match member {
        Some(m) => Some(MemberStats {
            amount: m.amount,
            claimed: m.claimed,
            passed_missions: check_missions(&deps.querier, &cfg, &addr)?,
        }),
        None => None,
    };
    Ok(MemberResponse { stats })
}

fn check_missions(querier: &QuerierWrapper, cfg: &Config, addr: &Addr) -> StdResult<Missions> {
    let mut missions = Missions {
        is_in_lp_staking: false,
        is_in_tland_staking: false,
        is_registered_on_platform: false,
        is_property_shareholder: false
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
            msg: to_binary(&PlatformRegistryQueryMsg::AddressBaseInfo{
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
