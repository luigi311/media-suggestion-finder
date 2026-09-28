use sea_orm_migration::{prelude::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {

        let stmt = Query::insert()
            .into_table("languages")
            .columns(["iso_639_1", "iso_639_2", "name"])
            .values_panic(["en".into(), "eng".into(), "English".into()])
            .values_panic(["es".into(), "spa".into(), "Spanish".into()])
            .values_panic(["fr".into(), "fra".into(), "French".into()])
            .values_panic(["de".into(), "deu".into(), "German".into()])
            .values_panic(["zh".into(), "zho".into(), "Chinese".into()])
            .values_panic(["ja".into(), "jpn".into(), "Japanese".into()])
            .values_panic(["ko".into(), "kor".into(), "Korean".into()])
            .values_panic(["ru".into(), "rus".into(), "Russian".into()])
            .values_panic(["it".into(), "ita".into(), "Italian".into()])
            .values_panic(["pt".into(), "por".into(), "Portuguese".into()])
            .to_owned();

        manager.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let stmt = Query::delete()
            .from_table("languages")
            .to_owned();

        manager.execute(stmt).await?;

        Ok(())
    }
}
