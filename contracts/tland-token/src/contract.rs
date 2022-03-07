use std::ops::{Div, Mul, Sub};

use cosmwasm_std::{Addr, BankMsg, Binary, coin, Decimal, Deps, DepsMut, Env, from_binary, MessageInfo, Response, StdError, StdResult, Storage, SubMsg, to_binary, Uint128};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::{get_contract_version, set_contract_version};
use cw20::{EmbeddedLogo, Logo, LogoInfo, MarketingInfoResponse};
use cw20_base::allowances::{
    execute_burn_from as cw20_execute_burn_from, execute_decrease_allowance as cw20_execute_decrease_allowance,
    execute_increase_allowance as cw20_execute_increase_allowance, execute_send_from as cw20_execute_send_from,
    execute_transfer_from as cw20_execute_transfer_from, query_allowance,
};
use cw20_base::contract::{
    create_accounts, execute_burn as cw20_execute_burn,
    execute_send as cw20_execute_send, execute_transfer as cw20_execute_transfer,
    execute_update_marketing as cw20_execute_update_marketing,
    execute_upload_logo as cw20_execute_upload_logo, query_balance,
    query_download_logo, query_marketing_info, query_token_info,
};
use cw20_base::ContractError;
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};
use cw20_base::state::{BALANCES, LOGO, MARKETING_INFO, TOKEN_INFO, TokenInfo};
use terraswap::pair::Cw20HookMsg;

use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SwapFeeConfigResponse};
use crate::querier::deduct_tax;
use crate::state::{CONFIG, Config, SWAP_FEE_CONFIG, SwapFeeConfig};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:tland-token";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const LOGO_SIZE_CAP: usize = 5 * 1024;

/// Checks if data starts with XML preamble
fn verify_xml_preamble(data: &[u8]) -> Result<(), ContractError> {
    // The easiest way to perform this check would be just match on regex, however regex
    // compilation is heavy and probably not worth it.

    let preamble = data
        .split_inclusive(|c| *c == b'>')
        .next()
        .ok_or(ContractError::InvalidXmlPreamble {})?;

    const PREFIX: &[u8] = b"<?xml ";
    const POSTFIX: &[u8] = b"?>";

    if !(preamble.starts_with(PREFIX) && preamble.ends_with(POSTFIX)) {
        Err(ContractError::InvalidXmlPreamble {})
    } else {
        Ok(())
    }

    // Additionally attributes format could be validated as they are well defined, as well as
    // comments presence inside of preable, but it is probably not worth it.
}

/// Validates XML logo
fn verify_xml_logo(logo: &[u8]) -> Result<(), ContractError> {
    verify_xml_preamble(logo)?;

    if logo.len() > LOGO_SIZE_CAP {
        Err(ContractError::LogoTooBig {})
    } else {
        Ok(())
    }
}

/// Validates png logo
fn verify_png_logo(logo: &[u8]) -> Result<(), ContractError> {
    // PNG header format:
    // 0x89 - magic byte, out of ASCII table to fail on 7-bit systems
    // "PNG" ascii representation
    // [0x0d, 0x0a] - dos style line ending
    // 0x1a - dos control character, stop displaying rest of the file
    // 0x0a - unix style line ending
    const HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if logo.len() > LOGO_SIZE_CAP {
        Err(ContractError::LogoTooBig {})
    } else if !logo.starts_with(&HEADER) {
        Err(ContractError::InvalidPngHeader {})
    } else {
        Ok(())
    }
}

