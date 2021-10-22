use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("No claims that can be released currently")]
    NothingToClaim {},

    #[error("Must send valid address to stake")]
    InvalidToken(String),

    #[error("Missed address")]
    MissedToken {},
}
