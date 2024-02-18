use std::{collections::HashMap, str::FromStr, sync::Arc};

use amms::amm::uniswap_v2::UniswapV2Pool;
use anyhow::Result;
use ethers_contract::BaseContract;
use ethers_core::{
    abi::parse_abi,
    types::{Filter, Log, TransactionRequest, H160, H256, U256},
};
use ethers_providers::{spoof, Http, Middleware, Provider, RawCall};

use crate::{
    abi::{IErc20, IUniswapV2Pair, TOKEN_INFO_REQUEST_BYTES},
    types::{PoolReserves, PoolToken, PoolTokenBalances, Token, TouchedPool},
    utils::{block_number_chunks, create_new_wallet},
};

pub const TOKEN_INFO_REQUEST_CONTRACT: &str = "0x9ae10196dfe6a01ea76e89d98e601b93e48807df";

pub async fn get_touched_pools_by_block_range(
    provider: Arc<Provider<Http>>,
    from_block: u64,
    to_block: u64,
) -> Result<HashMap<H160, TouchedPool>> {
    let swap_event = "Swap(address,uint256,uint256,uint256,uint256,address)";
    let abi = parse_abi(&[&format!("event {}", swap_event)]).unwrap();
    let swap_signature = abi.event("Swap").unwrap().signature();

    let block_range = block_number_chunks(from_block, to_block);
    let mut touched_pools: HashMap<H160, TouchedPool> = HashMap::new();

    let mut get_log_futs = Vec::new();
    for range in block_range.clone() {
        let task = tokio::task::spawn(get_logs(
            provider.clone(),
            swap_event,
            range.0,
            range.1,
            swap_signature,
        ));
        get_log_futs.push(task);
    }

    let log_results = futures::future::join_all(get_log_futs).await;

    let mut create_touched_pool_futs = Vec::new();

    for result in log_results {
        match result {
            Ok(response) => match response {
                Ok(logs_response) => {
                    for log in logs_response.clone() {
                        let action_block = log.block_number.unwrap().as_u64();
                        let task = tokio::task::spawn(create_uniswap_v2_pool(
                            provider.clone(),
                            log.address,
                            action_block,
                        ));
                        create_touched_pool_futs.push(task);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    let touched_pool_results = futures::future::join_all(create_touched_pool_futs).await;

    for result in touched_pool_results {
        match result {
            Ok(response) => match response {
                Ok(touched_pool) => {
                    touched_pools.insert(touched_pool.pool.address, touched_pool);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(touched_pools)
}

pub async fn get_logs(
    provider: Arc<Provider<Http>>,
    event: &str,
    from_block: u64,
    to_block: u64,
    signature: H256,
) -> Result<Vec<Log>> {
    let event_filter = Filter::new()
        .from_block(from_block)
        .to_block(to_block)
        .event(event)
        .topic0(signature);
    let response = provider.get_logs(&event_filter).await?;

    Ok(response)
}

// Returns mutated touched_pools filled with pool tokens metadata and return pools_tokens map
pub async fn load_pools_tokens(
    provider: Arc<Provider<Http>>,
    mut touched_pools: HashMap<H160, TouchedPool>,
) -> Result<(HashMap<H160, TouchedPool>, HashMap<H160, PoolToken>)> {
    let mut pools_tokens: HashMap<H160, PoolToken> = HashMap::new();
    let mut pool_tokens_futs = Vec::new();

    for touched_pool in touched_pools.values() {
        let token_addresses = vec![touched_pool.pool.token_a, touched_pool.pool.token_b];
        for token_address in token_addresses {
            let task = tokio::task::spawn(get_token_metadata(
                provider.clone(),
                touched_pool.pool.address,
                token_address,
            ));
            pool_tokens_futs.push(task)
        }
    }

    let pool_tokens_results = futures::future::join_all(pool_tokens_futs).await;

    for result in pool_tokens_results {
        match result {
            Ok(response) => match response {
                Ok(pool_token) => {
                    // in place update
                    let updated_pool = &mut touched_pools
                        .get_mut(&pool_token.pool_address)
                        .unwrap()
                        .pool;
                    updated_pool.token_a = pool_token.token.address;
                    updated_pool.token_b = pool_token.token.address;
                    pools_tokens.insert(pool_token.token.address, pool_token);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok((touched_pools, pools_tokens))
}

pub async fn create_uniswap_v2_pool(
    provider: Arc<Provider<Http>>,
    address: H160,
    block_number: u64,
) -> Result<TouchedPool> {
    let pool = UniswapV2Pool::new_from_address(address, 0, provider).await?;
    let touched_pool = TouchedPool {
        pool,
        action_block: block_number,
    };

    Ok(touched_pool)
}

pub async fn load_pool_reserves_by_block(
    provider: Arc<Provider<Http>>,
    mut touched_pools: HashMap<H160, TouchedPool>,
) -> Result<HashMap<H160, TouchedPool>> {
    let mut pool_reserves_futs = Vec::new();

    for touched_pool in touched_pools.values() {
        let task = tokio::task::spawn(get_pool_reserves(
            provider.clone(),
            touched_pool.pool.clone(),
        ));
        pool_reserves_futs.push(task)
    }

    let pool_tokens_results = futures::future::join_all(pool_reserves_futs).await;

    for result in pool_tokens_results {
        match result {
            Ok(response) => match response {
                Ok(pool_reserves) => {
                    let pool = &mut touched_pools
                        .get_mut(&pool_reserves.pool_address)
                        .unwrap()
                        .pool;
                    pool.reserve_0 = pool_reserves.token_a_reserve;
                    pool.reserve_1 = pool_reserves.token_b_reserve;
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(touched_pools)
}

pub async fn get_pool_reserves(
    provider: Arc<Provider<Http>>,
    pool: UniswapV2Pool,
) -> Result<PoolReserves> {
    let v2_pair_contract = IUniswapV2Pair::new(pool.address, provider.clone());
    let (reserve_a, reserve_b, _) = v2_pair_contract.get_reserves().call().await?;

    Ok(PoolReserves {
        pool_address: pool.address,
        token_a_reserve: reserve_a,
        token_b_reserve: reserve_b,
    })
}

pub async fn load_pools_tokens_balances(
    provider: Arc<Provider<Http>>,
    touched_pools: Vec<TouchedPool>,
) -> Result<HashMap<H160, PoolTokenBalances>> {
    let mut pool_token_balances: HashMap<H160, PoolTokenBalances> = HashMap::new();

    let mut pool_reserves_futs = Vec::new();

    for touched_pool in touched_pools {
        let task = tokio::task::spawn(get_pool_token_balances(provider.clone(), touched_pool));
        pool_reserves_futs.push(task)
    }

    let pool_tokens_results = futures::future::join_all(pool_reserves_futs).await;

    for result in pool_tokens_results {
        match result {
            Ok(response) => match response {
                Ok(pool_token_balance_results) => {
                    for (pool_address, pool_token_balance) in pool_token_balance_results {
                        pool_token_balances.insert(pool_address, pool_token_balance);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(pool_token_balances)
}

pub async fn get_pool_token_balances(
    provider: Arc<Provider<Http>>,
    touched_pool: TouchedPool,
) -> Result<HashMap<H160, PoolTokenBalances>> {
    let mut pool_token_balances: HashMap<H160, PoolTokenBalances> = HashMap::new();

    let token_a_balance = IErc20::new(touched_pool.pool.token_a, provider.clone())
        .balance_of(touched_pool.pool.address)
        .block(touched_pool.action_block)
        .call()
        .await?
        .as_u128();

    let token_b_balance = IErc20::new(touched_pool.pool.token_b, provider.clone())
        .balance_of(touched_pool.pool.address)
        .block(touched_pool.action_block)
        .call()
        .await?
        .as_u128();

    let token_balances = PoolTokenBalances {
        pool_address: touched_pool.pool.address,
        token_a_balance,
        token_b_balance,
    };

    pool_token_balances.insert(touched_pool.pool.address, token_balances);

    Ok(pool_token_balances)
}

pub async fn get_token_metadata(
    provider: Arc<Provider<Http>>,
    pool_address: H160,
    token_address: H160,
) -> Result<PoolToken> {
    let owner = create_new_wallet().1;

    let mut state = spoof::state();

    let request_address = H160::from_str(TOKEN_INFO_REQUEST_CONTRACT).unwrap();
    state
        .account(request_address)
        .code((*TOKEN_INFO_REQUEST_BYTES).clone());

    let request_abi = BaseContract::from(parse_abi(&[
        "function getTokenInfo(address) external returns (string,string,uint8,uint256)",
    ])?);
    let calldata = request_abi.encode("getTokenInfo", token_address)?;

    let tx = TransactionRequest::default()
        .from(owner)
        .to(request_address)
        .value(U256::zero())
        .data(calldata.0)
        .nonce(U256::zero())
        .chain_id(137)
        .into();

    let result = provider.call_raw(&tx).state(&state).await.unwrap();

    let out: (String, String, u8, U256) = request_abi.decode_output("getTokenInfo", result)?;
    let pool_token = PoolToken {
        pool_address,
        token: Token {
            address: token_address,
            name: out.0,
            symbol: out.1,
            decimals: out.2,
        },
    };

    Ok(pool_token)
}
