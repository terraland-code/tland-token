use cosmwasm_std::StdError;
use cw0::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("No claims that can be released currently")]
    NothingToClaim {},

    #[error("Must send valid address to stake")]
    InvalidToken(String),

    #[error("Missed address")]
    MissedToken {},

    #[error("Staking closed")]
    StakingClosed {},

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid fee amount")]
    InvalidFeeAmount {},

    #[error("No reward to release")]
    NothingToWithdraw {},
}
