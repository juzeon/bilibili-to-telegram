pub mod entity;
use crate::db::entity::history;
use crate::db::entity::prelude::History;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter,
};

#[derive(Clone)]
pub struct DB {
    db: DatabaseConnection,
}
impl DB {
    pub async fn new() -> Self {
        Self {
            db: Database::connect("sqlite://bili-to-tg.db").await.unwrap(),
        }
    }
    pub async fn find_history_by_bids(&self, bids: &[String]) -> Vec<history::Model> {
        History::find()
            .filter(history::Column::Bid.is_in(bids))
            .all(&self.db)
            .await
            .unwrap()
    }
    pub async fn update_history(&self, history: history::ActiveModel) {
        history.save(&self.db).await.unwrap();
    }
    pub async fn insert_history_arr(&self, arr: &[history::ActiveModel]) {
        History::insert_many(arr.to_vec())
            .exec(&self.db)
            .await
            .unwrap();
    }
}
