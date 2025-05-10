use crate::types::{DisplayHistory, DisplayHistoryURL};
use anyhow::{Context, Error, anyhow, bail};
use qrcode_generator::QrCodeEcc;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue, SET_COOKIE, USER_AGENT};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{read_to_string, remove_file, try_exists, write};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, info, instrument, warn};

#[derive(Default, Clone)]
pub struct Client {
    rc: Arc<Mutex<reqwest::Client>>,
    cookie: Arc<Mutex<String>>,
    mid: Arc<Mutex<i64>>,
}
impl Client {
    pub async fn new() -> anyhow::Result<Client> {
        let client = Self { ..Self::default() };
        client.build_rc().await;
        if try_exists("cookie.txt").await? {
            client.read_file_cookie().await?;
            client.build_rc().await;
            if !client.check_update_user_status().await? {
                client.login().await?;
            }
        } else {
            client.login().await?;
        }
        Ok(client)
    }
    async fn get_rc(&self) -> reqwest::Client {
        self.rc.lock().await.clone()
    }
    async fn get_mid(&self) -> i64 {
        *self.mid.lock().await
    }
    async fn build_rc(&self) {
        let mut map = HeaderMap::new();
        map.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:138.0) Gecko/20100101 Firefox/138.0",
            ),
        );
        let cookie = self.cookie.lock().await;
        if !cookie.is_empty() {
            map.insert(COOKIE, HeaderValue::from_str(cookie.as_str()).unwrap());
        }
        let mut rc = self.rc.lock().await;
        *rc = reqwest::Client::builder()
            .default_headers(map)
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
    }
    pub async fn check_update_user_status(&self) -> anyhow::Result<bool> {
        let resp: serde_json::Value = self
            .get_rc()
            .await
            .get("https://api.bilibili.com/x/web-interface/nav")
            .send()
            .await?
            .json()
            .await?;
        if resp["code"].as_i64().context("cannot get code")? == 0 {
            let mid = resp["data"]["mid"].as_i64().context("no mid")?;
            let mut mid_lock = self.mid.lock().await;
            *mid_lock = mid;
            return Ok(true);
        }
        warn!(message=?resp["message"]);
        Ok(false)
    }
    async fn read_file_cookie(&self) -> anyhow::Result<()> {
        let mut cookie = self.cookie.lock().await;
        *cookie = read_to_string("cookie.txt").await?;
        Ok(())
    }
    async fn write_file_cookie(&self) {
        let cookie = self.cookie.lock().await;
        write("cookie.txt", cookie.as_str()).await.unwrap();
    }
    pub async fn load() -> anyhow::Result<Client> {
        let mut client = Self { ..Self::default() };
        client.read_file_cookie().await?;
        Ok(client)
    }
    #[instrument(skip_all)]
    pub async fn get_recent_upvotes(&self) -> anyhow::Result<Vec<DisplayHistory>> {
        let resp: serde_json::Value = self
            .get_rc()
            .await
            .get(format!(
                "https://api.bilibili.com/x/space/like/video?vmid={}",
                self.get_mid().await
            ))
            .send()
            .await?
            .json()
            .await?;
        debug!(?resp);
        let arr = resp["data"]["list"]
            .as_array()
            .context("data is not an array")?
            .iter()
            .map(|item| {
                let bid = item["bvid"].as_str().context("no bvid")?;
                let title = item["title"].as_str().context("no title")?;
                Ok(DisplayHistory {
                    bid: bid.to_string(),
                    title: title.to_string(),
                    url: DisplayHistoryURL::from_bid(bid),
                })
            })
            .collect::<anyhow::Result<Vec<DisplayHistory>>>()?;
        Ok(arr)
    }
    #[instrument(skip_all)]
    pub async fn get_recent_view(&self) -> anyhow::Result<Vec<DisplayHistory>> {
        let resp: serde_json::Value = self
            .get_rc()
            .await
            .get("https://api.bilibili.com/x/web-interface/history/cursor")
            .send()
            .await?
            .json()
            .await?;
        debug!(?resp);
        let arr = resp["data"]["list"]
            .as_array()
            .context("data is not an array")?
            .iter()
            .filter_map(|item| -> Option<anyhow::Result<DisplayHistory>> {
                // Option<Result<T>>
                let bvid = match item["history"]["bvid"].as_str() {
                    Some(v) => v,
                    None => return Some(Err(anyhow!("bvid not str"))),
                };
                if bvid.is_empty() {
                    return None;
                }
                let title = match item["title"].as_str() {
                    Some(v) => v,
                    None => return Some(Err(anyhow!("title not str"))),
                };
                Some(Ok(DisplayHistory {
                    bid: bvid.to_string(),
                    title: title.to_string(),
                    url: DisplayHistoryURL::from_bid(bvid),
                }))
            })
            .collect::<anyhow::Result<Vec<DisplayHistory>>>()?;
        Ok(arr)
    }
    pub async fn login(&self) -> anyhow::Result<()> {
        let resp: serde_json::Value = self
            .get_rc()
            .await
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
            .send()
            .await?
            .json()
            .await?;
        let code = resp["code"].as_i64().context("cannot convert code")?;
        if code != 0 {
            bail!(
                resp["message"]
                    .as_str()
                    .context("cannot convert message")?
                    .to_string()
            )
        }
        let qr_url = resp["data"]["url"].as_str().context("no qr url")?;
        let qr_key = resp["data"]["qrcode_key"].as_str().context("no qr key")?;
        info!(qr_url, "Please scan the QR code");
        qrcode_generator::to_png_to_file(qr_url, QrCodeEcc::Medium, 1024, "qr.png")?;
        loop {
            let response = self
                .get_rc()
                .await
                .get(format!(
                    "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
                    qr_key
                ))
                .send()
                .await?;
            let cookie = response
                .headers()
                .iter()
                .filter_map(|(k, v)| {
                    if k != SET_COOKIE {
                        return None;
                    }
                    Some(format!(
                        "{}; ",
                        v.to_str().unwrap().split(";").nth(0).unwrap()
                    ))
                })
                .collect::<Vec<_>>()
                .join("");
            let json_resp: serde_json::Value = response.json().await?;
            let message = json_resp["data"]["message"]
                .as_str()
                .context("no message")?
                .to_string();
            let code = json_resp["data"]["code"].as_i64().context("no code")?;
            info!(message);
            if code == 0 {
                info!(cookie);
                let mut cookie_lock = self.cookie.lock().await;
                *cookie_lock = cookie;
                drop(cookie_lock);
                self.write_file_cookie().await;
                self.build_rc().await;
                remove_file("qr.png").await?;
                break;
            }
            if code == 86038 {
                bail!(message);
            }
            sleep(Duration::from_secs(2)).await;
        }
        if !self.check_update_user_status().await? {
            bail!("cannot get user status");
        }
        Ok(())
    }
}
