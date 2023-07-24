use cosmwasm_schema::{cw_serde, QueryResponses};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use crate::curves::{decimal, Constant, Curve, DecimalPlaces, Linear, SquareRoot, Squared, Sigmoid};
use cosmwasm_std::{Addr, Binary, Uint128}; //Decimal
use cw20::Expiration;
use cw20::Logo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
	pub struct MigrateMsg {}

#[cw_serde]
pub struct InstantiateMsg {
    /// name of the supply token
    pub name: String,
    /// symbol / ticker of the supply token
    pub symbol: String,
    /// number of decimal places of the supply token, needed for proper curve math.
    /// If it is eg. BTC, where a balance of 10^8 means 1 BTC, then use 8 here.
    pub decimals: u8,

    /// this is the reserve token denom (only support native for now)
    pub reserve_denom: String,
    /// number of decimal places for the reserve token, needed for proper curve math.
    /// Same format as decimals above, eg. if it is uatom, where 1 unit is 10^-6 ATOM, use 6 here
    pub reserve_decimals: u8,

    /// enum to store the curve parameters used for this contract
    /// if you want to add a custom Curve, you should make a new contract that imports this one.
    /// write a custom `instantiate`, and then dispatch `your::execute` -> `cw20_bonding::do_execute`
    /// with your custom curve as a parameter (and same with `query` -> `do_query`)
    pub curve_type: CurveType,
}

pub type CurveFn = Box<dyn Fn(DecimalPlaces) -> Box<dyn Curve>>;

#[cw_serde]
pub enum CurveType {
    /// Constant always returns `value * 10^-scale` as spot price
    Constant { value: Uint128, scale: u32 },
    /// Linear returns `slope * 10^-scale * supply` as spot price
    Linear { slope: Uint128, scale: u32 },
    /// SquareRoot returns spot_price is slope * (supply)^0.5
    SquareRoot { slope: Uint128, scale: u32 },
    /// Squared returns spot_price is 'slope * (supply)^2'
    Squared { slope: Uint128, scale: u32 },
    /// Sigmoid returns spot_price (based on array not slope & scale)
    Sigmoid { slope: Uint128, scale: u32 },
}

impl CurveType {
    pub fn to_curve_fn(&self) -> CurveFn {
        match self.clone() {
            CurveType::Constant { value, scale } => {
                let calc = move |places| -> Box<dyn Curve> {
                    Box::new(Constant::new(decimal(value, scale), places))
                };
                Box::new(calc)
            }
            CurveType::Linear { slope, scale } => {
                let calc = move |places| -> Box<dyn Curve> {
                    Box::new(Linear::new(decimal(slope, scale), places))
                };
                Box::new(calc)
            }
            CurveType::SquareRoot { slope, scale } => {
                let calc = move |places| -> Box<dyn Curve> {
                    Box::new(SquareRoot::new(decimal(slope, scale), places))
                };
                Box::new(calc)
            }
            CurveType::Squared { slope, scale } => {
                let calc = move |places| -> Box<dyn Curve> {
                    Box::new(Squared::new(decimal(slope, scale), places))
                };
                Box::new(calc)
            }
            CurveType::Sigmoid { slope, scale } => {
                let calc = move |places| -> Box<dyn Curve> {
                    Box::new(Sigmoid::new(decimal(slope, scale), places))
                };
                Box::new(calc)
            }

        }
    }
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Buy will attempt to purchase as many supply tokens as possible.
    /// You must send only reserve tokens in that message. An affiliate
    /// wallet address may be included for rewards, or pass empty String.
    Buy {
	affiliate: String,
    },

