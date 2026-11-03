//! Actual SMS / email delivery for a queued [`Notification`](super::notification::Notification).
//!
//! `docs/notifications.md` used to describe this as "a deployment concern —
//! nothing in this repo talks to Twilio or an SMTP server." This module is
//! that wiring: SMS via the MSG91 REST API; email via SMTP (Gmail by
//! default, see [`SmtpConfig`]) or the Resend REST API. All are optional — an
//! environment with no credentials configured for a channel behaves exactly
//! as before (notifications stay `QUEUED`), so this is safe to ship without
//! any provider set up.
//!
//! SMTP exists alongside Resend because Resend only sends from a domain you
//! have verified, which rules out a `@gmail.com` sender.

use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::time::Duration;

/// Sending happens inline in the dispatch request, so a hung mail server must
/// not hold that request open for long.
const SMTP_TIMEOUT: Duration = Duration::from_secs(15);

/// Same reasoning as [`SMTP_TIMEOUT`].
const MSG91_TIMEOUT: Duration = Duration::from_secs(15);

const MSG91_FLOW_URL: &str = "https://control.msg91.com/api/v5/flow/";

/// DLT rejects a template variable value longer than this.
const SMS_VAR_MAX_CHARS: usize = 40;

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

/// One DLT-approved SMS message. Indian operators only deliver text matching
/// an approved template, so SMS sends a template ID plus variable values, never
/// free text. The wording and variables of each are in `docs/notifications.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemplate {
    DispatchCreatedCustomer,
    DispatchCreatedDriver,
    DispatchDelivered,
    DispatchRunningLate,
}

impl SmsTemplate {
    /// The environment variable holding this template's MSG91 ID.
    fn env_key(&self) -> &'static str {
        match self {
            SmsTemplate::DispatchCreatedCustomer => "MSG91_TEMPLATE_DISPATCH_CREATED_CUSTOMER",
            SmsTemplate::DispatchCreatedDriver => "MSG91_TEMPLATE_DISPATCH_CREATED_DRIVER",
            SmsTemplate::DispatchDelivered => "MSG91_TEMPLATE_DISPATCH_DELIVERED",
            SmsTemplate::DispatchRunningLate => "MSG91_TEMPLATE_DISPATCH_RUNNING_LATE",
        }
    }
}

/// Cut a template variable value down to what DLT accepts.
pub fn sms_var(value: &str) -> String {
    value.trim().chars().take(SMS_VAR_MAX_CHARS).collect()
}

/// A phone number as MSG91 wants it, `91` plus the 10-digit mobile number.
/// Stored numbers are free text (`+91 98765 43210`, `9876543210`, `098765...`).
/// `None` for anything else, including non-Indian numbers: DLT templates only
/// cover Indian operators, and a junk number shouldn't be sent or billed.
fn normalize_indian_mobile(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    let national = if raw.trim_start().starts_with('+') {
        digits.strip_prefix("91")?
    } else {
        match digits.len() {
            10 => digits.as_str(),
            11 => digits.strip_prefix('0')?,
            12 => digits.strip_prefix("91")?,
            _ => return None,
        }
    };
    (national.len() == 10 && national.starts_with(['6', '7', '8', '9']))
        .then(|| format!("91{national}"))
}

/// MSG91 settings: `MSG91_AUTH_KEY` turns SMS on. `MSG91_SENDER_ID` is only
/// needed when a template's sender is set to come from the API.
struct Msg91Config {
    auth_key: String,
    sender_id: Option<String>,
    url: String,
}

impl Msg91Config {
    fn from_env() -> Option<Self> {
        Some(Self {
            auth_key: non_blank_env("MSG91_AUTH_KEY")?,
            sender_id: non_blank_env("MSG91_SENDER_ID"),
            url: MSG91_FLOW_URL.to_string(),
        })
    }
}

fn non_blank_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|s| !s.trim().is_empty())
}

