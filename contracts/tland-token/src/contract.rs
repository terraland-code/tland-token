use cosmwasm_std::{
    Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, to_binary, Uint128,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cw2::{get_contract_version, set_contract_version};
use cw20::{
    BalanceResponse, Cw20Coin, Cw20ReceiveMsg, DownloadLogoResponse, EmbeddedLogo, Logo, LogoInfo,
    MarketingInfoResponse, TokenInfoResponse,
};

use crate::allowances::{
    execute_burn_from, execute_decrease_allowance, execute_increase_allowance, execute_send_from,
    execute_transfer_from, is_transfer_disabled, query_allowance,
};
use crate::enumerable::{query_all_accounts, query_all_allowances};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{BALANCES, CONFIG, Config, LOGO, MARKETING_INFO, STATE, State, TOKEN_INFO, TokenInfo};

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
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => verify_xml_logo(&logo),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => verify_png_logo(&logo),
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
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    // store config
    let cfg = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        antibot_protection_trigger_address: deps.api.addr_validate(&msg.antibot_protection_trigger_address)?,
        antibot_protcetion_burn_address: deps.api.addr_validate(&msg.antibot_protcetion_burn_address)?,
    };
    CONFIG.save(deps.storage, &cfg)?;

    // store state
    STATE.save(deps.storage, &State {
        antibot_protection_disabled_forever: false,
        antibot_protection_height: Default::default(),
    })?;

    // store marketing info
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

    Ok(Response::default())
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

pub fn create_accounts(deps: &mut DepsMut, accounts: &[Cw20Coin]) -> StdResult<Uint128> {
    let mut total_supply = Uint128::zero();
    for row in accounts {
        let address = deps.api.addr_validate(&row.address)?;
        BALANCES.save(deps.storage, &address, &row.amount)?;
        total_supply += row.amount;
    }
    Ok(total_supply)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            antibot_protection_trigger_address,
            antibot_protection_burn_address,
        } =>
            execute_update_config(deps, env, info, owner, antibot_protection_trigger_address, antibot_protection_burn_address),
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_increase_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_decrease_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(deps, env, info, owner, recipient, amount),
        ExecuteMsg::BurnFrom { owner, amount } => execute_burn_from(deps, env, info, owner, amount),
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
        } => execute_update_marketing(deps, env, info, project, description, marketing),
        ExecuteMsg::UploadLogo(logo) => execute_upload_logo(deps, env, info, logo),
        ExecuteMsg::DisableTransfers { num_of_blocks } =>
            execute_disable_transfers(deps, env, info, num_of_blocks),
        ExecuteMsg::DisableAntibotDefenseForever {} =>
            execute_disable_antibot_defense_forever(deps, env, info)
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_owner: Option<String>,
    new_antibot_protection_trigger_address: Option<String>,
    new_antibot_protection_burn_address: Option<String>,
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
        if let Some(addr) = new_antibot_protection_trigger_address {
            existing_config.antibot_protection_trigger_address = api.addr_validate(&addr)?;
        }
        if let Some(addr) = new_antibot_protection_burn_address {
            existing_config.antibot_protcetion_burn_address = api.addr_validate(&addr)?;
        }
        Ok(existing_config)
    })?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("sender", info.sender))
}

pub fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let mut rcpt_addr = deps.api.addr_validate(&recipient)?;

    // if transfer is disabled by antibot protection, then recipient address is burn address
    let state = STATE.load(deps.storage)?;
    if is_transfer_disabled(&state, env.block.height) {
        let cfg = CONFIG.load(deps.storage)?;
        rcpt_addr = cfg.antibot_protcetion_burn_address.into();
    }

    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("from", info.sender)
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_burn(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {});
    }

    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    // lower balance
    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    // reduce total_supply
    TOKEN_INFO.update(deps.storage, |mut info| -> StdResult<_> {
        info.total_supply = info.total_supply.checked_sub(amount)?;
        Ok(info)
    })?;

    let res = Response::new()
        .add_attribute("action", "burn")
        .add_attribute("from", info.sender)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let mut rcpt_addr = deps.api.addr_validate(&contract)?;

    // if transfer is disabled by antibot protection, then recipient address is burn address
    let state = STATE.load(deps.storage)?;
    if is_transfer_disabled(&state, env.block.height) {
        let cfg = CONFIG.load(deps.storage)?;
        rcpt_addr = cfg.antibot_protcetion_burn_address.into();
    }

    // move the tokens to the contract
    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "send")
        .add_attribute("from", &info.sender)
        .add_attribute("to", &contract)
        .add_attribute("amount", amount)
        .add_message(
            Cw20ReceiveMsg {
                sender: info.sender.into(),
                amount,
                msg,
            }
                .into_cosmos_msg(contract)?,
        );
    Ok(res)
}

