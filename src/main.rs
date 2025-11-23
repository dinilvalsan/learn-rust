mod simulation;
use simulation::SimulationEngine;

use eyre::Result;
use reth::cli::Cli;
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_revm::database::StateProviderDatabase;
use reth::providers::StateProviderFactory;
use futures::StreamExt;
use alloy_sol_types::{sol, SolCall}; 

sol! {
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] path,
        address to,
        uint256 deadline
    ) external returns (uint[] amounts);
}

async fn exocortex_logic<Node: FullNodeComponents>(
    mut ctx: ExExContext<Node>,
) -> Result<()> {
    println!("\n\n==================================================");
    println!("   EXOCORTEX: PHASE 2 SIMULATION ONLINE      ");
    println!("==================================================\n");

    while let Some(Ok(notification)) = ctx.notifications.next().await {
        match notification {
            ExExNotification::ChainCommitted { new } => {
                
                for (block_num, block) in new.blocks() {
                    
                    // 1. Get State & Create DB
                    let state = ctx.components.provider().history_by_block_number(*block_num)?;
                    let db = StateProviderDatabase::new(state);
                    
                    // 2. Initialize Engine (Once per block)
                    let mut engine = SimulationEngine::new(db);

                    for tx in block.block.body.transactions.iter() {
                        let input_data = tx.input();

                        if let Ok(decoded) = swapExactTokensForTokensCall::abi_decode(input_data, true) {
                            
                            println!("\n[ExoCortex] >> 🎯 TARGET ACQUIRED (Block {})", block_num);
                            println!("    Input: {} tokens", decoded.amountIn);
                            
                            if decoded.path.len() >= 2 {
                                
                                println!("    ⚡ Simulating execution...");
                                
                                let caller = tx.recover_signer().unwrap_or_default();
                                let router = tx.to().unwrap_or_default();

                                // Convert types for the Simulation Engine
                                let router_revm = revm::primitives::Address::from(router.0 .0);
                                let caller_revm = revm::primitives::Address::from(caller.0 .0);
                                let amount_in_revm = revm::primitives::U256::from_limbs(decoded.amountIn.into_limbs());
                                
                                let path_revm: Vec<revm::primitives::Address> = decoded.path.iter()
                                    .map(|a| revm::primitives::Address::from(a.0 .0))
                                    .collect();

                                // 3. Run Simulation
                                let prediction = engine.simulate_swap(
                                    router_revm,
                                    caller_revm,
                                    amount_in_revm,
                                    path_revm,
                                );

                                match prediction {
                                    Ok(amount_out) => {
                                        println!("    ✅ SIMULATION SUCCESS!");
                                        println!("    🔮 Predicted Output: {} tokens", amount_out);
                                    }
                                    Err(e) => {
                                        println!("    ⚠️ Simulation Failed: {}", e);
                                    }
                                }
                            }
                            println!("    Tx Hash: {:?}\n", tx.hash);
                        }
                    }
                }

                ctx.events.send(ExExEvent::FinishedHeight(
                    new.tip().num_hash(),
                ))?;
            }
            ExExNotification::ChainReorged { .. } => {}
            ExExNotification::ChainReverted { .. } => {}
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(EthereumNode::default())
            .install_exex("ExoCortex", |ctx| async move { Ok(exocortex_logic(ctx)) })
            .launch()
            .await?;

        handle.wait_for_node_exit().await
    })
}