use eyre::Result;
use reth::cli::Cli;
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use futures::StreamExt;
use alloy_sol_types::{sol, SolCall}; // The Decoder

// --- 1. THE DICTIONARY ---
// We define the specific ABI pattern we are looking for.
sol! {
    // Uniswap V2 Router Function
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] path,
        address to,
        uint256 deadline
    ) external returns (uint[] amounts);
}

/// The core logic of your "ExoCortex".
async fn exocortex_logic<Node: FullNodeComponents>(
    mut ctx: ExExContext<Node>,
) -> Result<()> {
    println!("\n\n==================================================");
    println!("   EXOCORTEX: EYES ONLINE (Decoding Enabled)      ");
    println!("==================================================\n");

    while let Some(Ok(notification)) = ctx.notifications.next().await {
        match notification {
            ExExNotification::ChainCommitted { new } => {
                
                // --- 2. THE PROCESSING LOOP ---
                // Destructure the tuple (block_number, block_data)
                for (_block_num, block) in new.blocks() {
                    
                    // FIX: Access .transactions inside .body
                    // Block structure: SealedBlock -> Block -> Body -> transactions
                    for tx in block.block.body.transactions.iter() {
                        
                        // Extract the input data (calldata)
                        let input_data = tx.input();

                        // --- 3. THE DECODE ---
                        // Use `abi_decode` for alloy-sol-types v0.7+
                        if let Ok(decoded) = swapExactTokensForTokensCall::abi_decode(input_data, true) {
                            
                            // VISUALIZATION
                            println!("\n[ExoCortex] >> 🎯 TARGET ACQUIRED (Uniswap V2 Swap)");
                            println!("    From:      {:?}", tx.recover_signer().unwrap_or_default());
                            println!("    Amount In: {} (raw)", decoded.amountIn);
                            println!("    Min Out:   {} (raw)", decoded.amountOutMin);
                            
                            // Path Logic: Show the route (Token A -> Token B)
                            if decoded.path.len() >= 2 {
                                println!("    Route:     {:?} -> ... -> {:?}", decoded.path.first(), decoded.path.last());
                            }
                            
                            println!("    Tx Hash:   {:?}\n", tx.hash);
                        }
                    }
                }

                // Notify the node we are done with this block range
                ctx.events.send(ExExEvent::FinishedHeight(
                    new.tip().num_hash(),
                ))?;
            }
            ExExNotification::ChainReorged { old, new } => {
                println!("[ExoCortex] Chain Reorg! Dropped {:?}, Adopted {:?}", old.range(), new.range());
            }
            ExExNotification::ChainReverted { old } => {
                println!("[ExoCortex] Chain Revert! Dropped {:?}", old.range());
            }
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