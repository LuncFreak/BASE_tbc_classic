// This contract is forked from on github.com/CosmWasm/cw-tokens/tree/main/contracts/cw20-bonding
// Changes were made to make compatible with Terra Classic and add Sigmoid curve

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use thiserror::Error;

use cosmwasm_std::{
	attr, coins, to_binary, Addr, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo,
	Response, StdError, StdResult, Uint128, CosmosMsg, Coin,
};

use cw2::set_contract_version;
use cw20_base::allowances::{
	deduct_allowance, execute_decrease_allowance, execute_increase_allowance, execute_send_from,
	execute_transfer_from, query_allowance,
};

use cw20_base::contract::{
	execute_burn, execute_mint, execute_send, execute_transfer,
	execute_update_marketing, execute_upload_logo,
	query_minter, query_balance, query_token_info,
	query_marketing_info, query_download_logo,
};

use cw20_base::state::{MinterData, TokenInfo, TOKEN_INFO, LOGO,  MARKETING_INFO,
};
use cw20::{
	Logo, LogoInfo, MarketingInfoResponse,
};
use crate::curves::DecimalPlaces;
use crate::error::ContractError;
use crate::msg::{CurveFn, CurveInfoResponse, ParamInfoResponse, AcctInfoResponse,
	DexferInfoResponse, SafetyInfoResponse, ExecuteMsg, InstantiateMsg,
QueryMsg, MigrateMsg};

use crate::state::{CurveState, CURVE_STATE, CURVE_TYPE,
	PARAM_CONFIG, ParamConfig, ACCT_CONFIG, AcctConfig,
	DEXFER_CONFIG, DexferConfig, SAFETY_CONFIG, SafetyConfig, };

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-bonding";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
	Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
deps: DepsMut,
env: Env,
info: MessageInfo,
msg: InstantiateMsg,
) -> Result<Response, ContractError> {
	nonpayable(&info)?;
	set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
	
	let creator_addr = String::from(&info.sender);
	
	let paramconfig = ParamConfig {
		yield_percent: 0,
		burn_percent: 0,
		social_percent: 0,
		expense_percent: 0,
		affiliate_percent: 0,
		tax_percent: 0,
		presale_price: 200,
	};
	// Save the owner address to contract storage.
	PARAM_CONFIG.save(deps.storage, &paramconfig)?;
	
	let acctconfig = AcctConfig {
		owner: info.sender,
		presale_acct: "none".to_string(),
		yield_acct: "none".to_string(),
		burn_acct: "none".to_string(),
		social_acct: "none".to_string(),
		expense_acct: "none".to_string(),
		stake_acct: "none".to_string(),
		unstake_acct: "none".to_string(),
	};
	// Save the owner address to contract storage.
	ACCT_CONFIG.save(deps.storage, &acctconfig)?;
	
	let dexferconfig = DexferConfig {
		dexfer_manager: "none".to_string(),
		deposit_percent: 0,
		tax_exempt: "none".to_string(),
		token_minter: "contract".to_string(),
	};
	// Save the owner address to contract storage.
	DEXFER_CONFIG.save(deps.storage, &dexferconfig)?;
	
	let safetyconfig = SafetyConfig {
		can_buy: creator_addr.to_string(),
		can_sell: creator_addr.to_string(),
	};
	// Save the owner address to contract storage.
	SAFETY_CONFIG.save(deps.storage, &safetyconfig)?;
	
	// store token info using cw20-base format
	let data = TokenInfo {
		name: msg.name,
		symbol: msg.symbol,
		decimals: msg.decimals,
		total_supply: Uint128::zero(),
		// set self as minter, so we can properly execute mint and burn
		mint: Some(MinterData {
			minter: env.contract.address,
			cap: None,
		}),
	};
	TOKEN_INFO.save(deps.storage, &data)?;
	
	let logo = Logo::Url("".to_owned());
	LOGO.save(deps.storage, &logo)?;
	
	match logo {
		Logo::Url(url) => Some(LogoInfo::Url(url)),
		Logo::Embedded(_) => Some(LogoInfo::Embedded),
	};
	
	let metadata = MarketingInfoResponse {
		project: Some("".to_owned()),
		description: Some("".to_owned()),
		marketing: Some(acctconfig.owner),
		logo: Some(LogoInfo::Url("".to_owned())),
	};
	MARKETING_INFO.save(deps.storage, &metadata)?;
	
	let places = DecimalPlaces::new(msg.decimals, msg.reserve_decimals);
	let supply = CurveState::new(msg.reserve_denom, places);
	
	CURVE_STATE.save(deps.storage, &supply)?;
	
	CURVE_TYPE.save(deps.storage, &msg.curve_type)?;
	
	Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
deps: DepsMut,
env: Env,
info: MessageInfo,
msg: ExecuteMsg,
) -> Result<Response, ContractError> {
	// default implementation stores curve info as enum, you can do something else in a derived
	// contract and just pass in your custom curve to do_execute
	let curve_type = CURVE_TYPE.load(deps.storage)?;
	let curve_fn = curve_type.to_curve_fn();
	do_execute(deps, env, info, msg, curve_fn)
}

