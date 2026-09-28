use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("files")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("directory_id").not_null())
                    .col(string("name").not_null()) // S01E01
                    .col(string("extension").not_null()) // mkv avi mp4 srt
                    .col(string("hash").not_null())
                    .col(string("hash_algorithm").not_null())
                    .col(big_integer("size_bytes").not_null())
                    .col(timestamp_with_time_zone("created_at").not_null())
                    .col(timestamp_with_time_zone("modified_at").not_null())
                    .col(timestamp_with_time_zone("missing_since").null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_files_directory_id")
                            .from("files", "directory_id")
                            .to("directories", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                .name("uq_files_directory_id_name")
                .if_not_exists()
                .table("files")
                .col("directory_id")
                .col("name")
                .unique()
                .to_owned()
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_files_hash")
                    .if_not_exists()
                    .table("files")
                    .col("hash")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_files_missing_since")
                    .if_not_exists()
                    .table("files")
                    .col("missing_since")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_files_directory_id_name")
                    .if_exists()
                    .table("files")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_files_hash")
                    .if_exists()
                    .table("files")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_files_missing_since")
                    .if_exists()
                    .table("files")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("files").if_exists().to_owned())
            .await?;

        Ok(())
    }
}
