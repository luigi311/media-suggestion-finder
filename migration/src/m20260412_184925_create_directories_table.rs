use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("directories")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(string("path").unique_key().not_null())
                    .col(integer("parent_id").null())
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk_directories_parent_id")
                            .from("directories", "parent_id")
                            .to("directories", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("directories").if_exists().to_owned())
            .await?;

        Ok(())
    }
}