/// We pull out logic here, so we can import this from another contract and set a different Curve.
/// This contacts sets a curve with an enum in InstantiateMsg and stored in state, but you may want
/// to use custom math not included - make this easily reusable
pub fn do_execute(
deps: DepsMut,
env: Env,
info: MessageInfo,
msg: ExecuteMsg,
curve_fn: CurveFn,
) -> Result<Response, ContractError> {
	match msg {
		ExecuteMsg::Buy { affiliate } => execute_buy(deps, env, info, curve_fn, affiliate),
		
		// we override these from cw20
		ExecuteMsg::Burn { amount } => Ok(execute_sell(deps, env, info, curve_fn, amount)?),
		ExecuteMsg::BurnFrom { owner, amount } => {
			Ok(execute_sell_from(deps, env, info, curve_fn, owner, amount)?)
		}
		
		// these all come from cw20-base to implement the cw20 standard
		ExecuteMsg::Transfer { recipient, amount } => {
			Ok(execute_transfer(deps, env, info, recipient, amount)?)
		}
		ExecuteMsg::Send {
			contract,
			amount,
			msg,
		} => Ok(execute_send(deps, env, info, contract, amount, msg)?),
		ExecuteMsg::IncreaseAllowance {
			spender,
			amount,
			expires,
		} => Ok(execute_increase_allowance(
		deps, env, info, spender, amount, expires,
		)?),
		ExecuteMsg::DecreaseAllowance {
			spender,
			amount,
			expires,
		} => Ok(execute_decrease_allowance(
		deps, env, info, spender, amount, expires,
		)?),
		ExecuteMsg::TransferFrom {
			owner,
			recipient,
			amount,
		} => Ok(execute_transfer_from(
		deps, env, info, owner, recipient, amount,
		)?),
		ExecuteMsg::SendFrom {
			owner,
			contract,
			amount,
			msg,
		} => Ok(execute_send_from(deps, env, info, owner, contract, amount, msg,
		)?),
		ExecuteMsg::UpdateMarketing {
			project,
			description,
			marketing,
		} => Ok(execute_update_marketing(deps, env, info, project, description, marketing)?),
		ExecuteMsg::UploadLogo(logo) => Ok(execute_upload_logo(deps, env, info, logo)?),
		ExecuteMsg::UpdateParamConfig {  yield_percent, burn_percent, social_percent, expense_percent,
			affiliate_percent, tax_percent, presale_price,
		} => Ok(execute_update_paramconfig(deps, env, info, yield_percent, burn_percent, social_percent,
		expense_percent,affiliate_percent, tax_percent, presale_price,
		)?),
		ExecuteMsg::UpdateAcctConfig {presale_acct, yield_acct, burn_acct, social_acct,
			expense_acct, stake_acct, unstake_acct,
		} => Ok(execute_update_acctconfig(deps, env, info, presale_acct, yield_acct, burn_acct,
		social_acct, expense_acct, stake_acct, unstake_acct,
		)?),
		ExecuteMsg::UpdateDexferConfig { dexfer_manager, deposit_percent, tax_exempt, token_minter,
		} => Ok(execute_update_dexferconfig(deps, env, info, dexfer_manager, deposit_percent,
		tax_exempt, token_minter,
		)?),
		ExecuteMsg::UpdateSafetyConfig { can_buy, can_sell,
		} => Ok(execute_update_safetyconfig(deps, env, info, can_buy, can_sell,
		)?),
		ExecuteMsg::UpdateMinter { new_minter,
		} => Ok(execute_update_minter(deps, env, info, new_minter,
		)?),
	}
}