    /// Implements CW20. Transfer is a base message to move tokens to another account without triggering actions
    Transfer { recipient: String, amount: Uint128 },
    /// Implements CW20. Burn is a base message to destroy tokens forever
    Burn { amount: Uint128 },
    /// Implements CW20.  Send is a base message to transfer tokens to a contract and trigger an action
    /// on the receiving contract.
    Send {
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    /// Implements CW20 "approval" extension. Allows spender to access an additional amount tokens
    /// from the owner's (env.sender) account. If expires is Some(), overwrites current allowance
    /// expiration with this one.
    IncreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Implements CW20 "approval" extension. Lowers the spender's access of tokens
    /// from the owner's (env.sender) account by amount. If expires is Some(), overwrites current
    /// allowance expiration with this one.
    DecreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Implements CW20 "approval" extension. Transfers amount tokens from owner -> recipient
    /// if `env.sender` has sufficient pre-approval.
    TransferFrom {
        owner: String,
        recipient: String,
        amount: Uint128,
    },
    /// Implements CW20 "approval" extension. Sends amount tokens from owner -> contract
    /// if `env.sender` has sufficient pre-approval.
    SendFrom {
        owner: String,
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    /// Implements CW20 "approval" extension. Destroys tokens forever
    BurnFrom { owner: String, amount: Uint128 },
    /// Only with the "marketing" extension. If authorized, updates marketing metadata.
    /// Setting None/null for any of these will leave it unchanged.
    /// Setting Some("") will clear this field on the contract storage
    UpdateMarketing {
        /// A URL pointing to the project behind this token.
        project: Option<String>,
        /// A longer description of the token and it's utility. Designed for tooltips or such
        description: Option<String>,
        /// The address (if any) who can update this data structure
        marketing: Option<String>,
    },
    /// If set as the "marketing" role on the contract, upload a new URL, SVG, or PNG for the token
    UploadLogo(Logo),
    /// Sets percentages for the tax as well as set the presale_price
    UpdateParamConfig { yield_percent: u32, burn_percent: u32, social_percent: u32, expense_percent: u32,
                        affiliate_percent: u32, tax_percent: u32, presale_price: u32, },
    /// Set the accounts where funds will be deposited
    UpdateAcctConfig { presale_acct: String, yield_acct: String, burn_acct: String, 
			social_acct: String, expense_acct: String, stake_acct: String,
			unstake_acct: String,},
    /// Options for transferring BASE to an external DEX
    UpdateDexferConfig { dexfer_manager: String, deposit_percent: u32, tax_exempt: String,
			token_minter: String,},
   /// Serves as an emergency switch
    UpdateSafetyConfig { can_buy: String, can_sell: String, },
   ///The current minter may set a new minter. Setting the minter to None is irreversible
    UpdateMinter { new_minter: Option<String> },
  }

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the reserve and supply quantities, as well as the spot price to buy 1 token
    #[returns(CurveInfoResponse)]
    CurveInfo {},
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    #[returns(cw20::BalanceResponse)]
    Balance { address: String },
    /// Implements CW20. Returns metadata on the contract - name, decimals, supply, etc.
    #[returns(cw20::TokenInfoResponse)]
    TokenInfo {},
    /// Implements CW20 "allowance" extension.
    /// Returns how much spender can use from owner account, 0 if unset.
    #[returns(cw20::AllowanceResponse)]
    Allowance { owner: String, spender: String },
    /// Only with "marketing" extension
    /// Returns more metadata on the contract to display in the client:
    /// - description, logo, project url, etc.
    #[returns(cw20::MarketingInfoResponse)]
    MarketingInfo {},
    /// Only with "marketing" extension
    /// Downloads the embedded logo data (if stored on chain). Errors if no logo data is stored for this
    /// contract.
    #[returns(cw20::DownloadLogoResponse)]
    DownloadLogo {},
    /// Only with "mintable" extension.
    /// Returns who can mint and the hard cap on maximum tokens after minting.
    #[returns(cw20::MinterResponse)]
    Minter {},
    /// Returns params of taxes and presale.
    #[returns(ParamInfoResponse)]
    ParamInfo {},
    /// Returns who can buy and sell on curve.
    #[returns(AcctInfoResponse)]
    AcctInfo{},
    /// Returns who can buy and sell on curve.
    #[returns(DexferInfoResponse)]
    DexferInfo {},
    /// Returns who can buy and sell on curve.
    #[returns(SafetyInfoResponse)]
    SafetyInfo {},

}

#[cw_serde]
pub struct CurveInfoResponse {
    // how many reserve tokens have been received
    pub reserve: Uint128,
    // how many supply tokens have been issued
    pub supply: Uint128,
    pub spot_price: Uint128,
    pub reserve_denom: String,
    pub tax_collected: Uint128,
}

#[cw_serde]
pub struct ParamInfoResponse {
    pub yield_percent: u32,
    pub burn_percent: u32,
    pub social_percent: u32,
    pub expense_percent: u32,
    pub affiliate_percent: u32,
    pub tax_percent: u32,
    pub presale_price: u32,
}

#[cw_serde]
pub struct AcctInfoResponse {
    pub owner: Addr,
    pub presale_acct: String,
    pub yield_acct: String,
    pub burn_acct: String,
    pub social_acct: String,
    pub expense_acct: String,
    pub stake_acct: String,
    pub unstake_acct: String,
}

#[cw_serde]
pub struct DexferInfoResponse {
    pub dexfer_manager: String,
    pub deposit_percent: u32,
    pub tax_exempt: String,
    pub token_minter: String,
}

#[cw_serde]
pub struct SafetyInfoResponse {
    pub can_buy: String,
    pub can_sell: String,

}
