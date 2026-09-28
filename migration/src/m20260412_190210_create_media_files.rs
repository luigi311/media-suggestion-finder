use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("media_files")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("file_id").not_null().unique_key())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_files_file_id")
                            .from("media_files", "file_id")
                            .to("files", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("media_files").if_exists().to_owned())
            .await?;

        Ok(())
    }
}