pub fn execute_buy(
deps: DepsMut,
env: Env,
info: MessageInfo,
curve_fn: CurveFn,
affiliate: String,
) -> Result<Response, ContractError> {
	
	//Check can_buy flag
	let check = SAFETY_CONFIG.load(deps.storage)?;
	
	if  &check.can_buy != "1" && &check.can_buy != &info.sender.to_string() {
		return Err(ContractError::MintPaused{});
	}
	
	// Load state data
	let mut state = CURVE_STATE.load(deps.storage)?;
	let mut payment = must_pay(&info, &state.reserve_denom)?;
	
	let accounts = ACCT_CONFIG.load(deps.storage)?;
	let params = PARAM_CONFIG.load(deps.storage)?;
	let special = DEXFER_CONFIG.load(deps.storage)?;
	
	let curve = curve_fn(state.clone().decimals);
	let spot_price = curve.spot_price(state.supply);
	let presale_price: Uint128 = Uint128::new(params.presale_price.into());
	
	let mut minted: Uint128 = Uint128::new(0);
	let mut affiliate_amt: Uint128 = Uint128::new(0);
	
	//Give error if the presale has ended
	if  presale_price != Uint128::new(0) && presale_price.u128() < spot_price.u128() {
		return Err(ContractError::PreSaleOver{});
	}
	
	// fund denom (uluna)
	let reserve_denom = &state.reserve_denom;
	
	// Messages(tx) buffer
	let mut messages = vec![];
	
	//Save gross-in before deductions
	let gross_in = payment.u128();
	
	// Calc tax
	let tax_full_amt: Uint128;
	let mut net_payment_amt: Uint128;
	let mut presale_fund = Uint128::new(0u128);

	let mut deposit_amt = Uint128::new(0u128);
	let mut returned_amt = Uint128::new(0u128);

	//Subtract mandatory chain_burn_tax (for buy&sell together since not paid on burn)
	payment = payment.clone()
		.checked_mul(Uint128::new(995)).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
	
	// Don't charge BASE Tax in some special cases
	if  presale_price == Uint128::new(0) && &special.dexfer_manager != &info.sender.to_string() && &special.tax_exempt != &info.sender.to_string() {
		
		tax_full_amt = payment
		.checked_mul(Uint128::new(params.tax_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		//This is the amount left after the total tax is collected
		net_payment_amt = payment - tax_full_amt;
		
		// tax breakdown
		let tax_yield_amt = tax_full_amt
		.checked_mul(Uint128::new(params.yield_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		let tax_burn_amt = tax_full_amt
		.checked_mul(Uint128::new(params.burn_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		let tax_social_amt = tax_full_amt
		.checked_mul(Uint128::new(params.social_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		let tax_expense_amt = tax_full_amt
		.checked_sub(tax_yield_amt).unwrap()
		.checked_sub(tax_burn_amt).unwrap()
		.checked_sub(tax_social_amt).unwrap();
		
		// tax deposit addresses
		let tax_yield_addr  = deps.api.addr_validate(&accounts.yield_acct)?;
		let tax_burn_addr   = deps.api.addr_validate(&accounts.burn_acct)?;
		let tax_social_addr = deps.api.addr_validate(&accounts.social_acct)?;
		let tax_expen_addr  = deps.api.addr_validate(&accounts.expense_acct)?;
		
		//Update lifetime tax collected
		state.tax_collected += Uint128::from(tax_full_amt);
		
		// Build messages(tx) to send
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_yield_addr.to_string(),
			amount: coins(tax_yield_amt.u128(), reserve_denom),
		}));
		
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_burn_addr.to_string(),
			amount: coins(tax_burn_amt.u128(), reserve_denom),
		}));
		
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_social_addr.to_string(),
			amount: coins(tax_social_amt.u128(), reserve_denom),
		}));
		
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_expen_addr.to_string(),
			amount: coins(tax_expense_amt.u128(), reserve_denom),
		}));
		
		payment = Uint128::from(net_payment_amt);
		
	}
	else{
		tax_full_amt = Uint128::zero();
	}
	
	//Msg for affiliate reward
	if  affiliate != "" && &special.dexfer_manager != &info.sender.to_string() && &special.tax_exempt != &info.sender.to_string(){
		
		let affiliate_addr = deps.api.addr_validate(&affiliate)?;
		
		affiliate_amt = payment
		.checked_mul(Uint128::new(params.affiliate_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		net_payment_amt = payment.checked_sub(affiliate_amt).unwrap();
		
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: affiliate_addr.to_string(),
			amount: coins(affiliate_amt.u128(), reserve_denom),
		}));
		
		payment = net_payment_amt;
	}
	
	// calculate how many tokens can be purchased with this
	// rides curve if presale_price == 0, else uses presale_price
	if  presale_price == Uint128::new(0) || &special.dexfer_manager == &info.sender.to_string(){
		state.reserve += payment;
		let new_supply = curve.supply(state.reserve);
		minted = new_supply
		.checked_sub(state.supply)
		.map_err(StdError::overflow)?;
		state.supply = new_supply;
		
		CURVE_STATE.save(deps.storage, &state)?;
	}
	else {
		// Calc expected supply
		if payment.u128() < (2u128 * presale_price.u128()) {
			return Err(ContractError::TooLittle{});
		}
		
		let result = (payment.u128() * 100u128) / presale_price.u128();
		minted = Uint128::from(result.checked_mul(1000000u128).unwrap());
		minted = minted.checked_div(Uint128::new(100u128)).unwrap();
		
		// Calc amounts for Reserve and pre-sale fund
		let before_reserve = curve.reserve(state.supply);
		let after_reserve = curve.reserve(state.supply + minted);
		let delta_reserve = after_reserve - before_reserve;
		presale_fund = payment.clone().checked_sub(delta_reserve).unwrap();
		
		// Update State Variable
		state.reserve += delta_reserve;
		state.supply += minted;
		
		CURVE_STATE.save(deps.storage, &state)?;
		
		// Build message for sending to pre-sale fund
		let presale_addr  = deps.api.addr_validate(&accounts.presale_acct)?;
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: presale_addr.to_string(),
			amount: coins(presale_fund.u128(), reserve_denom),
		}));
		
		payment = delta_reserve;
		
	}
	
	// Refund all but deposit to dexfer_manager to fund DEX
	if  &special.dexfer_manager == &info.sender.to_string()  {
		
		let dexfer_addr = deps.api.addr_validate(&special.dexfer_manager)?;
		
		deposit_amt = payment.clone()
		.checked_mul(Uint128::new(special.deposit_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		returned_amt = payment.clone()
		.checked_sub(deposit_amt).unwrap();
		
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: dexfer_addr.to_string(),
			amount: coins(returned_amt.clone().u128(), reserve_denom),
		}));
	}
	else{
		
        //send amount left to stake account
        let stake_addr = deps.api.addr_validate(&accounts.stake_acct)?;
        messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: stake_addr.to_string(),
			amount: coins(payment.clone().u128(), reserve_denom),
		}));
	}
	// call into cw20-base to mint the token, call as self as no one else is allowed
	let sender_addr = env.contract.address.clone();
	if &special.token_minter != "contract" {
		//Only minter can mint
		return Err(ContractError::WrongMinter{})
	}
	
	let sub_info = MessageInfo {
		sender: sender_addr,
		funds: vec![],
	};
	execute_mint(deps, env, sub_info, info.sender.to_string(), minted.clone())?;
	
	//Send Transactions
	let mut res = Response::new()
	.add_messages(messages)
	.add_attribute("action", "buy")
	.add_attribute("from", info.sender.clone());
	
	res = res.add_attribute("LUNC Sent: ", Uint128::from(gross_in));
	res = res.add_attribute("LUNC Tax: ", Uint128::from(tax_full_amt));
	
	if  affiliate != "" && &special.dexfer_manager != &info.sender.to_string() && &special.tax_exempt != &info.sender.to_string() {
		res = res.add_attribute("Affiliate Reward: ", Uint128::from(affiliate_amt));
	}

	if &special.dexfer_manager == &info.sender.to_string(){
		res = res.add_attribute("LUNC Deposit: ", Uint128::from(deposit_amt));
		res = res.add_attribute("Xfer to DEX: ", Uint128::from(returned_amt));
	}
	else{
	res = res.add_attribute("LUNC Staked: ", Uint128::from(payment));
	}

	if  presale_price != Uint128::new(0) && &special.dexfer_manager != &info.sender.to_string(){
		res = res.add_attribute("Pre-sale Fund: ", Uint128::from(presale_fund));
	}

	res = res.add_attribute("BASE Minted: ", Uint128::from(minted));
	
	Ok(res)
}

