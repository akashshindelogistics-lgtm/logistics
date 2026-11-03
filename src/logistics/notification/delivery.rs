//! Actual SMS / email delivery for a queued [`Notification`](super::notification::Notification).
//!
//! `docs/notifications.md` used to describe this as "a deployment concern —
//! nothing in this repo talks to Twilio or an SMTP server." This module is
//! that wiring: SMS via the Twilio REST API; email via SMTP (Gmail by
//! default, see [`SmtpConfig`]) or the Resend REST API. All are optional — an
//! environment with no credentials configured for a channel behaves exactly
//! as before (notifications stay `QUEUED`), so this is safe to ship without
//! any provider set up.
//!
//! SMTP exists alongside Resend because Resend only sends from a domain you
//! have verified, which rules out a `@gmail.com` sender.

use crate::logistics::notification::notification::NotificationChannel;
use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::time::Duration;

/// Sending happens inline in the dispatch request, so a hung mail server must
/// not hold that request open for long.
const SMTP_TIMEOUT: Duration = Duration::from_secs(15);

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
    let (account_sid, auth_token, from) =
        twilio_credentials().ok_or(DeliveryError::NotConfigured)?;

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
    Some((
        non_blank("RESEND_API_KEY")?,
        non_blank("RESEND_FROM_EMAIL")?,
    ))
}

/// How the connection to the SMTP server is encrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmtpSecurity {
    /// Connect in the clear and upgrade with STARTTLS (required, not
    /// opportunistic). The usual choice on port 587.
    StartTls,
    /// TLS from the first byte. The usual choice on port 465.
    ImplicitTls,
    /// No encryption, for the fake server in this module's tests. Deliberately
    /// not reachable from the environment: the password must never go out in
    /// the clear because of a config typo.
    #[cfg(test)]
    Plaintext,
}

/// SMTP settings. `SMTP_USERNAME` and `SMTP_PASSWORD` are what turn SMTP on;
/// the rest default to Gmail: `SMTP_HOST` (`smtp.gmail.com`), `SMTP_PORT`
/// (`587`; `465` selects implicit TLS) and `SMTP_FROM` (the username, or
/// `Display Name <address>`).
#[derive(Debug, Clone)]
struct SmtpConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    from: String,
    security: SmtpSecurity,
}

impl SmtpConfig {
    fn from_env() -> Option<Self> {
        let non_blank = |key: &str| std::env::var(key).ok().filter(|s| !s.trim().is_empty());
        let username = non_blank("SMTP_USERNAME")?;
        let password = non_blank("SMTP_PASSWORD")?;
        let port = non_blank("SMTP_PORT")
            .and_then(|p| p.trim().parse().ok())
            .unwrap_or(587);
        Some(Self {
            host: non_blank("SMTP_HOST").unwrap_or_else(|| "smtp.gmail.com".to_string()),
            port,
            from: non_blank("SMTP_FROM").unwrap_or_else(|| username.clone()),
            username,
            password,
            security: if port == 465 {
                SmtpSecurity::ImplicitTls
            } else {
                SmtpSecurity::StartTls
            },
        })
    }
}

fn build_email(from: &str, to: &str, subject: &str, body: &str) -> Result<Message, DeliveryError> {
    let parse = |label: &str, address: &str| {
        address.trim().parse::<Mailbox>().map_err(|e| {
            DeliveryError::ProviderError(format!("Invalid {label} address {address:?}: {e}"))
        })
    };
    Message::builder()
        .from(parse("sender", from)?)
        .to(parse("recipient", to)?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .map_err(|e| DeliveryError::ProviderError(format!("Failed to build email: {e}")))
}

async fn send_via_smtp(
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), DeliveryError> {
    let message = build_email(&config.from, to, subject, body)?;

    let smtp_error = |e: lettre::transport::smtp::Error| {
        DeliveryError::ProviderError(format!("SMTP error: {e}"))
    };
    let builder = match config.security {
        SmtpSecurity::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
                .map_err(smtp_error)?
        }
        SmtpSecurity::ImplicitTls => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host).map_err(smtp_error)?
        }
        #[cfg(test)]
        SmtpSecurity::Plaintext => {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
        }
    };
    let transport = builder
        .port(config.port)
        .credentials(Credentials::new(
            config.username.clone(),
            config.password.clone(),
        ))
        .timeout(Some(SMTP_TIMEOUT))
        .build();

    transport.send(message).await.map_err(smtp_error)?;
    Ok(())
}

