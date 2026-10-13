use sea_orm_migration::prelude::*;

#[tokio::main]
#[hotpath::main]
async fn main() {
    hotpath::tokio_runtime!();
    cli::run_cli(migration::Migrator).await;
}
