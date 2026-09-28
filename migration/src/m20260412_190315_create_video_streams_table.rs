use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("video_streams")
                    .if_not_exists()
                    .col(pk_auto("id"))
                    .col(integer("file_id").not_null())
                    .col(integer("stream_index").not_null())
                    .col(string("codec").not_null())
                    .col(integer("width").not_null())
                    .col(integer("height").not_null())
                    .col(integer("bit_depth").null())
                    .col(string("hdr_format").null())
                    .col(integer("bitrate").null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_video_streams_file_id")
                            .from("video_streams", "file_id")
                            .to("files", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_video_streams_file_id_stream_index")
                    .if_not_exists()
                    .table("video_streams")
                    .col("file_id")
                    .col("stream_index")
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_video_streams_codec")
                    .if_not_exists()
                    .table("video_streams")
                    .col("codec")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_video_streams_file_id_stream_index")
                    .if_exists()
                    .table("video_streams")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_video_streams_codec")
                    .if_exists()
                    .table("video_streams")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table("video_streams").if_exists().to_owned())
            .await?;

        Ok(())
    }
}
