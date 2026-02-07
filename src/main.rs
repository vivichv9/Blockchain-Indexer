mod app;
mod core;
mod modules;

use anyhow::Result;
use app::App;
use modules::logging;

#[tokio::main]
async fn main() -> Result<()> {
    logging::init();

    let app = App::bootstrap().await?;
    app.run().await
}
