use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_video_streams_file_id")
                    .if_not_exists()
                    .table("video_streams")
                    .col("file_id")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_audio_streams_file_id")
                    .if_not_exists()
                    .table("audio_streams")
                    .col("file_id")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_subtitle_streams_file_id")
                    .if_not_exists()
                    .table("subtitle_streams")
                    .col("file_id")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_video_streams_file_id")
                    .if_exists()
                    .table("audio_streams")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_audio_streams_file_id")
                    .if_exists()
                    .table("audio_streams")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_subtitle_streams_file_id")
                    .if_exists()
                    .table("audio_streams")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