async fn send_via_msg91(
    config: &Msg91Config,
    template_id: &str,
    to: &str,
    vars: &[String],
) -> Result<(), DeliveryError> {
    let mobile = normalize_indian_mobile(to)
        .ok_or_else(|| DeliveryError::ProviderError("Not an Indian mobile number".to_string()))?;

    let mut recipient = serde_json::Map::new();
    recipient.insert("mobiles".to_string(), mobile.into());
    for (i, value) in vars.iter().enumerate() {
        recipient.insert(format!("VAR{}", i + 1), value.as_str().into());
    }
    // MSG91's docs name this field both `template_id` and `flow_id`, depending
    // on the page, so send the ID under both.
    let mut payload = serde_json::json!({
        "template_id": template_id,
        "flow_id": template_id,
        // Reports a bad key or template as an error sooner. It doesn't catch
        // them all: tried on 2026-11-03, a made-up key still got "success" on
        // its first call and only an error on the next.
        "realTimeResponse": "1",
        "recipients": [recipient],
    });
    if let Some(sender) = &config.sender_id {
        payload["sender"] = sender.as_str().into();
    }

    let resp = reqwest::Client::new()
        .post(&config.url)
        .header("authkey", &config.auth_key)
        .timeout(MSG91_TIMEOUT)
        .json(&payload)
        .send()
        .await
        .map_err(|e| DeliveryError::ProviderError(format!("Failed to reach MSG91: {e}")))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    // A rejected request can still come back as HTTP 200, with `"type": "error"`.
    // "success" means MSG91 queued the request, not that the phone got it.
    let accepted = status.is_success()
        && serde_json::from_str::<serde_json::Value>(&text)
            .is_ok_and(|reply| reply["type"] == "success");
    if !accepted {
        return Err(DeliveryError::ProviderError(format!(
            "MSG91 error ({status}): {text}"
        )));
    }
    Ok(())
}

/// Send one templated SMS. `NotConfigured` without `MSG91_AUTH_KEY` or without
/// an ID for this template, so templates can be approved and added one by one.
pub async fn send_sms(
    to: &str,
    template: SmsTemplate,
    vars: &[String],
) -> Result<(), DeliveryError> {
    let config = Msg91Config::from_env().ok_or(DeliveryError::NotConfigured)?;
    let template_id = non_blank_env(template.env_key()).ok_or(DeliveryError::NotConfigured)?;
    send_via_msg91(&config, &template_id, to, vars).await
}

