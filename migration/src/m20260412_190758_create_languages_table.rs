use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("languages")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(string("iso_639_1").null())
                    .col(string("iso_639_2").null())
                    .col(string("name").null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_languages_name")
                    .if_not_exists()
                    .table("languages")
                    .col("name")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_languages_name")
                    .if_exists()
                    .table("languages")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("languages").if_exists().to_owned())
            .await?;

        Ok(())
    }
}