pub fn execute_sell(
deps: DepsMut,
env: Env,
info: MessageInfo,
curve_fn: CurveFn,
amount: Uint128,
) -> Result<Response, ContractError> {
	
	//Check can_sell flag
	let check = SAFETY_CONFIG.load(deps.storage)?;
	
	if  &check.can_sell != "1" && &check.can_sell != &info.sender.to_string() {
		return Err(ContractError::BurnPaused{});
	}
	
	nonpayable(&info)?;
	let receiver = info.sender.clone();
	// do all the work
	let mut res = do_sell(deps, env, info, curve_fn, receiver, amount)?;
	
	// add our custom attributes
	res.attributes.push(attr("action", "burn"));
	Ok(res)
}

pub fn execute_sell_from(
deps: DepsMut,
env: Env,
info: MessageInfo,
curve_fn: CurveFn,
owner: String,
amount: Uint128,
) -> Result<Response, ContractError> {
	nonpayable(&info)?;
	let owner_addr = deps.api.addr_validate(&owner)?;
	let spender_addr = info.sender.clone();
	
	// deduct allowance before doing anything else have enough allowance
	deduct_allowance(deps.storage, &owner_addr, &spender_addr, &env.block, amount)?;
	
	// do all the work in do_sell
	let receiver_addr = info.sender;
	let owner_info = MessageInfo {
		sender: owner_addr,
		funds: info.funds,
	};
	let mut res = do_sell(
	deps,
	env,
	owner_info,
	curve_fn,
	receiver_addr.clone(),
	amount,
	)?;
	
	// add our custom attributes
	res.attributes.push(attr("action", "burn_from"));
	res.attributes.push(attr("by", receiver_addr));
	Ok(res)
}