async fn send_via_resend(to: &str, subject: &str, body: &str) -> Result<(), DeliveryError> {
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

/// SMTP when it's configured, otherwise Resend.
async fn send_email(to: &str, subject: &str, body: &str) -> Result<(), DeliveryError> {
    match SmtpConfig::from_env() {
        Some(config) => send_via_smtp(&config, to, subject, body).await,
        None => send_via_resend(to, subject, body).await,
    }
}

/// Attempt to actually send one notification. `subject` is only used for
/// [`NotificationChannel::Email`].
pub async fn deliver(
    channel: NotificationChannel,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), DeliveryError> {
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
    async fn deliver_email_is_not_configured_without_any_email_credentials() {
        // SAFETY: this module is the only code that reads the RESEND_* and
        // SMTP_* variables, and no other test sets them, so this doesn't race
        // with anything else.
        unsafe {
            std::env::remove_var("SMTP_USERNAME");
            std::env::remove_var("SMTP_PASSWORD");
            std::env::remove_var("RESEND_API_KEY");
            std::env::remove_var("RESEND_FROM_EMAIL");
        }
        let err = deliver(
            NotificationChannel::Email,
            "customer@example.com",
            "subject",
            "body",
        )
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

    /// Serve one SMTP session on a local port, accepting everything, and
    /// return the commands and message data the client sent.
    async fn fake_smtp_server() -> (u16, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut lines = BufReader::new(read).lines();
            let mut transcript = String::new();
            let mut in_data = false;
            write.write_all(b"220 fake ESMTP\r\n").await.unwrap();
            while let Some(line) = lines.next_line().await.unwrap() {
                transcript.push_str(&line);
                transcript.push('\n');
                let reply: &[u8] = if in_data {
                    if line != "." {
                        continue;
                    }
                    in_data = false;
                    b"250 queued\r\n"
                } else if line.starts_with("EHLO") {
                    b"250-fake\r\n250 AUTH PLAIN LOGIN\r\n"
                } else if line.starts_with("AUTH") {
                    b"235 ok\r\n"
                } else if line == "DATA" {
                    in_data = true;
                    b"354 go\r\n"
                } else if line == "QUIT" {
                    write.write_all(b"221 bye\r\n").await.unwrap();
                    break;
                } else {
                    b"250 ok\r\n"
                };
                write.write_all(reply).await.unwrap();
            }
            transcript
        });
        (port, handle)
    }

    fn fake_config(port: u16, from: &str) -> SmtpConfig {
        SmtpConfig {
            host: "127.0.0.1".to_string(),
            port,
            username: "sender@gmail.com".to_string(),
            password: "app-password".to_string(),
            from: from.to_string(),
            security: SmtpSecurity::Plaintext,
        }
    }

    #[actix_web::test]
    async fn smtp_authenticates_and_sends_the_message_over_the_wire() {
        let (port, server) = fake_smtp_server().await;
        send_via_smtp(
            &fake_config(port, "Dispatch Desk <sender@gmail.com>"),
            "customer@example.com",
            "Your order is on its way",
            "Hi Asha, your order is on its way.",
        )
        .await
        .expect("send should succeed");

        let transcript = server.await.unwrap();
        for expected in [
            "AUTH ",
            "MAIL FROM:<sender@gmail.com>",
            "RCPT TO:<customer@example.com>",
            "From: \"Dispatch Desk\" <sender@gmail.com>",
            "Subject: Your order is on its way",
            "Hi Asha, your order is on its way.",
        ] {
            assert!(transcript.contains(expected), "{expected}\n{transcript}");
        }
    }

    #[actix_web::test]
    async fn smtp_rejects_a_bad_recipient_without_connecting() {
        // Port 1 has nothing listening: reaching the network would surface as
        // a connection error rather than this address error.
        let err = send_via_smtp(
            &fake_config(1, "sender@gmail.com"),
            "not an email",
            "subject",
            "body",
        )
        .await
        .expect_err("should reject the address");
        assert!(
            matches!(&err, DeliveryError::ProviderError(m) if m.contains("recipient")),
            "{err}"
        );
    }

    #[actix_web::test]
    async fn smtp_reports_an_unreachable_server_as_a_provider_error() {
        let err = send_via_smtp(
            &fake_config(1, "sender@gmail.com"),
            "customer@example.com",
            "subject",
            "body",
        )
        .await
        .expect_err("nothing is listening");
        assert!(matches!(err, DeliveryError::ProviderError(_)), "{err}");
    }

    /// An address carrying a line break must not be able to add mail headers.
    /// Customer emails are free text from the dashboard.
    #[test]
    fn email_recipient_with_a_line_break_is_rejected() {
        let err = build_email(
            "sender@gmail.com",
            "a@example.com\r\nBcc: victim@example.com",
            "subject",
            "body",
        )
        .expect_err("should reject the address");
        assert!(matches!(err, DeliveryError::ProviderError(_)), "{err}");
    }
}
