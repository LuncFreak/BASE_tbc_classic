// This contract is forked from on github.com/CosmWasm/cw-tokens/tree/main/contracts/cw20-bonding
// Changes were made to make compatible with Terra Classic and add Sigmoid curve

use cosmwasm_schema::cw_serde;
use integer_cbrt::IntegerCubeRoot;
use integer_sqrt::IntegerSquareRoot;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;
use cosmwasm_std::{Decimal as StdDecimal, Uint128};

struct Point {
    supply: Uint128,
    spot_price: Uint128,
    reserve: Uint128,
}

const SIGMOID_CURVE: [Point; 26] = [
    Point { supply: Uint128::new(0), spot_price: Uint128::new(0), reserve: Uint128::new(0 ) },
    Point { supply: Uint128::new(10000000000000), spot_price: Uint128::new(100000), reserve: Uint128::new(500000000000 ) },
    Point { supply: Uint128::new(19999000000000), spot_price: Uint128::new(200000), reserve: Uint128::new(1999850000000 ) },
    Point { supply: Uint128::new(20000000000000), spot_price: Uint128::new(80000000), reserve: Uint128::new(2039950000000 ) },
    Point { supply: Uint128::new(25000000000000), spot_price: Uint128::new(120000000), reserve: Uint128::new(502039950000000 ) },
    Point { supply: Uint128::new(30000000000000), spot_price: Uint128::new(180000000), reserve: Uint128::new(1252039950000000 ) },
    Point { supply: Uint128::new(32000000000000), spot_price: Uint128::new(280000000), reserve: Uint128::new(1712039950000000 ) },
    Point { supply: Uint128::new(34000000000000), spot_price: Uint128::new(378000000), reserve: Uint128::new(2370039950000000 ) },
    Point { supply: Uint128::new(36000000000000), spot_price: Uint128::new(504000000), reserve: Uint128::new(3252039950000000 ) },
    Point { supply: Uint128::new(38000000000000), spot_price: Uint128::new(651000000), reserve: Uint128::new(4407039950000000 ) },
    Point { supply: Uint128::new(40000000000000), spot_price: Uint128::new(1000000000), reserve: Uint128::new(6058039950000000 ) },
    Point { supply: Uint128::new(42000000000000), spot_price: Uint128::new(1500000000), reserve: Uint128::new(8558039950000000 ) },
    Point { supply: Uint128::new(45000000000000), spot_price: Uint128::new(2500000000), reserve: Uint128::new(14558039950000000 ) },
    Point { supply: Uint128::new(47500000000000), spot_price: Uint128::new(4000000000), reserve: Uint128::new(22683039950000000 ) },
    Point { supply: Uint128::new(58000000000000), spot_price: Uint128::new(15000000000), reserve: Uint128::new(122433039950000000 ) },
    Point { supply: Uint128::new(61000000000000), spot_price: Uint128::new(18000000000), reserve: Uint128::new(171933039950000000 ) },
    Point { supply: Uint128::new(65000000000000), spot_price: Uint128::new(22000000000), reserve: Uint128::new(251933039950000000 ) },
    Point { supply: Uint128::new(69000000000000), spot_price: Uint128::new(25000000000), reserve: Uint128::new(345933039950000000 ) },
    Point { supply: Uint128::new(74000000000000), spot_price: Uint128::new(27500000000), reserve: Uint128::new(477183039950000000 ) },
    Point { supply: Uint128::new(80000000000000), spot_price: Uint128::new(29500000000), reserve: Uint128::new(648183039950000000 ) },
    Point { supply: Uint128::new(87000000000000), spot_price: Uint128::new(31000000000), reserve: Uint128::new(859933039950000000 ) },
    Point { supply: Uint128::new(95000000000000), spot_price: Uint128::new(32000000000), reserve: Uint128::new(1111933039950000000 ) },
    Point { supply: Uint128::new(100000000000000), spot_price: Uint128::new(32500000000), reserve: Uint128::new(1273183039950000000 ) },
    Point { supply: Uint128::new(110000000000000), spot_price: Uint128::new(33000000000), reserve: Uint128::new(1600683039950000000 ) },
    Point { supply: Uint128::new(150000000000000), spot_price: Uint128::new(33500000000), reserve: Uint128::new(2930683039950000000 ) },
    Point { supply: Uint128::new(200000000000000), spot_price: Uint128::new(34000000000), reserve: Uint128::new(4618183039950000000 ) },
];

/// This defines the curves we are using.
pub trait Curve {
    /// Returns the spot price given the supply.
    /// `f(x)` from the README
    fn spot_price(&self, supply: Uint128) -> Uint128;
	
    /// Returns the total price paid up to purchase supply tokens (integral)
    /// `F(x)` from the README
    fn reserve(&self, supply: Uint128) -> Uint128;
	
