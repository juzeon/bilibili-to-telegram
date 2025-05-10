use serde::{Deserialize, Serialize};
use tokio::fs::read_to_string;

#[derive(Debug, Clone)]
pub struct DisplayHistory {
    pub bid: String,
    pub title: String,
    pub url: DisplayHistoryURL,
}
#[derive(Debug, Clone)]
pub struct DisplayHistoryURL(String);
impl DisplayHistoryURL {
    pub fn from_bid(bid: &str) -> Self {
        if bid.starts_with("BV") {
            Self(format!("https://www.bilibili.com/video/{}/", bid))
        } else {
            unimplemented!("bid unimpl")
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Config {
    pub chat_id: String,
    pub token: String,
}
impl Config {
    pub async fn from_file() -> Self {
        serde_yaml::from_str(read_to_string("config.yml").await.unwrap().as_str()).unwrap()
    }
}
