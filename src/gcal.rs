use anyhow::{bail, Context, Result};
use chrono::{DateTime, FixedOffset, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::{Client, RequestBuilder, Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tracing::warn;

const API: &str = "https://www.googleapis.com/calendar/v3";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";
const MAX_ATTEMPTS: u32 = 4;

#[derive(Debug, Deserialize)]
struct ServiceAccount {
    client_email: String,
    private_key: String,
    #[serde(default = "default_token_uri")]
    token_uri: String,
}

fn default_token_uri() -> String {
    "https://oauth2.googleapis.com/token".to_string()
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: i64,
    exp: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventTime {
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtendedProperties {
    pub private: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub id: String,
    pub summary: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub start: Option<EventTime>,
    pub end: Option<EventTime>,
    #[serde(rename = "extendedProperties")]
    pub extended_properties: Option<ExtendedProperties>,
}

impl Event {
    pub fn property(&self, name: &str) -> Option<&str> {
        self.extended_properties
            .as_ref()?
            .private
            .as_ref()?
            .get(name)
            .map(String::as_str)
    }

    pub fn is_managed(&self) -> bool {
        self.property("bakasync") == Some("true")
    }

    pub fn lesson_key(&self) -> Option<&str> {
        self.property("bakasync_key")
    }
}

#[derive(Deserialize)]
struct EventList {
    #[serde(default)]
    items: Vec<Event>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

pub struct Calendar {
    http: Client,
    calendar_id: String,
    account: ServiceAccount,
    token: Option<(String, i64)>,
}

impl Calendar {
    pub fn new(http: Client, calendar_id: String, key_path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(key_path)
            .with_context(|| format!("reading service account key {}", key_path.display()))?;
        let account: ServiceAccount = serde_json::from_str(&raw)
            .with_context(|| format!("parsing service account key {}", key_path.display()))?;
        Ok(Calendar {
            http,
            calendar_id,
            account,
            token: None,
        })
    }

    pub async fn list(&mut self, from: &str, to: &str) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let token = self.token().await?;
            let mut request = self.http.get(self.url(&[])?).bearer_auth(&token).query(&[
                ("timeMin", from),
                ("timeMax", to),
                ("privateExtendedProperty", "bakasync=true"),
                ("singleEvents", "true"),
                ("maxResults", "2500"),
            ]);
            if let Some(page) = &page_token {
                request = request.query(&[("pageToken", page)]);
            }
            let page: EventList = send(request).await?.json().await?;
            events.extend(page.items);
            match page.next_page_token {
                Some(next) => page_token = Some(next),
                None => return Ok(events),
            }
        }
    }

    pub async fn insert(&mut self, body: &Value) -> Result<()> {
        let token = self.token().await?;
        let request = self.http.post(self.url(&[])?).bearer_auth(token).json(body);
        send(request).await?;
        Ok(())
    }

    pub async fn patch(&mut self, id: &str, body: &Value) -> Result<()> {
        let token = self.token().await?;
        let request = self
            .http
            .patch(self.url(&[id])?)
            .bearer_auth(token)
            .json(body);
        send(request).await?;
        Ok(())
    }

    pub async fn delete(&mut self, id: &str) -> Result<()> {
        let token = self.token().await?;
        let request = self.http.delete(self.url(&[id])?).bearer_auth(token);
        match send(request).await {
            Ok(_) => Ok(()),
            Err(error) => match error.downcast_ref::<ApiError>() {
                Some(api)
                    if api.status == StatusCode::GONE || api.status == StatusCode::NOT_FOUND =>
                {
                    Ok(())
                }
                _ => Err(error),
            },
        }
    }

    fn url(&self, extra: &[&str]) -> Result<Url> {
        let mut url = Url::parse(API)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("bad api base"))?;
            path.extend(["calendars", self.calendar_id.as_str(), "events"]);
            path.extend(extra.iter().copied());
        }
        Ok(url)
    }

    async fn token(&mut self) -> Result<String> {
        let now = Utc::now().timestamp();
        if let Some((token, expires_at)) = &self.token {
            if *expires_at > now + 60 {
                return Ok(token.clone());
            }
        }
        let claims = Claims {
            iss: &self.account.client_email,
            scope: SCOPE,
            aud: &self.account.token_uri,
            iat: now,
            exp: now + 3600,
        };
        let key = EncodingKey::from_rsa_pem(self.account.private_key.as_bytes())
            .context("service account private_key is not an RSA PEM key")?;
        let assertion = jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &key)?;
        let request = self.http.post(&self.account.token_uri).form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", assertion.as_str()),
        ]);
        let response: TokenResponse = send(request).await?.json().await?;
        self.token = Some((
            response.access_token.clone(),
            now + response.expires_in.max(60),
        ));
        Ok(response.access_token)
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "google api returned {}: {}",
            self.status, self.body
        )
    }
}

impl std::error::Error for ApiError {}

async fn send(request: RequestBuilder) -> Result<Response> {
    let mut attempt = 1;
    loop {
        let try_again = attempt < MAX_ATTEMPTS;
        let cloned = match request.try_clone() {
            Some(cloned) => cloned,
            None => bail!("request cannot be retried"),
        };
        match cloned.send().await {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                if !try_again || !retryable(status) {
                    return Err(ApiError { status, body }.into());
                }
                warn!("google api {status}, retrying (attempt {attempt})");
            }
            Err(error) => {
                if !try_again {
                    return Err(error).context("google api request failed");
                }
                warn!("google api transport error {error}, retrying (attempt {attempt})");
            }
        }
        tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
        attempt += 1;
    }
}

fn retryable(status: StatusCode) -> bool {
    status == StatusCode::FORBIDDEN
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

pub fn parse_instant(time: &Option<EventTime>) -> Option<DateTime<FixedOffset>> {
    let value = time.as_ref()?.date_time.as_deref()?;
    DateTime::parse_from_rfc3339(value).ok()
}
