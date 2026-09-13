//! Actual SMS / email delivery for a queued [`Notification`](super::notification::Notification).
//!
//! `docs/notifications.md` used to describe this as "a deployment concern —
//! nothing in this repo talks to Twilio or an SMTP server." This module is
//! that wiring: SMS via the Twilio REST API, email via the Resend REST API
//! (a JSON-over-HTTPS transactional email API, chosen over raw SMTP so this
//! reuses the same reqwest-based HTTP-call shape already used for the
//! Anthropic integration rather than adding a new client/protocol
//! dependency). Both are optional — an environment with no credentials
//! configured for a channel behaves exactly as before (notifications stay
//! `QUEUED`), so this is safe to ship without either provider set up.

use crate::logistics::notification::notification::NotificationChannel;

/// Why a delivery attempt didn't end in a sent message.
#[derive(Debug)]
pub enum DeliveryError {
    /// No credentials are configured for this channel's provider — nothing
    /// was attempted. Not a failure: the caller should leave the
    /// notification `QUEUED`, ready to send once it is configured.
    NotConfigured,
    /// The provider was actually called and it reported an error.
    ProviderError(String),
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeliveryError::NotConfigured => write!(f, "provider not configured"),
            DeliveryError::ProviderError(msg) => write!(f, "{msg}"),
        }
    }
}

/// `(account_sid, auth_token, from_number)`, or `None` if any of
/// `TWILIO_ACCOUNT_SID` / `TWILIO_AUTH_TOKEN` / `TWILIO_FROM_NUMBER` is
/// unset or blank.
fn twilio_credentials() -> Option<(String, String, String)> {
    let non_blank = |key: &str| std::env::var(key).ok().filter(|s| !s.trim().is_empty());
    Some((
        non_blank("TWILIO_ACCOUNT_SID")?,
        non_blank("TWILIO_AUTH_TOKEN")?,
        non_blank("TWILIO_FROM_NUMBER")?,
    ))
}

async fn send_sms(to: &str, body: &str) -> Result<(), DeliveryError> {
    let (account_sid, auth_token, from) = twilio_credentials().ok_or(DeliveryError::NotConfigured)?;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{account_sid}/Messages.json"
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&[("To", to), ("From", from.as_str()), ("Body", body)])
        .send()
        .await
        .map_err(|e| DeliveryError::ProviderError(format!("Failed to reach Twilio: {e}")))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(DeliveryError::ProviderError(format!("Twilio error: {err}")));
    }
    Ok(())
}

/// `(api_key, from_address)`, or `None` if either `RESEND_API_KEY` /
/// `RESEND_FROM_EMAIL` is unset or blank.
fn resend_credentials() -> Option<(String, String)> {
    let non_blank = |key: &str| std::env::var(key).ok().filter(|s| !s.trim().is_empty());
    Some((non_blank("RESEND_API_KEY")?, non_blank("RESEND_FROM_EMAIL")?))
}

async fn send_email(to: &str, subject: &str, body: &str) -> Result<(), DeliveryError> {
    let (api_key, from) = resend_credentials().ok_or(DeliveryError::NotConfigured)?;

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.resend.com/emails")
        .bearer_auth(&api_key)
        .json(&serde_json::json!({
            "from": from,
            "to": [to],
            "subject": subject,
            "text": body,
        }))
        .send()
        .await
        .map_err(|e| DeliveryError::ProviderError(format!("Failed to reach Resend: {e}")))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(DeliveryError::ProviderError(format!("Resend error: {err}")));
    }
    Ok(())
}

/// Attempt to actually send one notification. `subject` is only used for
/// [`NotificationChannel::Email`].
pub async fn deliver(channel: NotificationChannel, to: &str, subject: &str, body: &str) -> Result<(), DeliveryError> {
    match channel {
        NotificationChannel::Sms => send_sms(to, body).await,
        NotificationChannel::Email => send_email(to, subject, body).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both tests below only exercise the "not configured" short-circuit,
    // which returns before any network call — matching this codebase's
    // convention of never hitting a live third-party API in tests (see
    // ai/status.rs's equivalent note about the Anthropic call).

    #[actix_web::test]
    async fn deliver_sms_is_not_configured_without_twilio_credentials() {
        // SAFETY: this module is the only code that reads the TWILIO_*
        // variables, and no other test sets them, so this doesn't race with
        // anything else.
        unsafe {
            std::env::remove_var("TWILIO_ACCOUNT_SID");
            std::env::remove_var("TWILIO_AUTH_TOKEN");
            std::env::remove_var("TWILIO_FROM_NUMBER");
        }
        let err = deliver(NotificationChannel::Sms, "+15551234567", "subject", "body")
            .await
            .expect_err("should be unconfigured");
        assert!(matches!(err, DeliveryError::NotConfigured));
    }

    #[actix_web::test]
    async fn deliver_email_is_not_configured_without_resend_credentials() {
        // SAFETY: this module is the only code that reads the RESEND_*
        // variables, and no other test sets them, so this doesn't race with
        // anything else.
        unsafe {
            std::env::remove_var("RESEND_API_KEY");
            std::env::remove_var("RESEND_FROM_EMAIL");
        }
        let err = deliver(NotificationChannel::Email, "customer@example.com", "subject", "body")
            .await
            .expect_err("should be unconfigured");
        assert!(matches!(err, DeliveryError::NotConfigured));
    }

    #[actix_web::test]
    async fn deliver_sms_is_not_configured_when_only_some_twilio_variables_are_set() {
        // SAFETY: see above.
        unsafe {
            std::env::set_var("TWILIO_ACCOUNT_SID", "AC123");
            std::env::remove_var("TWILIO_AUTH_TOKEN");
            std::env::remove_var("TWILIO_FROM_NUMBER");
        }
        let err = deliver(NotificationChannel::Sms, "+15551234567", "subject", "body")
            .await
            .expect_err("should be unconfigured with a partial credential set");
        assert!(matches!(err, DeliveryError::NotConfigured));
        // SAFETY: see above.
        unsafe {
            std::env::remove_var("TWILIO_ACCOUNT_SID");
        }
    }
}
