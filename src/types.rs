use amms::amm::uniswap_v2::UniswapV2Pool;
use ethers_core::types::H160;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TouchedPoolJson {
    pub pair_address: String,
    pub token0_address: String,
    pub token1_address: String,
    pub token0_symbol: String,
    pub token1_symbol: String,
    pub token0_reserve_get_reserves: u128,
    pub token1_reserve_get_reserves: u128,
    pub token0_reserve_balance_of: u128,
    pub token1_reserve_balance_of: u128,
    pub block_num: u64,
    pub strange_reserves: bool,
}

#[derive(Debug, Clone)]
pub struct TouchedPool {
    pub pool: UniswapV2Pool,
    pub action_block: u64,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub address: H160,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone)]
pub struct PoolToken {
    pub pool_address: H160,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct PoolReserves {
    pub pool_address: H160,
    pub token_a_reserve: u128,
    pub token_b_reserve: u128,
}

#[derive(Debug, Clone)]
pub struct PoolTokenBalances {
    pub pool_address: H160,
    pub token_a_balance: u128,
    pub token_b_balance: u128,
}
