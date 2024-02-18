use ethers::signers::{LocalWallet, Signer};
use ethers_core::{rand::thread_rng, types::H160};

pub fn create_new_wallet() -> (LocalWallet, H160) {
    let wallet = LocalWallet::new(&mut thread_rng());
    let address = wallet.address();
    (wallet, address)
}

pub fn block_number_chunks(from_block: u64, to_block: u64) -> Vec<(u64, u64)> {
    let chunk_size = 5000;
    let mut block_range = Vec::new();
    let mut current_block = from_block;

    while current_block <= to_block {
        let end_idx = (current_block + chunk_size - 1).min(to_block);
        block_range.push((current_block, end_idx));
        current_block += chunk_size;
    }

    block_range
}