/// `(api_key, from_address)`, or `None` if either `RESEND_API_KEY` /
/// `RESEND_FROM_EMAIL` is unset or blank.
fn resend_credentials() -> Option<(String, String)> {
    Some((
        non_blank_env("RESEND_API_KEY")?,
        non_blank_env("RESEND_FROM_EMAIL")?,
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
        let username = non_blank_env("SMTP_USERNAME")?;
        let password = non_blank_env("SMTP_PASSWORD")?;
        let port = non_blank_env("SMTP_PORT")
            .and_then(|p| p.trim().parse().ok())
            .unwrap_or(587);
        Some(Self {
            host: non_blank_env("SMTP_HOST").unwrap_or_else(|| "smtp.gmail.com".to_string()),
            port,
            from: non_blank_env("SMTP_FROM").unwrap_or_else(|| username.clone()),
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

/// Send one email: SMTP when it's configured, otherwise Resend.
pub async fn send_email(to: &str, subject: &str, body: &str) -> Result<(), DeliveryError> {
    match SmtpConfig::from_env() {
        Some(config) => send_via_smtp(&config, to, subject, body).await,
        None => send_via_resend(to, subject, body).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The "not configured" tests return before any network call, matching this
    // codebase's convention of never hitting a live third-party API in tests
    // (see ai/status.rs's equivalent note about the Anthropic call). The rest
    // talk to fake servers on localhost.
    //
    // No test sets a MSG91_TEMPLATE_* variable, so setting MSG91_AUTH_KEY below
    // can't make a parallel test reach the real API.

    #[actix_web::test]
    async fn sms_is_not_configured_without_an_auth_key() {
        // SAFETY: this module is the only code that reads the MSG91_*
        // variables, and it only ever leaves them unusable, see above.
        unsafe {
            std::env::remove_var("MSG91_AUTH_KEY");
        }
        let err = send_sms(
            "+91 98765 43210",
            SmsTemplate::DispatchCreatedDriver,
            &["a1b2c3d4".to_string(), "Asha".to_string()],
        )
        .await
        .expect_err("should be unconfigured");
        assert!(matches!(err, DeliveryError::NotConfigured));
    }

    #[actix_web::test]
    async fn sms_is_not_configured_without_an_id_for_that_template() {
        // SAFETY: see above.
        unsafe {
            std::env::set_var("MSG91_AUTH_KEY", "test-key");
        }
        let err = send_sms(
            "+91 98765 43210",
            SmsTemplate::DispatchDelivered,
            &["Asha".to_string(), "a1b2c3d4".to_string()],
        )
        .await
        .expect_err("should be unconfigured");
        assert!(matches!(err, DeliveryError::NotConfigured));
        // SAFETY: see above.
        unsafe {
            std::env::remove_var("MSG91_AUTH_KEY");
        }
    }

    #[actix_web::test]
    async fn email_is_not_configured_without_any_email_credentials() {
        // SAFETY: this module is the only code that reads the RESEND_* and
        // SMTP_* variables, and no other test sets them, so this doesn't race
        // with anything else.
        unsafe {
            std::env::remove_var("SMTP_USERNAME");
            std::env::remove_var("SMTP_PASSWORD");
            std::env::remove_var("RESEND_API_KEY");
            std::env::remove_var("RESEND_FROM_EMAIL");
        }
        let err = send_email("customer@example.com", "subject", "body")
            .await
            .expect_err("should be unconfigured");
        assert!(matches!(err, DeliveryError::NotConfigured));
    }

    #[test]
    fn indian_mobile_numbers_are_normalized_from_the_formats_we_store() {
        for raw in [
            "+91 98765 43210",
            "+919876543210",
            "9876543210",
            "09876543210",
            "919876543210",
            " 98765-43210 ",
        ] {
            assert_eq!(
                normalize_indian_mobile(raw).as_deref(),
                Some("919876543210"),
                "{raw}"
            );
        }
    }

    #[test]
    fn junk_and_foreign_numbers_are_rejected() {
        for raw in [
            "",
            "0",
            "9",
            "12345",
            "+91 98765432",    // too short, must not gain a second 91
            "+1 415 555 0100", // foreign
            "+44 7911 123456", // foreign
            "1234567890",      // Indian mobiles start with 6-9
            "98765432101234",  // too long
        ] {
            assert_eq!(normalize_indian_mobile(raw), None, "{raw}");
        }
    }

    #[test]
    fn sms_var_is_cut_to_the_dlt_limit_on_a_character_boundary() {
        assert_eq!(sms_var("  Asha Rao "), "Asha Rao");
        let long = "आ".repeat(50);
        assert_eq!(sms_var(&long).chars().count(), SMS_VAR_MAX_CHARS);
    }

    /// Answer one HTTP request on a local port and return what was sent.
    async fn fake_msg91(
        status: &'static str,
        reply: &'static str,
    ) -> (Msg91Config, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request).to_lowercase();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&request).into_owned()
        });
        let config = Msg91Config {
            auth_key: "test-key".to_string(),
            sender_id: Some("LOGIST".to_string()),
            url: format!("http://127.0.0.1:{port}/api/v5/flow/"),
        };
        (config, handle)
    }

    #[actix_web::test]
    async fn msg91_gets_the_template_normalized_number_and_variables() {
        let (config, server) =
            fake_msg91("200 OK", r#"{"message":"5762846b","type":"success"}"#).await;
        send_via_msg91(
            &config,
            "tmpl-1",
            "+91 98765 43210",
            &["Asha Rao".to_string(), "a1b2c3d4".to_string()],
        )
        .await
        .expect("send should succeed");

        let request = server.await.unwrap();
        assert!(request.starts_with("POST /api/v5/flow/ "), "{request}");
        assert!(
            request.to_lowercase().contains("authkey: test-key"),
            "{request}"
        );
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        let payload: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(payload["template_id"], "tmpl-1");
        assert_eq!(payload["flow_id"], "tmpl-1");
        assert_eq!(payload["realTimeResponse"], "1");
        assert_eq!(payload["sender"], "LOGIST");
        assert_eq!(
            payload["recipients"],
            serde_json::json!([{
                "mobiles": "919876543210",
                "VAR1": "Asha Rao",
                "VAR2": "a1b2c3d4",
            }])
        );
    }

    #[actix_web::test]
    async fn msg91_error_reported_with_http_200_is_a_provider_error() {
        let (config, _server) =
            fake_msg91("200 OK", r#"{"message":"flow id missing","type":"error"}"#).await;
        let err = send_via_msg91(&config, "tmpl-1", "9876543210", &[])
            .await
            .expect_err("should fail");
        assert!(
            matches!(&err, DeliveryError::ProviderError(m) if m.contains("flow id missing")),
            "{err}"
        );
    }

    #[actix_web::test]
    async fn msg91_http_error_is_a_provider_error() {
        let (config, _server) = fake_msg91(
            "401 Unauthorized",
            r#"{"message":"Authentication failure","type":"error"}"#,
        )
        .await;
        let err = send_via_msg91(&config, "tmpl-1", "9876543210", &[])
            .await
            .expect_err("should fail");
        assert!(matches!(err, DeliveryError::ProviderError(_)), "{err}");
    }

    #[actix_web::test]
    async fn junk_number_is_rejected_without_calling_msg91() {
        // Port 1 has nothing listening: reaching the network would surface as
        // "Failed to reach MSG91" rather than this error.
        let config = Msg91Config {
            auth_key: "test-key".to_string(),
            sender_id: None,
            url: "http://127.0.0.1:1/".to_string(),
        };
        let err = send_via_msg91(&config, "tmpl-1", "9", &[])
            .await
            .expect_err("should fail");
        assert!(
            matches!(&err, DeliveryError::ProviderError(m) if m.contains("Not an Indian mobile")),
            "{err}"
        );
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
