# Rust UniswapV2 data extractor

## Running

`cargo run --release -- --rpc-url https://polygon-mainnet.example-rpc.com`

`--rpc-url` — param is required, while `--from-block` and `--to-block` has a default vaules

## How it works

### High level
- The program scans blockchain blocks and filters them based on the Swap event produced by UniswapV2Pair contracts within a specified block range, with the default range set from `52,900,000` to `53,000,000`.
- In the second step, it loads and updates the affected pools with corresponding tokens' metadata.
- Following that, it retrieves the reserves of the touched pools by calling the getReserves function from a UniswapV2Pair contract.
- As the fourth step, it loads the balances of the token vaults for each UniswapV2Pair and stores them in a separate map.
- `strange_reserves` sets to `true` if there are differences in balances. Saves processing result to an `output.json` file.

### Low level
- Request all tx logs in a specific block range, collect transactions that matches with UniswapV2Pair `Swap` event topic and extract pool addresses that fire this event
- After touched pools extraction it loads and initialize all necessary pool data
- Then request pool reservs for a each block in a block range
- At the final step when all data is synced program processes all pools to find all inequalities between token vaul balances and pool reserves by marking them as a `strange_reserves` in a final output JSON

### Request token metadata contract, deployed on [polygon mainnet](https://polygonscan.com/address/0x9ae10196dfe6a01ea76e89d98e601b93e48807df)

```
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IERC20 {
    function name() external view returns (string memory);

    function symbol() external view returns (string memory);

    function decimals() external view returns (uint8);

    function totalSupply() external view returns (uint256);
}

contract Request {
    function getTokenInfo(
        address targetToken
    )
        external
        view
        returns (
            string memory name,
            string memory symbol,
            uint8 decimals,
            uint256 totalSupply
        )
    {
        IERC20 t = IERC20(targetToken);

        name = t.name();
        symbol = t.symbol();
        decimals = t.decimals();
        totalSupply = t.totalSupply();
    }
}

```
