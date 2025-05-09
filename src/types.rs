#[derive(Debug)]
pub struct DisplayHistory {
    pub bid: String,
    pub title: String,
    pub url: DisplayHistoryURL,
}
#[derive(Debug)]
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
