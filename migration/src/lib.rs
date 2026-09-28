pub use sea_orm_migration::prelude::*;

mod m20260412_184925_create_directories_table;
mod m20260412_190129_create_files_table;
mod m20260412_190210_create_media_files;
mod m20260412_190250_create_subtitle_files;
mod m20260412_190315_create_video_streams_table;
mod m20260412_190758_create_languages_table;
mod m20260412_191001_create_audio_streams_table;
mod m20260412_191344_subtitle_streams_table;
mod m20260412_212710_seed_language_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260412_184925_create_directories_table::Migration),
            Box::new(m20260412_190129_create_files_table::Migration),
            Box::new(m20260412_190210_create_media_files::Migration),
            Box::new(m20260412_190250_create_subtitle_files::Migration),
            Box::new(m20260412_190315_create_video_streams_table::Migration),
            Box::new(m20260412_190758_create_languages_table::Migration),
            Box::new(m20260412_191001_create_audio_streams_table::Migration),
            Box::new(m20260412_191344_subtitle_streams_table::Migration),
            Box::new(m20260412_212710_seed_language_table::Migration),

        ]
    }
}