fn do_sell(
mut deps: DepsMut,
env: Env,
// info.sender is the one burning tokens
info: MessageInfo,
curve_fn: CurveFn,
// receiver is the one who gains (same for execute_sell, diff for execute_sell_from)
_receiver: Addr,
amount: Uint128,
) -> Result<Response, ContractError> {
	// burn from the caller, this ensures there are tokens to cover this
	execute_burn(deps.branch(), env, info.clone(), amount)?;
	
	// Load state data
	let accounts = ACCT_CONFIG.load(deps.storage)?;
	let params = PARAM_CONFIG.load(deps.storage)?;
	let special = DEXFER_CONFIG.load(deps.storage)?;
	let mut state = CURVE_STATE.load(deps.storage)?;
	
	// calculate how many tokens to release
	let curve = curve_fn(state.clone().decimals);
	state.supply = state
	.supply
	.checked_sub(amount)
	.map_err(StdError::overflow)?;
	let new_reserve = curve.reserve(state.supply);
	let released = state
	.reserve
	.checked_sub(new_reserve)
	.map_err(StdError::overflow)?;
	state.reserve = new_reserve;
	
	// fund denom (uluna)
	let reserve_denom = state.reserve_denom.clone();
	// Messages(tx) buffer
	let mut messages = vec![];
	let mut post_tax_amt: Uint128 = Uint128::new(0);
	let mut net_released_amt: Uint128 = Uint128::new(0);
	
	if  &special.tax_exempt != &info.sender.to_string() && &special.dexfer_manager != &info.sender.to_string() {
		// Calc tax
		post_tax_amt = released
		.checked_mul(Uint128::new(params.tax_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		//This is the amount left after the total tax is collected
		net_released_amt = released
		.checked_sub(post_tax_amt).unwrap();
		
		// tax breakdown
		let tax_yield_amt = post_tax_amt
		.checked_mul(Uint128::new(params.yield_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		let tax_burn_amt = post_tax_amt
		.checked_mul(Uint128::new(params.burn_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		let tax_social_amt = post_tax_amt
		.checked_mul(Uint128::new(params.social_percent.into())).unwrap()
		.checked_div(Uint128::new(1000)).unwrap();
		
		let _tax_expense_amt = post_tax_amt
		.checked_sub(tax_yield_amt).unwrap()
		.checked_sub(tax_burn_amt).unwrap()
		.checked_sub(tax_social_amt).unwrap();
				
		//Update lifetime tax collected
		state.tax_collected += Uint128::from(post_tax_amt.clone());
		
		// tax deposit addresses
		let _tax_yield_addr  = deps.api.addr_validate(&accounts.yield_acct)?;
		let _tax_burn_addr   = deps.api.addr_validate(&accounts.burn_acct)?;
		let _tax_social_addr = deps.api.addr_validate(&accounts.social_acct)?;
		let _tax_expense_addr  = deps.api.addr_validate(&accounts.expense_acct)?;
		
			/*Do Not actually build messages, as funds are in validator
			
			// Build tx for tax collection
			messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_yield_addr.to_string(),
			amount: coins(tax_yield_amt.u128(), &reserve_denom),
			}));
			
			messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_burn_addr.to_string(),
			amount: coins(tax_burn_amt.u128(), &reserve_denom),
			}));
			
			messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_social_addr.to_string(),
			amount: coins(tax_social_amt.u128(), &reserve_denom),
			}));
			
			messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: tax_expense_addr.to_string(),
			amount: coins(tax_expense_amt.u128(), &reserve_denom),
		})); */
		
		} else {
		//tax excempt
		net_released_amt = released.clone();
	}
	
	// No uluna is returned here since all funds are in the validator
	// These messages are for documentation only
	net_released_amt = Uint128::zero();
	
	if &special.dexfer_manager != &info.sender.to_string() {
		
		let unstake_addr = deps.api.addr_validate(&accounts.unstake_acct)?;
		// Build messages(tx) to send transfer 101uluna to track this tx
		messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: unstake_addr.to_string(),
			amount: coins(101u128, &reserve_denom),
		}));
    	}
	else{

        let dexfer_addr = deps.api.addr_validate(&special.dexfer_manager)?;
        // Build messages(tx) to send transfer 102uluna to track this tx
        messages.push(CosmosMsg::Bank(BankMsg::Send {
			to_address: dexfer_addr.to_string(),
			amount: coins(102u128, &reserve_denom),
		}));
        		
	}

	// Save the state
	CURVE_STATE.save(deps.storage, &state)?;
	
	// Send Transactions
	let res = Response::new()
	.add_messages(messages)
	.add_attribute("from", info.sender)
	.add_attribute("BASE Burn: ", amount)
	.add_attribute("LUNC Unstake: ", released)
	.add_attribute("LUNC Tax: ", post_tax_amt)
	.add_attribute("Net Unstake: ", net_released_amt)
	.add_attribute("Unstake Period: ", "21 Days");
	
	Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
	// default implementation stores curve info as enum, you can do something else in a derived
	// contract and just pass in your custom curve to do_execute
	let curve_type = CURVE_TYPE.load(deps.storage)?;
	let curve_fn = curve_type.to_curve_fn();
	do_query(deps, env, msg, curve_fn)
}

/// We pull out logic here, so we can import this from another contract and set a different Curve.
/// This contacts sets a curve with an enum in InstantitateMsg and stored in state, but you may want
/// to use custom math not included - make this easily reusable
pub fn do_query(deps: Deps, _env: Env, msg: QueryMsg, curve_fn: CurveFn) -> StdResult<Binary> {
	match msg {
		// custom queries
		QueryMsg::CurveInfo {} => to_binary(&query_curve_info(deps, curve_fn)?),
		// inherited from cw20-base
		QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
		QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
		QueryMsg::Allowance { owner, spender } => {
			to_binary(&query_allowance(deps, owner, spender)?)
		}
		QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
		QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
		QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
		QueryMsg::ParamInfo {} => to_binary(&query_paraminfo(deps)?),
		QueryMsg::AcctInfo {} => to_binary(&query_acctinfo(deps)?),
		QueryMsg::DexferInfo {} => to_binary(&query_dexferinfo(deps)?),
		QueryMsg::SafetyInfo {} => to_binary(&query_safetyinfo(deps)?),
	}
}

pub fn query_curve_info(deps: Deps, curve_fn: CurveFn) -> StdResult<CurveInfoResponse> {
	let CurveState {
		reserve,
		supply,
		reserve_denom,
		decimals,
		tax_collected,
	} = CURVE_STATE.load(deps.storage)?;
	
	// This we can get from the local digits stored in instantiate
	let curve = curve_fn(decimals);
	let spot_price = curve.spot_price(supply);
	
	Ok(CurveInfoResponse {
		reserve,
		supply,
		spot_price,
		reserve_denom,
		tax_collected,
	})
}

pub fn query_paraminfo(deps: Deps) -> StdResult<ParamInfoResponse> {
	let ParamConfig {
		yield_percent,
		burn_percent,
		social_percent,
		expense_percent,
		affiliate_percent,
		tax_percent,
		presale_price,
	} = PARAM_CONFIG.load(deps.storage)?;
	
	Ok(ParamInfoResponse {
		yield_percent,
		burn_percent,
		social_percent,
		expense_percent,
		affiliate_percent,
		tax_percent,
		presale_price,
	})
}

pub fn execute_update_paramconfig(
deps: DepsMut,
_env: Env,
info: MessageInfo,
yield_percent: u32,
burn_percent: u32,
social_percent: u32,
expense_percent: u32,
affiliate_percent: u32,
tax_percent: u32,
presale_price: u32,
) -> Result<Response, ContractError> {
	
	//Only owner is authorized to proceed
	let accounts = ACCT_CONFIG.load(deps.storage)?;
	if &accounts.owner != &info.sender {
		return Err(ContractError::Unauthorized{});
	}
	
	let mut config = PARAM_CONFIG
	.may_load(deps.storage)?
	.ok_or(ContractError::Unauthorized {})?;
	config.yield_percent = yield_percent;
	config.burn_percent = burn_percent;
	config.social_percent = social_percent;
	config.expense_percent = expense_percent;
	config.affiliate_percent = affiliate_percent;
	config.tax_percent = tax_percent;
	config.presale_price = presale_price;
	// Save config back to contract storage.
	PARAM_CONFIG.save(deps.storage, &config)?;
	
	Ok(Response::default())
}

pub fn query_acctinfo(deps: Deps) -> StdResult<AcctInfoResponse> {
	let AcctConfig {
		owner,
		presale_acct,
		yield_acct,
		burn_acct,
		social_acct,
		expense_acct,
		stake_acct,
		unstake_acct,
	} = ACCT_CONFIG.load(deps.storage)?;
	
	Ok(AcctInfoResponse {
		owner,
		presale_acct,
		yield_acct,
		burn_acct,
		social_acct,
		expense_acct,
		stake_acct,
		unstake_acct,
	})
}

pub fn execute_update_acctconfig(
deps: DepsMut,
_env: Env,
info: MessageInfo,
presale_acct: String,
yield_acct: String,
burn_acct: String,
social_acct: String,
expense_acct: String,
stake_acct: String,
unstake_acct: String,
) -> Result<Response, ContractError> {
	
	//Only owner is authorized to proceed
	let accounts = ACCT_CONFIG.load(deps.storage)?;
	if &accounts.owner != &info.sender {
		return Err(ContractError::Unauthorized{});
	}
	
	let mut config = ACCT_CONFIG
	.may_load(deps.storage)?
	.ok_or(ContractError::Unauthorized {})?;
	config.presale_acct = presale_acct;
	config.yield_acct = yield_acct;
	config.burn_acct = burn_acct;
	config.social_acct = social_acct;
	config.expense_acct = expense_acct;
	config.stake_acct = stake_acct;
	config.unstake_acct = unstake_acct;
	// Save config back to contract storage.
	ACCT_CONFIG.save(deps.storage, &config)?;
	
	Ok(Response::default())
}

pub fn query_dexferinfo(deps: Deps) -> StdResult<DexferInfoResponse> {
	let DexferConfig {
		dexfer_manager,
		deposit_percent,
		tax_exempt,
		token_minter,
	} = DEXFER_CONFIG.load(deps.storage)?;
	
	Ok(DexferInfoResponse {
		dexfer_manager,
		deposit_percent,
		tax_exempt,
		token_minter,
	})
}

pub fn execute_update_dexferconfig(
deps: DepsMut,
_env: Env,
info: MessageInfo,
dexfer_manager: String,
deposit_percent: u32,
tax_exempt: String,
token_minter: String,
) -> Result<Response, ContractError> {
	
	//Only owner is authorized to proceed
	let accounts = ACCT_CONFIG.load(deps.storage)?;
	if &accounts.owner != &info.sender {
		return Err(ContractError::Unauthorized{});
	}
	
	let mut config = DEXFER_CONFIG
	.may_load(deps.storage)?
	.ok_or(ContractError::Unauthorized {})?;
	config.dexfer_manager = dexfer_manager;
	config.deposit_percent = deposit_percent;
	config.tax_exempt = tax_exempt;
	config.token_minter = token_minter;
	// Save config back to contract storage.
	DEXFER_CONFIG.save(deps.storage, &config)?;
	
	Ok(Response::default())
}

pub fn query_safetyinfo(deps: Deps) -> StdResult<SafetyInfoResponse> {
	let SafetyConfig {
		can_buy,
		can_sell,
	} = SAFETY_CONFIG.load(deps.storage)?;
	
	Ok(SafetyInfoResponse {
		can_buy,
		can_sell,
	})
}

pub fn execute_update_safetyconfig(
deps: DepsMut,
_env: Env,
info: MessageInfo,
can_buy: String,
can_sell: String,
) -> Result<Response, ContractError> {
	
	//Only owner is authorized to proceed
	let accounts = ACCT_CONFIG.load(deps.storage)?;
	if &accounts.owner != &info.sender {
		return Err(ContractError::Unauthorized{});
	}
	
	let mut config = SAFETY_CONFIG
	.may_load(deps.storage)?
	.ok_or(ContractError::Unauthorized {})?;
	config.can_buy = can_buy;
	config.can_sell = can_sell;
	// Save config back to contract storage.
	SAFETY_CONFIG.save(deps.storage, &config)?;
	
	Ok(Response::default())
}

pub fn execute_update_minter(
deps: DepsMut,
_env: Env,
info: MessageInfo,
new_minter: Option<String>,
) -> Result<Response, ContractError> {
	
	let accounts = ACCT_CONFIG.load(deps.storage)?;
	let mut config = TOKEN_INFO
	.may_load(deps.storage)?
	.ok_or(ContractError::Unauthorized {})?;
	
	let mint = config.mint.as_ref().ok_or(ContractError::Unauthorized {})?;
	//if mint.minter != info.sender {
	if &accounts.owner != &info.sender {
		return Err(ContractError::Unauthorized {});
	}
	
	let minter_data = new_minter
	.map(|new_minter| deps.api.addr_validate(&new_minter))
	.transpose()?
	.map(|minter| MinterData {
		minter,
		cap: mint.cap,
	});
	
	config.mint = minter_data;
	
	TOKEN_INFO.save(deps.storage, &config)?;
	
	Ok(Response::default()
	.add_attribute("action", "update_minter")
	.add_attribute(
	"new_minter",
	config
	.mint
	.map(|m| m.minter.into_string())
	.unwrap_or_else(|| "None".to_string()),
	))
}

pub fn must_pay(info: &MessageInfo, denom: &str) -> Result<Uint128, PaymentError> {
	let coin = one_coin(info)?;
	if coin.denom != denom {
		Err(PaymentError::MissingDenom(denom.to_string()))
		} else {
		Ok(coin.amount)
	}
}

pub fn nonpayable(info: &MessageInfo) -> Result<(), PaymentError> {
	if info.funds.is_empty() {
		Ok(())
		} else {
		Err(PaymentError::NonPayable {})
	}
}

pub fn one_coin(info: &MessageInfo) -> Result<Coin, PaymentError> {
	match info.funds.len() {
		0 => Err(PaymentError::NoFunds {}),
		1 => {
			let coin = &info.funds[0];
			if coin.amount.is_zero() {
				Err(PaymentError::NoFunds {})
				} else {
				Ok(coin.clone())
			}
		}
		_ => Err(PaymentError::MultipleDenoms {}),
	}
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PaymentError {
	#[error("Must send reserve token '{0}'")]
	MissingDenom(String),
	
	#[error("Received unsupported denom '{0}'")]
	ExtraDenom(String),
	
	#[error("Sent more than one denomination")]
	MultipleDenoms {},
	
	#[error("No funds sent")]
	NoFunds {},
	
	#[error("This message does no accept funds")]
	NonPayable {},
}