    /// Inverse of reserve. Returns how many tokens would be issued
    /// with a total paid amount of reserve.
    /// `F^-1(x)` from the README
    fn supply(&self, reserve: Uint128) -> Uint128;
}

/// decimal returns an object = num * 10 ^ -scale
/// We use this function in contract.rs rather than call the crate constructor
/// itself, in case we want to swap out the implementation, we can do it only in this file.
pub fn decimal<T: Into<u128>>(num: T, scale: u32) -> Decimal {
    Decimal::from_i128_with_scale(num.into() as i128, scale)
}

/// StdDecimal stores as a u128 with 18 decimal points of precision
fn decimal_to_std(x: Decimal) -> StdDecimal {
    // this seems straight-forward (if inefficient), converting via string representation
    // TODO: execute errors better? Result?
    StdDecimal::from_str(&x.to_string()).unwrap()
	
    // // maybe a better approach doing math, not sure about rounding
    //
    // // try to preserve decimal points, max 9
    // let digits = min(x.scale(), 9);
    // let multiplier = 10u128.pow(digits);
    //
    // // we multiply up before we round off to u128,
    // // let StdDecimal do its best to keep these decimal places
    // let nominator = (x * decimal(multiplier, 0)).to_u128().unwrap();
    // StdDecimal::from_ratio(nominator, multiplier)
}

/// StdDecimal stores as a u128 with 18 decimal points of precision
fn decimal_to_uint128(x: Decimal) -> Uint128 {
	let factor = decimal(10u128.pow(6), 0);
	let out = x * factor;
	out.floor().to_u128().unwrap().into()
}


//////////////////////////////////////////////////////////////////////////////////////////////////////
/// Constant: spot price is always a constant value
pub struct Constant {
    pub value: Decimal,
    pub normalize: DecimalPlaces,
}

impl Constant {
    pub fn new(value: Decimal, normalize: DecimalPlaces) -> Self {
        Self { value, normalize }
	}
}

impl Curve for Constant {
    // we need to normalize value with the reserve decimal places
    // (eg 0.1 value would return 100_000 if reserve was uatom)
    fn spot_price(&self, _supply: Uint128) -> Uint128 {
        // f(x) = self.value
        decimal_to_uint128(self.value)
	}
	
    /// Returns total number of reserve tokens needed to purchase a given number of supply tokens.
    /// Note that both need to be normalized.
    fn reserve(&self, supply: Uint128) -> Uint128 {
        // f(x) = supply * self.value
        let reserve = self.normalize.from_supply(supply) * self.value;
        self.normalize.clone().to_reserve(reserve)
	}
	
