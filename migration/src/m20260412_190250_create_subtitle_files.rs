use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("subtitle_files")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("file_id").not_null().unique_key())
                    .col(integer("media_file_id").not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_subtitle_files_file_id")
                            .from("subtitle_files", "file_id")
                            .to("files", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_subtitle_files_media_file_id")
                            .from("subtitle_files", "media_file_id")
                            .to("media_files", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_subtitle_files_media_file_id")
                    .if_not_exists()
                    .table("subtitle_files")
                    .col("media_file_id")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_subtitle_files_media_file_id")
                    .if_exists()
                    .table("subtitle_files")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("subtitle_files").if_exists().to_owned())
            .await?;

        Ok(())
    }
}
