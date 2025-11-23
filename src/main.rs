use eyre::Result;
use reth::cli::Cli;
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use futures::StreamExt; 

/// The core logic of your "ExoCortex".
async fn exocortex_logic<Node: FullNodeComponents>(
    mut ctx: ExExContext<Node>,
) -> Result<()> {
    // PRINTLN: This bypasses all log filters. You WILL see this.
    println!("\n\n==================================================");
    println!("   EXOCORTEX: ONLINE AND LISTENING (Stable v1.1)   ");
    println!("==================================================\n");

    while let Some(Ok(notification)) = ctx.notifications.next().await {
        match notification {
            ExExNotification::ChainCommitted { new } => {
                let range = new.range();
                let block_count = range.end() - range.start() + 1;
                
                // Print to console directly
                println!("\n[ExoCortex] >> Detected Commit! Block #{:?} ({} blocks)", range.end(), block_count);
                
                // Notify the node we are done
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