    fn supply(&self, reserve: Uint128) -> Uint128 {
        // f(x) = reserve / self.value
        let supply = self.normalize.from_reserve(reserve) / self.value;
        self.normalize.clone().to_supply(supply)
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////
/// Linear: spot_price is slope * supply
pub struct Linear {
    pub slope: Decimal,
    pub normalize: DecimalPlaces,
}

impl Linear {
    pub fn new(slope: Decimal, normalize: DecimalPlaces) -> Self {
        Self { slope, normalize }
	}
}

impl Curve for Linear {
    fn spot_price(&self, supply: Uint128) -> Uint128 {
        // f(x) = supply * self.value
        let out = self.normalize.from_supply(supply) * self.slope;
        decimal_to_uint128(out)
	}
	
    fn reserve(&self, supply: Uint128) -> Uint128 {
        // f(x) = self.slope * supply * supply / 2
        let normalized = self.normalize.from_supply(supply);
        let square = normalized * normalized;
        // Note: multiplying by 0.5 is much faster than dividing by 2
        let reserve = square * self.slope * Decimal::new(5, 1);
        self.normalize.clone().to_reserve(reserve)
	}
	
    fn supply(&self, reserve: Uint128) -> Uint128 {
        // f(x) = (2 * reserve / self.slope) ^ 0.5
        // note: use addition here to optimize 2* operation
        let square = self.normalize.from_reserve(reserve + reserve) / self.slope;
        let supply = square_root(square);
        self.normalize.clone().to_supply(supply)
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////
/// SquareRoot: spot_price is slope * (supply)^0.5
pub struct SquareRoot {
    pub slope: Decimal,
    pub normalize: DecimalPlaces,
}

impl SquareRoot {
    pub fn new(slope: Decimal, normalize: DecimalPlaces) -> Self {
        Self { slope, normalize }
	}
}

impl Curve for SquareRoot {
    fn spot_price(&self, supply: Uint128) -> Uint128 {
        // f(x) = self.slope * supply^0.5
        let square = self.normalize.from_supply(supply);
        let root = square_root(square);
        decimal_to_uint128(root * self.slope)
	}
	
    fn reserve(&self, supply: Uint128) -> Uint128 {
        // f(x) = self.slope * supply * supply^0.5 / 1.5
        let normalized = self.normalize.from_supply(supply);
        let root = square_root(normalized);
        let reserve = self.slope * normalized * root / Decimal::new(15, 1);
        self.normalize.clone().to_reserve(reserve)
	}
	
    fn supply(&self, reserve: Uint128) -> Uint128 {
        // f(x) = (1.5 * reserve / self.slope) ^ (2/3)
        let base = self.normalize.from_reserve(reserve) * Decimal::new(15, 1) / self.slope;
        let squared = base * base;
        let supply = cube_root(squared);
        self.normalize.clone().to_supply(supply)
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////
/// Squared: spot_price is slope * (supply)^2
pub struct Squared {
    pub slope: Decimal,
    pub normalize: DecimalPlaces,
}

impl Squared {
    pub fn new(slope: Decimal, normalize: DecimalPlaces) -> Self {
        Self { slope, normalize }
	}
}

//Squared Curve: y=k*x^2
impl Curve for Squared {
    fn spot_price(&self, supply: Uint128) -> Uint128 {
        // f(x) = self.slope * supply^2
        let normalized = self.normalize.from_supply(supply);
        let raised = normalized * normalized;
        decimal_to_uint128(self.slope * raised)
	}
	
    fn reserve(&self, supply: Uint128) -> Uint128 {
        // F(x) = (self.slope * supply^3) / 3
        let normalized = self.normalize.from_supply(supply);
        let raised = normalized * normalized * normalized;
        let reserve = (self.slope * raised) / Decimal::new(30, 1);
        self.normalize.clone().to_reserve(reserve)
	}
	
    fn supply(&self, reserve: Uint128) -> Uint128 {
        // F^-1(x) = (3.0 * reserve / self.slope) ^ (1/3)
        let base = self.normalize.from_reserve(reserve) * Decimal::new(30, 1) / self.slope;
        let supply = cube_root(base);
        self.normalize.clone().to_supply(supply)
	}
}

//////////////////////////////////////////////////////////////////////////////////////////////////////
/// Sigmoid Function: SIGMOID_CURVE defines spot_price, supply and reserve in array above
pub struct Sigmoid {
    pub slope: Decimal,
    pub normalize: DecimalPlaces,
}

impl Sigmoid {
    pub fn new(slope: Decimal, normalize: DecimalPlaces) -> Self {
        Self { slope, normalize }
	}
}

//Sigmoid function Curve; SIGMOID_CURVE defines spot_price, supply and reserve in array above
impl Curve for Sigmoid {
	
    fn spot_price(&self, supply: Uint128) -> Uint128 {
        let mut index: usize = 0;
    
        if supply < SIGMOID_CURVE[0].supply || supply > SIGMOID_CURVE[SIGMOID_CURVE.len() - 1].supply {
            return Uint128::new(u128::MAX);
        }
        else if supply == Uint128::zero(){
            return decimal_to_uint128(Decimal::new(1, 6)); 
        }
    
        for i in 0..(SIGMOID_CURVE.len() - 1) {
            
            if supply > SIGMOID_CURVE[i].supply && supply <= SIGMOID_CURVE[i+1].supply {
                index = i;
                break;
            }
        }

        let delta_y = SIGMOID_CURVE[index + 1].spot_price - SIGMOID_CURVE[index].spot_price;
        let delta_x = SIGMOID_CURVE[index + 1].supply - SIGMOID_CURVE[index].supply;
        let slope = (delta_y
            .checked_mul(Uint128::new(1_000_000)).unwrap()
            .checked_div(delta_x)).unwrap();

        let virtual_supply = supply
        .checked_sub(SIGMOID_CURVE[index].supply).unwrap();

        let mut new_price = virtual_supply
            .checked_mul(slope).unwrap()
            .checked_div(Uint128::new(1_000_000)).unwrap();

            new_price = new_price.clone()
                .checked_add(SIGMOID_CURVE[index].spot_price).unwrap();
   
        let spot_price = new_price;
 
    spot_price
    
    }

fn reserve(&self, supply: Uint128) -> Uint128 { 
    let mut index: usize = 0;
    
    if supply == Uint128::zero() || supply < SIGMOID_CURVE[0].supply{
        return Uint128::zero(); 
    }

    else if supply > SIGMOID_CURVE[SIGMOID_CURVE.len() - 1].supply {
        return SIGMOID_CURVE[SIGMOID_CURVE.len() - 1].reserve;
    }

    for i in 0..(SIGMOID_CURVE.len() - 1) {
        
        if supply > SIGMOID_CURVE[i].supply && supply <= SIGMOID_CURVE[i+1].supply {
            index = i;
            break;
        }
    }

        //Shift to zero
        let virtual_supply = supply 
                        .checked_sub(SIGMOID_CURVE[index].supply).unwrap();

        /*https://www.wolframalpha.com/input?i=R+%3D+L*S+%2B+.5*%28U-L%29S+%3B++++solve+for+R*/            
        let virtual_reserve = virtual_supply.clone()
                        .checked_mul(
                        SIGMOID_CURVE[index].spot_price.checked_add(SIGMOID_CURVE[index+1].spot_price).unwrap()).unwrap()
                        .checked_div(Uint128::new(2_000_000)).unwrap();
                        
        //Shift back 
        let reserve = virtual_reserve.clone()
                        .checked_add(SIGMOID_CURVE[index].reserve).unwrap();

        reserve

    }

    fn supply(&self, reserve: Uint128) -> Uint128 {
        let mut index: usize = 0;
    
        if reserve == Uint128::zero() || reserve > SIGMOID_CURVE[SIGMOID_CURVE.len() - 1].reserve {
            return Uint128::zero();
        }
    
        for i in 0..(SIGMOID_CURVE.len() - 1) {
            
            if reserve > SIGMOID_CURVE[i].reserve && reserve <= SIGMOID_CURVE[i+1].reserve {
                index = i;
                break;
            }
        }
    
        //Shift to zero
        let virtual_reserve = reserve 
                        .checked_sub(SIGMOID_CURVE[index].reserve).unwrap();

        /*https://www.wolframalpha.com/input?i=R+%3D+L*S+%2B+.5*%28U-L%29S+%3B++++solve+for+S*/            
        let mut virtual_supply = virtual_reserve.clone()
                        .checked_mul(Uint128::new(2u128)).unwrap()
                        .checked_mul(Uint128::new(1_000_000)).unwrap();
                        
        virtual_supply = virtual_supply.clone()
                        .checked_div(
                        SIGMOID_CURVE[index].spot_price.checked_add(SIGMOID_CURVE[index+1].spot_price).unwrap()
                        ).unwrap();
        //Shift back 
        let supply = virtual_supply.clone()
                        .checked_add(SIGMOID_CURVE[index].supply).unwrap();

        supply

    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////


// we multiply by 10^12, turn to int, take square root, then divide by 10^6 as we convert back to decimal
fn square_root(square: Decimal) -> Decimal {
    // must be even
    // TODO: this can overflow easily at 18... what is a good value?
    const EXTRA_DIGITS: u32 = 12;
    let multiplier = 10u128.saturating_pow(EXTRA_DIGITS);
	
    // multiply by 10^18 and turn to u128
    let extended = square * decimal(multiplier, 0);
    let extended = extended.floor().to_u128().unwrap();
	
    // take square root, and build a decimal again
    let root = extended.integer_sqrt();
    decimal(root, EXTRA_DIGITS / 2)
}

// we multiply by 10^9, turn to int, take cube root, then divide by 10^3 as we convert back to decimal
fn cube_root(cube: Decimal) -> Decimal {
    // must be multiple of 3
    // TODO: what is a good value?
    const EXTRA_DIGITS: u32 = 9;
    let multiplier = 10u128.saturating_pow(EXTRA_DIGITS);
	
    // multiply out and turn to u128
    let extended = cube * decimal(multiplier, 0);
    let extended = extended.floor().to_u128().unwrap();
	
    // take cube root, and build a decimal again
    let root = extended.integer_cbrt();
    decimal(root, EXTRA_DIGITS / 3)
}

/// DecimalPlaces should be passed into curve constructors
#[cw_serde]
pub struct DecimalPlaces {
    /// Number of decimal places for the supply token (this is what was passed in cw20-base instantiate
    pub supply: u32,
    /// Number of decimal places for the reserve token (eg. 6 for uatom, 9 for nstep, 18 for wei)
    pub reserve: u32,
}

impl DecimalPlaces {
    pub fn new(supply: u8, reserve: u8) -> Self {
        DecimalPlaces {
            supply: supply as u32,
            reserve: reserve as u32,
		}
	}
	
    pub fn to_reserve(self, reserve: Decimal) -> Uint128 {
        let factor = decimal(10u128.pow(self.reserve), 0);
        let out = reserve * factor;
        // TODO: execute overflow better? Result?
        out.floor().to_u128().unwrap().into()
	}
	
    pub fn to_supply(self, supply: Decimal) -> Uint128 {
        let factor = decimal(10u128.pow(self.supply), 0);
        let out = supply * factor;
        // TODO: execute overflow better? Result?
        out.floor().to_u128().unwrap().into()
	}
	
    pub fn from_supply(&self, supply: Uint128) -> Decimal {
        decimal(supply, self.supply)
	}
	
    pub fn from_reserve(&self, reserve: Uint128) -> Decimal {
        decimal(reserve, self.reserve)
	}
}
