use alloy_sol_types::{sol, SolCall};
// FIX: We remove direct alloy_primitives imports to avoid conflicts.
use revm::{
    db::CacheDB,
    primitives::{ExecutionResult, Output, TransactTo, Env, Address, U256, Bytes},
    DatabaseRef, Evm,
};
use eyre::{Result, eyre};
use std::fmt::Debug;

sol! {
    #[derive(Debug)]
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] memory path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

pub struct SimulationEngine<DB> {
    pub db: DB,
}

// FIX: Removed `+ Clone` bound. We don't need to clone the DB anymore.
impl<DB: DatabaseRef> SimulationEngine<DB> 
where <DB as DatabaseRef>::Error: Debug 
{
    pub fn new(db: DB) -> Self {
        Self { db }
    }

    pub fn simulate_swap(
        &mut self,
        router: Address,
        caller: Address,
        amount_in: U256,
        path: Vec<Address>,
    ) -> Result<U256> {
        let mut env = Env::default();
        env.tx.caller = caller;
        env.tx.transact_to = TransactTo::Call(router);
        env.tx.gas_limit = 500_000;
        env.tx.value = U256::ZERO;

        let amount_in_alloy = alloy_primitives::U256::from_limbs(amount_in.into_limbs());
        
        let path_alloy: Vec<alloy_primitives::Address> = path.iter()
            .map(|a| alloy_primitives::Address::from(a.0 .0))
            .collect();
            
        let caller_alloy = alloy_primitives::Address::from(caller.0 .0);

        let swap_call = swapExactTokensForTokensCall {
            amountIn: amount_in_alloy,
            amountOutMin: alloy_primitives::U256::ZERO,
            path: path_alloy,
            to: caller_alloy,
            deadline: alloy_primitives::U256::MAX,
        };
        
        env.tx.data = Bytes::copy_from_slice(&swap_call.abi_encode());

        // FIX: Use `&self.db` (Reference) instead of `self.db.clone()` (Copy).
        // This allows us to use non-cloneable databases like Reth's StateProvider.
        let mut cache_db = CacheDB::new(&self.db);
        
        let mut evm = Evm::builder()
            .with_db(&mut cache_db)
            .with_env(Box::new(env))
            .build();

        let result = evm.transact_commit()
            .map_err(|e| eyre!("EVM execution failed: {:?}", e))?;

        match result {
            ExecutionResult::Success { output, .. } => {
                match output {
                    Output::Call(value) => {
                        let decoded = swapExactTokensForTokensCall::abi_decode_returns(&value, true)
                            .map_err(|_| eyre!("Failed to decode return data"))?;
                        
                        let amount_out_alloy = decoded.amounts.last()
                            .ok_or(eyre!("Empty amounts array returned"))?;
                            
                        Ok(U256::from_limbs(amount_out_alloy.into_limbs()))
                    },
                    Output::Create(_, _) => Err(eyre!("Unexpected contract creation output")),
                }
            },
            ExecutionResult::Revert { output, .. } => {
                Err(eyre!("Simulation Reverted (Slippage/Liquidity): {:?}", output))
            },
            ExecutionResult::Halt { reason, .. } => {
                Err(eyre!("Simulation Halted: {:?}", reason))
            },
        }
    }
}