pub fn execute_update_marketing(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    project: Option<String>,
    description: Option<String>,
    marketing: Option<String>,
) -> Result<Response, ContractError> {
    let mut marketing_info = MARKETING_INFO
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    if marketing_info
        .marketing
        .as_ref()
        .ok_or(ContractError::Unauthorized {})?
        != &info.sender
    {
        return Err(ContractError::Unauthorized {});
    }

    match project {
        Some(empty) if empty.trim().is_empty() => marketing_info.project = None,
        Some(project) => marketing_info.project = Some(project),
        None => (),
    }

    match description {
        Some(empty) if empty.trim().is_empty() => marketing_info.description = None,
        Some(description) => marketing_info.description = Some(description),
        None => (),
    }

    match marketing {
        Some(empty) if empty.trim().is_empty() => marketing_info.marketing = None,
        Some(marketing) => marketing_info.marketing = Some(deps.api.addr_validate(&marketing)?),
        None => (),
    }

    if marketing_info.project.is_none()
        && marketing_info.description.is_none()
        && marketing_info.marketing.is_none()
        && marketing_info.logo.is_none()
    {
        MARKETING_INFO.remove(deps.storage);
    } else {
        MARKETING_INFO.save(deps.storage, &marketing_info)?;
    }

    let res = Response::new().add_attribute("action", "update_marketing");
    Ok(res)
}

pub fn execute_upload_logo(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    logo: Logo,
) -> Result<Response, ContractError> {
    let mut marketing_info = MARKETING_INFO
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    verify_logo(&logo)?;

    if marketing_info
        .marketing
        .as_ref()
        .ok_or(ContractError::Unauthorized {})?
        != &info.sender
    {
        return Err(ContractError::Unauthorized {});
    }

    LOGO.save(deps.storage, &logo)?;

    let logo_info = match logo {
        Logo::Url(url) => LogoInfo::Url(url),
        Logo::Embedded(_) => LogoInfo::Embedded,
    };

    marketing_info.logo = Some(logo_info);
    MARKETING_INFO.save(deps.storage, &marketing_info)?;

    let res = Response::new().add_attribute("action", "upload_logo");
    Ok(res)
}

pub fn execute_disable_transfers(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    num_of_blocks: u64,
) -> Result<Response, ContractError> {
    // authorized address
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.antibot_protection_trigger_address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // check if this functionality is not disabled
    let state = STATE.load(deps.storage)?;
    if state.antibot_protection_disabled_forever {
        return Err(ContractError::DisabledAction {});
    }

    // calculate antibot protection block height
    STATE.update(deps.storage, |mut existing_state| -> StdResult<_> {
        existing_state.antibot_protection_height = env.block.height + num_of_blocks;
        Ok(existing_state)
    })?;

    Ok(Response::new()
        .add_attribute("action", "disable_transfers")
        .add_attribute("sender", info.sender))
}

pub fn execute_disable_antibot_defense_forever(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // authorized owner
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    STATE.update(deps.storage, |mut existing_state| -> StdResult<_> {
        existing_state.antibot_protection_disabled_forever = true;
        existing_state.antibot_protection_height = 0;
        Ok(existing_state)
    })?;

    Ok(Response::new()
        .add_attribute("action", "disable_antibot_defense_forever")
        .add_attribute("sender", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
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
    }
}

pub fn query_config(deps: Deps) -> StdResult<Config> {
    Ok(CONFIG.load(deps.storage)?)
}

pub fn query_state(deps: Deps) -> StdResult<State> {
    Ok(STATE.load(deps.storage)?)
}

pub fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let balance = BALANCES
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(BalanceResponse { balance })
}