/// Checks if passed logo is correct, and if not, returns an error
fn verify_logo(logo: &Logo) -> Result<(), ContractError> {
    match logo {
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => verify_xml_logo(logo),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => verify_png_logo(logo),
        Logo::Url(_) => Ok(()), // Any reasonable url validation would be regex based, probably not worth it
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // check valid token info
    msg.validate()?;
    // create initial accounts
    let total_supply = create_accounts(&mut deps, &msg.initial_balances)?;

    // store token info
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply,
        mint: None,
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    let cfg = Config { owner: deps.api.addr_validate(&msg.owner)? };
    CONFIG.save(deps.storage, &cfg)?;

    if let Some(marketing) = msg.marketing {
        let logo = if let Some(logo) = marketing.logo {
            verify_logo(&logo)?;
            LOGO.save(deps.storage, &logo)?;

            match logo {
                Logo::Url(url) => Some(LogoInfo::Url(url)),
                Logo::Embedded(_) => Some(LogoInfo::Embedded),
            }
        } else {
            None
        };

        let data = MarketingInfoResponse {
            project: marketing.project,
            description: marketing.description,
            marketing: marketing
                .marketing
                .map(|addr| deps.api.addr_validate(&addr))
                .transpose()?,
            logo,
        };
        MARKETING_INFO.save(deps.storage, &data)?;
    }

    if let Some(swap_fee_config) = msg.swap_fee_config {
        let data = SwapFeeConfig {
            fee_admin: deps.api.addr_validate(&swap_fee_config.fee_admin)?,
            enable_swap_fee: swap_fee_config.enable_swap_fee,
            swap_percent_fee: swap_fee_config.swap_percent_fee,
            fee_receiver: deps.api.addr_validate(&swap_fee_config.fee_receiver)?,
        };
        SWAP_FEE_CONFIG.save(deps.storage, &data)?;
    }

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(StdError::generic_err(
            format!("previous_contract: {}", version.contract)
        ).into());
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
        ExecuteMsg::UpdateConfig { owner } =>
            execute_update_config(deps, env, info, owner),
        ExecuteMsg::Transfer { recipient, amount } => {
            cw20_execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => cw20_execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => cw20_execute_increase_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => cw20_execute_decrease_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => cw20_execute_transfer_from(deps, env, info, owner, recipient, amount),
        ExecuteMsg::BurnFrom { owner, amount } => cw20_execute_burn_from(deps, env, info, owner, amount),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => execute_send_from(deps, env, info, owner, contract, amount, msg),
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => cw20_execute_update_marketing(deps, env, info, project, description, marketing),
        ExecuteMsg::UploadLogo(logo) => cw20_execute_upload_logo(deps, env, info, logo),
        ExecuteMsg::WithdrawLockedFunds {
            denom,
            amount,
            recipient
        } => execute_withdraw_locked_funds(deps, info, denom, amount, recipient),
        ExecuteMsg::UpdateSwapFeeConfig {
            fee_admin,
            enable_swap_fee,
            swap_percent_fee,
            fee_receiver,
        } => update_swap_fee_config(deps, info, fee_admin, enable_swap_fee, swap_percent_fee, fee_receiver)
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: Option<String>,
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
        Ok(existing_config)
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub fn execute_withdraw_locked_funds(
    deps: DepsMut,
    info: MessageInfo,
    denom: String,
    amount: Uint128,
    recipient: String,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    Ok(Response::new()
        .add_attribute("method", "withdraw_locked_funds")
        .add_attribute("sender", info.sender)
        .add_attribute("denom", denom.clone())
        .add_attribute("amount", amount.to_string())
        .add_attribute("recipient", recipient.clone())
        .add_submessage(SubMsg::new(BankMsg::Send {
            to_address: recipient,
            amount: vec![deduct_tax(deps, coin(amount.u128(), denom))?],
        })))
}

pub fn execute_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let fee_config = SWAP_FEE_CONFIG.may_load(deps.storage)?;

    if let Some(fee_config) = fee_config {
        // Calculate fee amount based on message type
        let fee_amount = calculate_fee_amount(amount, &msg, &fee_config);

        // If the fee is non zero then transfer the fee amount to the fee recipient address and execute cw20 send for left amount
        if !fee_amount.is_zero() {
            // Transfer fee to configured receiver address
            transfer(deps.storage, &info.sender, &fee_config.fee_receiver, fee_amount)?;

            let send_amount = amount.sub(fee_amount);
            let res = cw20_execute_send(deps, env, info.clone(), contract.clone(), send_amount, msg)?;

            return Ok(Response::new()
                .add_attribute("action", "send")
                .add_attribute("from", &info.sender)
                .add_attribute("to", &contract)
                .add_attribute("amount", amount)
                .add_attribute("fee_amount", fee_amount.to_string())
                .add_submessages(res.messages));
        }
    }

    cw20_execute_send(deps, env, info, contract, amount, msg)
}

pub fn execute_send_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let fee_config = SWAP_FEE_CONFIG.may_load(deps.storage)?;

    if let Some(fee_config) = fee_config {
        // Calculate fee amount based on message type
        let fee_amount = calculate_fee_amount(amount, &msg, &fee_config);

        // If the fee is non zero then transfer the fee amount to the fee recipient address and execute cw20 send for left amount
        if !fee_amount.is_zero() {
            // Transfer fee to configured receiver address
            let owner_addr = deps.api.addr_validate(&owner)?;
            transfer(deps.storage, &owner_addr, &fee_config.fee_receiver, fee_amount)?;

            let send_amount = amount.sub(fee_amount);
            let res = cw20_execute_send_from(deps, env, info.clone(), owner.clone(), contract.clone(), send_amount, msg)?;

            return Ok(Response::new()
                .add_attribute("action", "send_from")
                .add_attribute("from", &owner)
                .add_attribute("to", &contract)
                .add_attribute("by", &info.sender)
                .add_attribute("amount", amount)
                .add_attribute("fee_amount", fee_amount.to_string())
                .add_submessages(res.messages));
        }
    }

    cw20_execute_send_from(deps, env, info, owner, contract, amount, msg)
}

pub fn update_swap_fee_config(
    deps: DepsMut,
    info: MessageInfo,
    fee_admin: Option<String>,
    enable_swap_fee: Option<bool>,
    swap_percent_fee: Option<Decimal>,
    fee_receiver: Option<String>,
) -> Result<Response, ContractError> {
    let mut swap_fee_config = SWAP_FEE_CONFIG
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    if swap_fee_config.fee_admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(fee_admin) = fee_admin {
        swap_fee_config.fee_admin = deps.api.addr_validate(&fee_admin)?;
    }

    if let Some(enable_swap_fee) = enable_swap_fee {
        swap_fee_config.enable_swap_fee = enable_swap_fee;
    }

    if let Some(swap_percent_fee) = swap_percent_fee {
        swap_fee_config.swap_percent_fee = swap_percent_fee
    }

    if let Some(fee_receiver) = fee_receiver {
        swap_fee_config.fee_receiver = deps.api.addr_validate(&fee_receiver)?;
    }

    SWAP_FEE_CONFIG.save(deps.storage, &swap_fee_config)?;

    Ok(Response::new()
        .add_attribute("method", "update_swap_fee_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
        QueryMsg::SwapFeeConfig {} => {
            to_binary(&query_swap_fee_config(deps)?)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

pub fn query_swap_fee_config(deps: Deps) -> StdResult<SwapFeeConfigResponse> {
    let swap_fee_config = SWAP_FEE_CONFIG.may_load(deps.storage)?;
    match swap_fee_config {
        Some(swap_fee_config) => {
            Ok(SwapFeeConfigResponse {
                fee_admin: swap_fee_config.fee_admin.to_string(),
                enable_swap_fee: swap_fee_config.enable_swap_fee,
                swap_percent_fee: swap_fee_config.swap_percent_fee,
                fee_receiver: swap_fee_config.fee_receiver.to_string(),
            })
        }
        None => Ok(Default::default())
    }
}

fn calculate_fee_amount(amount: Uint128, msg: &Binary, swap_fee_config: &SwapFeeConfig) -> Uint128 {
    if swap_fee_config.enable_swap_fee && is_swap_message(msg.clone()) {
        amount.mul(swap_fee_config.swap_percent_fee).div(Uint128::new(100))
    } else {
        Uint128::zero()
    }
}

fn is_swap_message(msg: Binary) -> bool {
    match from_binary(&msg) {
        Ok(Cw20HookMsg::Swap { .. }) => {
            true
        }
        _ => false
    }
}


fn transfer(
    storage: &mut dyn Storage,
    sender: &Addr,
    recipient: &Addr,
    amount: Uint128,
) -> Result<(), ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    BALANCES.update(
        storage,
        sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        storage,
        recipient,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    Ok(())
}
