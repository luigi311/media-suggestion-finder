use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("subtitle_streams")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("file_id").not_null())
                    .col(integer("stream_index").not_null())
                    .col(string("title").null())
                    .col(string("codec").not_null())
                    .col(integer("language_id").null())
                    .col(boolean("is_default").not_null().default(false))
                    .col(boolean("is_forced").not_null().default(false))
                    .col(boolean("is_sdh").not_null().default(false))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_subtitle_streams_file_id")
                            .from("subtitle_streams", "file_id")
                            .to("files", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_subtitle_streams_language_id")
                            .from("subtitle_streams", "language_id")
                            .to("languages", "id")
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_subtitle_streams_file_id_stream_index")
                    .if_not_exists()
                    .table("subtitle_streams")
                    .col("file_id")
                    .col("stream_index")
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_subtitle_streams_language_id")
                    .if_not_exists()
                    .table("subtitle_streams")
                    .col("language_id")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_subtitle_streams_file_id_stream_index")
                    .if_exists()
                    .table("subtitle_streams")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_subtitle_streams_language_id")
                    .if_exists()
                    .table("subtitle_streams")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table("subtitle_streams")
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
