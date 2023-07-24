use cosmwasm_std::StdError;
use crate::contract::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Base(#[from] cw20_base::ContractError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Pre-sale has finished!")]
    PreSaleOver {},

    #[error("Below Minimum provided!")]
    TooLittle {},

    #[error("Wrong Minter!")]
    WrongMinter {},

    #[error("Debug Point")]
    TracePoint {},

    #[error("Minting is Paused. Use DEX to Buy.")]
    MintPaused {},

    #[error("Burning is Paused. Use DEX to Sell.")]
    BurnPaused {},

}
