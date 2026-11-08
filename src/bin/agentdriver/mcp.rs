//! The MCP surface. Tools are async and call the synchronous core through
//! `spawn_blocking`, so a long wait never stalls the server.

use agentdriver::adapters::harness::recipes;
use rmcp::{ServiceExt, tool, tool_router, transport::stdio};

#[derive(Debug, Clone)]
struct Server;

#[tool_router(server_handler)]
impl Server {
    #[tool(description = "List the harnesses this build knows")]
    async fn harnesses(&self) -> Result<String, String> {
        tokio::task::spawn_blocking(|| {
            recipes()
                .iter()
                .map(|recipe| recipe.kind)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .await
        .map_err(|error| error.to_string())
    }
}

pub async fn serve() -> anyhow::Result<()> {
    Server.serve(stdio()).await?.waiting().await?;
    Ok(())
}
