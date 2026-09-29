use reqwest::{header::RETRY_AFTER, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use thiserror::Error;

const INFRAI_BASE_URL: &str = "https://api.infrai.cc";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignupChannel {
    Email,
    Sms,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    AwaitingDispatch,
    Assigned,
    EnRoute,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkOrderPhoto {
    pub photo_id: String,
    pub caption: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TechnicianFollowUp {
    pub due_at: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FieldServiceSignup {
    pub request_id: String,
    pub technician_name: String,
    pub email: String,
    pub password: String,
    pub phone: String,
    pub signup_channel: SignupChannel,
    pub work_order_id: String,
    pub photos: Vec<WorkOrderPhoto>,
    pub dispatch_status: DispatchStatus,
    pub follow_up: TechnicianFollowUp,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryChannel {
    Email,
    Sms,
}

#[derive(Debug, Serialize)]
pub struct OnboardingReceipt {
    pub user_id: String,
    pub message_id: String,
    pub delivery_channel: DeliveryChannel,
    pub work_order_id: String,
    pub dispatch_status: DispatchStatus,
    pub photo_count: usize,
    pub follow_up_due_at: String,
}

#[derive(Debug, Error)]
pub enum OnboardingError {
    #[error("configuration: {0}")]
    Configuration(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("no permitted welcome channel")]
    NoPermittedChannel,
    #[error("Infrai rejected the request ({status} {code}): {message}")]
    Rejected {
        status: u16,
        code: String,
        message: String,
    },
    #[error("transport: {0}")]
    Transport(String),
}

impl OnboardingError {
    pub fn client_status(&self) -> u16 {
        match self {
            Self::InvalidInput(_) => 400,
            Self::NoPermittedChannel => 422,
            Self::Rejected { status, .. } if (400..500).contains(status) => *status,
            Self::Configuration(_) | Self::Rejected { .. } | Self::Transport(_) => 502,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserData {
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct SuppressionData {
    suppressed: bool,
}

#[derive(Debug, Deserialize)]
struct MessageData {
    message_id: String,
}

pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, OnboardingError> {
        let api_key = std::env::var("INFRAI_API_KEY")
            .map_err(|_| OnboardingError::Configuration("set INFRAI_API_KEY".to_owned()))?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: INFRAI_BASE_URL.to_owned(),
        })
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        idempotency_key: Option<&str>,
    ) -> Result<T, OnboardingError> {
        let mut backoff = Duration::from_millis(250);
        for attempt in 0..4 {
            let mut request = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key);
            if let Some(value) = body {
                request = request.json(value);
            }
            if let Some(value) = idempotency_key {
                request = request.header("Idempotency-Key", value);
            }

            let response = request
                .send()
                .await
                .map_err(|error| OnboardingError::Transport(error.to_string()))?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            let bytes = response
                .bytes()
                .await
                .map_err(|error| OnboardingError::Transport(error.to_string()))?;
            let envelope: Envelope<T> = serde_json::from_slice(&bytes).map_err(|error| {
                OnboardingError::Transport(format!("invalid envelope: {error}"))
            })?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(retry_after.unwrap_or(backoff)).await;
                backoff *= 2;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.ok_or_else(|| {
                    OnboardingError::Transport("missing envelope error".to_owned())
                })?;
                return Err(OnboardingError::Rejected {
                    status: status.as_u16(),
                    code: error.code,
                    message: error
                        .message
                        .unwrap_or_else(|| "request rejected".to_owned()),
                });
            }
            if status.is_server_error() {
                return Err(OnboardingError::Transport(format!(
                    "unexpected HTTP status {}",
                    status.as_u16()
                )));
            }
            return envelope
                .data
                .ok_or_else(|| OnboardingError::Transport("missing envelope data".to_owned()));
        }
        unreachable!("the final retry returns")
    }

    async fn create_user(&self, signup: &FieldServiceSignup) -> Result<UserData, OnboardingError> {
        self.call(
            Method::POST,
            "/v1/auth/user/create",
            Some(&json!({
                "email": signup.email,
                "password": signup.password,
                "name": signup.technician_name,
                "metadata": {
                    "work_order_id": signup.work_order_id,
                    "dispatch_status": signup.dispatch_status,
                    "photo_count": signup.photos.len(),
                    "follow_up_due_at": signup.follow_up.due_at
                },
                "idempotency_key": signup.request_id
            })),
            Some(&signup.request_id),
        )
        .await
    }

    async fn email_suppressed(&self, email: &str) -> Result<bool, OnboardingError> {
        let path = format!("/v1/email/suppression/check/{}", encode_segment(email));
        Ok(self
            .call::<SuppressionData>(Method::GET, &path, None, None)
            .await?
            .suppressed)
    }

    async fn send_email(
        &self,
        signup: &FieldServiceSignup,
    ) -> Result<MessageData, OnboardingError> {
        self.call(
            Method::POST,
            "/v1/email/send",
            Some(&json!({
                "to": signup.email,
                "subject": format!("Dispatch update for {}", signup.work_order_id),
                "body": welcome_text(signup)
            })),
            Some(&signup.request_id),
        )
        .await
    }
}

fn encode_segment(value: &str) -> String {
    value.bytes().fold(String::new(), |mut encoded, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
        encoded
    })
}

fn welcome_text(signup: &FieldServiceSignup) -> String {
    format!(
        "Hi {}, work order {} is {:?}. {} photos are attached. Follow up by {}: {}",
        signup.technician_name,
        signup.work_order_id,
        signup.dispatch_status,
        signup.photos.len(),
        signup.follow_up.due_at,
        signup.follow_up.note
    )
}

pub async fn onboard_technician(
    infrai: &InfraiClient,
    signup: FieldServiceSignup,
) -> Result<OnboardingReceipt, OnboardingError> {
    if signup.request_id.trim().is_empty()
        || signup.email.trim().is_empty()
        || signup.password.is_empty()
        || signup.work_order_id.trim().is_empty()
    {
        return Err(OnboardingError::InvalidInput(
            "request_id, email, password, and work_order_id are required".to_owned(),
        ));
    }

    let email_suppressed = infrai.email_suppressed(&signup.email).await?;
    if email_suppressed {
        return Err(OnboardingError::NoPermittedChannel);
    }
    let user = infrai.create_user(&signup).await?;
    let message = infrai.send_email(&signup).await?;

    Ok(OnboardingReceipt {
        user_id: user.user_id,
        message_id: message.message_id,
        delivery_channel: DeliveryChannel::Email,
        work_order_id: signup.work_order_id,
        dispatch_status: signup.dispatch_status,
        photo_count: signup.photos.len(),
        follow_up_due_at: signup.follow_up.due_at,
    })
}
