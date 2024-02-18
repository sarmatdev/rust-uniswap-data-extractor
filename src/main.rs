use std::{collections::HashMap, fs::File, io::Write, sync::Arc};

use anyhow::Result;
use clap::Parser;
use ethers_core::types::H160;
use ethers_providers::{Http, Provider};
use rayon::prelude::*;
use rust_uniswap_data_extractor::{
    requests::{
        get_touched_pools_by_block_range, load_pool_reserves_by_block, load_pools_tokens,
        load_pools_tokens_balances,
    },
    types::{PoolToken, PoolTokenBalances, TouchedPool, TouchedPoolJson},
};
use tracing::{error, info, Level};

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[clap(long, env)]
    rpc_url: String,

    #[clap(long, default_value_t = 52900000)]
    from_block: u64,

    #[clap(long, default_value_t = 53000000)]
    to_block: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let args: Args = Args::parse();

    let provider = Provider::<Http>::try_from(args.rpc_url)?;
    let provider = Arc::new(provider);

    let from_block = args.from_block;
    let to_block = args.to_block;

    let touched_pools =
        get_touched_pools_by_block_range(provider.clone(), from_block, to_block).await?;

    let (touched_pools, pools_tokens) = load_pools_tokens(provider.clone(), touched_pools).await?;

    tokio::select! {
        _ = async {
            load_pool_reserves_by_block(provider.clone(), touched_pools.clone()).await.unwrap();
        } => {}
        _ = async {
            let touched_pools_token_balances =
                load_pools_tokens_balances(provider.clone(), touched_pools.values().cloned().collect())
                    .await.unwrap();
                info!("touched_pools: {:?}", touched_pools.len());
                info!("touched_pools_token_balances: {:?}", touched_pools_token_balances.len());
                info!("pools_tokens: {:?}", pools_tokens.len());
            let processed_pools = process_pools_comparison(&touched_pools, touched_pools_token_balances, pools_tokens);
            info!("processed_pools: {}", processed_pools.len());

            let json_data = serde_json::to_string_pretty(&processed_pools);
            match json_data {
                Ok(json_string) => {
                    if let Err(err) = File::create("output.json")
                        .and_then(|mut file| file.write_all(json_string.as_bytes()))
                    {
                        error!("Error writing to file: {}", err);
                    }
                }
                Err(err) => error!("Error serializing to JSON: {}", err),
            }

            info!("Done processing pools");
        } => {}
    }

    Ok(())
}

pub fn process_pools_comparison(
    touched_pools: &HashMap<H160, TouchedPool>,
    touched_pools_token_balances: HashMap<H160, PoolTokenBalances>,
    pools_tokens: HashMap<H160, PoolToken>,
) -> Vec<TouchedPoolJson> {
    touched_pools
        .par_iter()
        .filter_map(|(pool_address, touched_pool)| {
            let pool = &touched_pool.pool;
            let pool_reserves: (u128, u128) = (pool.reserve_0, pool.reserve_1);

            if let Some(pool_token_balances) = touched_pools_token_balances.get(pool_address) {
                let pool_token_balances: (u128, u128) = (
                    pool_token_balances.token_a_balance,
                    pool_token_balances.token_b_balance,
                );

                let strange_reserves = pool_reserves != pool_token_balances;

                let token_a_info = pools_tokens
                    .get(&pool.token_a)
                    .map(|pt| pt.token.symbol.clone())
                    .unwrap_or_default();
                let token_b_info = pools_tokens
                    .get(&pool.token_b)
                    .map(|pt| pt.token.symbol.clone())
                    .unwrap_or_default();

                let comparison_result = TouchedPoolJson {
                    pair_address: pool_address.to_string(),
                    token0_address: pool.token_a.to_string(),
                    token1_address: pool.token_b.to_string(),
                    token0_symbol: token_a_info,
                    token1_symbol: token_b_info,
                    token0_reserve_get_reserves: pool_reserves.0,
                    token1_reserve_get_reserves: pool_reserves.1,
                    token0_reserve_balance_of: pool_token_balances.0,
                    token1_reserve_balance_of: pool_token_balances.1,
                    block_num: touched_pool.action_block,
                    strange_reserves,
                };
                Some(comparison_result)
            } else {
                None
            }
        })
        .collect()
}