pub fn query_token_info(deps: Deps) -> StdResult<TokenInfoResponse> {
    let info = TOKEN_INFO.load(deps.storage)?;
    let res = TokenInfoResponse {
        name: info.name,
        symbol: info.symbol,
        decimals: info.decimals,
        total_supply: info.total_supply,
    };
    Ok(res)
}

pub fn query_marketing_info(deps: Deps) -> StdResult<MarketingInfoResponse> {
    Ok(MARKETING_INFO.may_load(deps.storage)?.unwrap_or_default())
}

pub fn query_download_logo(deps: Deps) -> StdResult<DownloadLogoResponse> {
    let logo = LOGO.load(deps.storage)?;
    match logo {
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => Ok(DownloadLogoResponse {
            mime_type: "image/svg+xml".to_owned(),
            data: logo,
        }),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => Ok(DownloadLogoResponse {
            mime_type: "image/png".to_owned(),
            data: logo,
        }),
        Logo::Url(_) => Err(StdError::not_found("logo")),
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, coins, CosmosMsg, from_binary, StdError, SubMsg, WasmMsg};
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};

    use crate::msg::InstantiateMarketingInfo;

    use super::*;

    fn get_balance<T: Into<String>>(deps: Deps, address: T) -> Uint128 {
        query_balance(deps, address.into()).unwrap().balance
    }

    // this will set up the instantiation for other tests
    fn do_instantiate(deps: DepsMut, addr: &str, amount: Uint128) -> TokenInfoResponse {
        _do_instantiate(deps, addr, amount)
    }

    // this will set up the instantiation for other tests
    fn _do_instantiate(
        mut deps: DepsMut,
        addr: &str,
        amount: Uint128,
    ) -> TokenInfoResponse {
        let instantiate_msg = InstantiateMsg {
            owner: "OWNER".to_string(),
            name: "Auto Gen".to_string(),
            symbol: "AUTO".to_string(),
            decimals: 3,
            initial_balances: vec![Cw20Coin {
                address: addr.to_string(),
                amount,
            }],
            marketing: None,
            antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
            antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = instantiate(deps.branch(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let meta = query_token_info(deps.as_ref()).unwrap();
        assert_eq!(
            meta,
            TokenInfoResponse {
                name: "Auto Gen".to_string(),
                symbol: "AUTO".to_string(),
                decimals: 3,
                total_supply: amount,
            }
        );
        assert_eq!(get_balance(deps.as_ref(), addr), amount);
        meta
    }

    const PNG_HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    mod instantiate {
        use super::*;

        #[test]
        fn basic() {
            let mut deps = mock_dependencies(&[]);
            let amount = Uint128::from(11223344u128);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![Cw20Coin {
                    address: String::from("addr0000"),
                    amount,
                }],
                marketing: None,
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };
            let info = mock_info("creator", &[]);
            let env = mock_env();
            let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
            assert_eq!(0, res.messages.len());

            assert_eq!(
                query_token_info(deps.as_ref()).unwrap(),
                TokenInfoResponse {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    total_supply: amount,
                }
            );
            assert_eq!(
                get_balance(deps.as_ref(), "addr0000"),
                Uint128::new(11223344)
            );
        }

        mod marketing {
            use super::*;

            #[test]
            fn basic() {
                let mut deps = mock_dependencies(&[]);
                let instantiate_msg = InstantiateMsg {
                    owner: "OWNER".to_string(),
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    initial_balances: vec![],
                    marketing: Some(InstantiateMarketingInfo {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some("marketing".to_owned()),
                        logo: Some(Logo::Url("url".to_owned())),
                    }),
                    antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                    antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
                };

                let info = mock_info("creator", &[]);
                let env = mock_env();
                let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
                assert_eq!(0, res.messages.len());

                assert_eq!(
                    query_marketing_info(deps.as_ref()).unwrap(),
                    MarketingInfoResponse {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some(Addr::unchecked("marketing")),
                        logo: Some(LogoInfo::Url("url".to_owned())),
                    }
                );

                let err = query_download_logo(deps.as_ref()).unwrap_err();
                assert!(
                    matches!(err, StdError::NotFound { .. }),
                    "Expected StdError::NotFound, received {}",
                    err
                );
            }

            #[test]
            fn invalid_marketing() {
                let mut deps = mock_dependencies(&[]);
                let instantiate_msg = InstantiateMsg {
                    owner: "OWNER".to_string(),
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    initial_balances: vec![],
                    marketing: Some(InstantiateMarketingInfo {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some("m".to_owned()),
                        logo: Some(Logo::Url("url".to_owned())),
                    }),
                    antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                    antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
                };

                let info = mock_info("creator", &[]);
                let env = mock_env();
                instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap_err();

                let err = query_download_logo(deps.as_ref()).unwrap_err();
                assert!(
                    matches!(err, StdError::NotFound { .. }),
                    "Expected StdError::NotFound, received {}",
                    err
                );
            }
        }
    }

    #[test]
    fn instantiate_multiple_accounts() {
        let mut deps = mock_dependencies(&[]);
        let amount1 = Uint128::from(11223344u128);
        let addr1 = String::from("addr0001");
        let amount2 = Uint128::from(7890987u128);
        let addr2 = String::from("addr0002");
        let instantiate_msg = InstantiateMsg {
            owner: "OWNER".to_string(),
            name: "Bash Shell".to_string(),
            symbol: "BASH".to_string(),
            decimals: 6,
            initial_balances: vec![
                Cw20Coin {
                    address: addr1.clone(),
                    amount: amount1,
                },
                Cw20Coin {
                    address: addr2.clone(),
                    amount: amount2,
                },
            ],
            marketing: None,
            antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
            antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(
            query_token_info(deps.as_ref()).unwrap(),
            TokenInfoResponse {
                name: "Bash Shell".to_string(),
                symbol: "BASH".to_string(),
                decimals: 6,
                total_supply: amount1 + amount2,
            }
        );
        assert_eq!(get_balance(deps.as_ref(), addr1), amount1);
        assert_eq!(get_balance(deps.as_ref(), addr2), amount2);
    }

    #[test]
    fn queries_work() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let amount1 = Uint128::from(12340000u128);

        let expected = do_instantiate(deps.as_mut(), &addr1, amount1);

        // check meta query
        let loaded = query_token_info(deps.as_ref()).unwrap();
        assert_eq!(expected, loaded);

        let _info = mock_info("test", &[]);
        let env = mock_env();
        // check balance query (full)
        let data = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Balance { address: addr1 },
        )
            .unwrap();
        let loaded: BalanceResponse = from_binary(&data).unwrap();
        assert_eq!(loaded.balance, amount1);

        // check balance query (empty)
        let data = query(
            deps.as_ref(),
            env,
            QueryMsg::Balance {
                address: String::from("addr0002"),
            },
        )
            .unwrap();
        let loaded: BalanceResponse = from_binary(&data).unwrap();
        assert_eq!(loaded.balance, Uint128::zero());
    }

    #[test]
    fn transfer() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let addr2 = String::from("addr0002");
        let amount1 = Uint128::from(12340000u128);
        let transfer = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot transfer nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: Uint128::zero(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // cannot send more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: too_much,
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // cannot send from empty account
        let info = mock_info(addr2.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr1.clone(),
            amount: transfer,
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // valid transfer
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: transfer,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let remainder = amount1.checked_sub(transfer).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1), remainder);
        assert_eq!(get_balance(deps.as_ref(), addr2), transfer);
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );
    }

    #[test]
    fn burn() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        // only owner can burn
        let addr1 = String::from("OWNER");
        let amount1 = Uint128::from(12340000u128);
        let burn = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot burn nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn {
            amount: Uint128::zero(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );

        // cannot burn more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn { amount: too_much };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );

        // valid burn reduces total supply
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn { amount: burn };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let remainder = amount1.checked_sub(burn).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1), remainder);
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            remainder
        );
    }

    #[test]
    fn send() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let contract = String::from("addr0002");
        let amount1 = Uint128::from(12340000u128);
        let transfer = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);
        let send_msg = Binary::from(r#"{"some":123}"#.as_bytes());

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot send nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: Uint128::zero(),
            msg: send_msg.clone(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // cannot send more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: too_much,
            msg: send_msg.clone(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // valid transfer
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: transfer,
            msg: send_msg.clone(),
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 1);

        // ensure proper send message sent
        // this is the message we want delivered to the other side
        let binary_msg = Cw20ReceiveMsg {
            sender: addr1.clone(),
            amount: transfer,
            msg: send_msg,
        }
            .into_binary()
            .unwrap();
        // and this is how it must be wrapped for the vm to process it
        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract.clone(),
                msg: binary_msg,
                funds: vec![],
            }))
        );

        // ensure balance is properly transferred
        let remainder = amount1.checked_sub(transfer).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1), remainder);
        assert_eq!(get_balance(deps.as_ref(), contract), transfer);
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );
    }

    #[test]
    fn antibot_protection() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let owner = String::from("OWNER");
        let addr = String::from("addr");
        let antibot_trigger = String::from("ANTIBOT_TRIGGER");
        let antibot_burn = String::from("ANTIBOT_BURN");
        let amount = Uint128::from(1_000_000u128);

        do_instantiate(deps.as_mut(), &owner, amount);

        // cannot disable transfer by the owner or other address
        let env = mock_env();
        let info = mock_info(owner.as_ref(), &[]);
        let msg = ExecuteMsg::DisableTransfers {
            num_of_blocks: 2,
        };
        let err = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
        assert!(matches!(err, ContractError::Unauthorized {}));
        let info = mock_info(addr.as_ref(), &[]);
        let err = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
        assert!(matches!(err, ContractError::Unauthorized {}));

        // disable transfer by the antibot trigger address only
        let info = mock_info(antibot_trigger.as_ref(), &[]);
        execute(deps.as_mut(), env, info, msg.clone()).unwrap();

        // all transfers are disabled for current and next block height,
        // ternsfered tokens will be sent to antibot burn address
        let env = mock_env();
        let info = mock_info(owner.as_ref(), &[]);
        let msg = ExecuteMsg::Transfer {
            recipient: addr.clone(),
            amount: Uint128::from(100_000u128),
        };
        execute(deps.as_mut(), env, info, msg.clone()).unwrap();
        assert_eq!(get_balance(deps.as_ref(), owner.clone()), Uint128::from(900_000u128));
        assert_eq!(get_balance(deps.as_ref(), antibot_burn.clone()), Uint128::from(100_000u128));

        // ... next block
        let mut env = mock_env();
        env.block.height += 1;
        let info = mock_info(owner.as_ref(), &[]);
        let msg = ExecuteMsg::Transfer {
            recipient: addr.clone(),
            amount: Uint128::from(100_000u128),
        };
        execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();
        assert_eq!(get_balance(deps.as_ref(), owner.clone()), Uint128::from(800_000u128));
        assert_eq!(get_balance(deps.as_ref(), antibot_burn.clone()), Uint128::from(200_000u128));

        // after 2 blocks transfer is enabled again
        let mut env = mock_env();
        env.block.height += 2;
        let info = mock_info(owner.as_ref(), &[]);
        let msg = ExecuteMsg::Transfer {
            recipient: addr.clone(),
            amount: Uint128::from(100_000u128),
        };
        execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();
        assert_eq!(get_balance(deps.as_ref(), owner.clone()), Uint128::from(700_000u128));
        assert_eq!(get_balance(deps.as_ref(), antibot_burn.clone()), Uint128::from(200_000u128));
        assert_eq!(get_balance(deps.as_ref(), addr.clone()), Uint128::from(100_000u128));

        // disable antibot protection forever
        let env = mock_env();
        let info = mock_info(owner.as_ref(), &[]);
        let msg = ExecuteMsg::DisableAntibotDefenseForever {};
        execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();

        // transfer enabled, funds are not send to antibot burn address anymore
        let env = mock_env();
        let info = mock_info(owner.as_ref(), &[]);
        let msg = ExecuteMsg::Transfer {
            recipient: addr.clone(),
            amount: Uint128::from(100_000u128),
        };
        execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap();
        assert_eq!(get_balance(deps.as_ref(), owner.clone()), Uint128::from(600_000u128));
        assert_eq!(get_balance(deps.as_ref(), antibot_burn.clone()), Uint128::from(200_000u128));
        assert_eq!(get_balance(deps.as_ref(), addr.clone()), Uint128::from(200_000u128));

        // check state
        let state = query_state(deps.as_ref()).unwrap();
        assert_eq!(state.antibot_protection_height, 0);
        assert_eq!(state.antibot_protection_disabled_forever, true);
    }

    mod marketing {
        use super::*;

        #[test]
        fn update_unauthorised() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("marketing".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: Some("New project".to_owned()),
                    description: Some("Better description".to_owned()),
                    marketing: Some("creator".to_owned()),
                },
            )
                .unwrap_err();

            assert_eq!(err, ContractError::Unauthorized {});

            // Ensure marketing didn't change
            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("marketing")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_project() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: Some("New project".to_owned()),
                    description: None,
                    marketing: None,
                },
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("New project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn clear_project() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: Some("".to_owned()),
                    description: None,
                    marketing: None,
                },
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: None,
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_description() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: Some("Better description".to_owned()),
                    marketing: None,
                },
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Better description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn clear_description() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: Some("".to_owned()),
                    marketing: None,
                },
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: None,
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_marketing() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: None,
                    marketing: Some("marketing".to_owned()),
                },
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("marketing")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_marketing_invalid() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: None,
                    marketing: Some("m".to_owned()),
                },
            )
                .unwrap_err();

            assert!(
                matches!(err, ContractError::Std(_)),
                "Expected Std error, received: {}",
                err
            );

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn clear_marketing() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: None,
                    marketing: Some("".to_owned()),
                },
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: None,
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_url() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "ONWER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Url("new_url".to_owned())),
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("new_url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_png() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Png(PNG_HEADER.into()))),
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Embedded),
                }
            );

            assert_eq!(
                query_download_logo(deps.as_ref()).unwrap(),
                DownloadLogoResponse {
                    mime_type: "image/png".to_owned(),
                    data: PNG_HEADER.into(),
                }
            );
        }

        #[test]
        fn update_logo_svg() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = "<?xml version=\"1.0\"?><svg></svg>".as_bytes();
            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Svg(img.into()))),
            )
                .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Embedded),
                }
            );

            assert_eq!(
                query_download_logo(deps.as_ref()).unwrap(),
                DownloadLogoResponse {
                    mime_type: "image/svg+xml".to_owned(),
                    data: img.into(),
                }
            );
        }

        #[test]
        fn update_logo_png_oversized() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = [&PNG_HEADER[..], &[1; 6000][..]].concat();
            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Png(img.into()))),
            )
                .unwrap_err();

            assert_eq!(err, ContractError::LogoTooBig {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_svg_oversized() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = [
                "<?xml version=\"1.0\"?><svg>",
                std::str::from_utf8(&[b'x'; 6000]).unwrap(),
                "</svg>",
            ]
                .concat()
                .into_bytes();

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Svg(img.into()))),
            )
                .unwrap_err();

            assert_eq!(err, ContractError::LogoTooBig {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_png_invalid() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = &[1];
            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Png(img.into()))),
            )
                .unwrap_err();

            assert_eq!(err, ContractError::InvalidPngHeader {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_svg_invalid() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                owner: "OWNER".to_string(),
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
                antibot_protection_trigger_address: "ANTIBOT_TRIGGER".to_string(),
                antibot_protcetion_burn_address: "ANTIBOT_BURN".to_string(),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = &[1];

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Svg(img.into()))),
            )
                .unwrap_err();

            assert_eq!(err, ContractError::InvalidXmlPreamble {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }
    }
}
