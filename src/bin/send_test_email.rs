//! Send one email through the configured provider, to check credentials
//! without creating a dispatch. See docs/notifications.md.
//!
//!     cargo run --bin send_test_email -- you@example.com

use logistics_system::logistics::notification::delivery::{DeliveryError, deliver};
use logistics_system::logistics::notification::notification::NotificationChannel;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let Some(to) = std::env::args().nth(1) else {
        eprintln!("usage: send_test_email <recipient>");
        return ExitCode::from(2);
    };
    let result = deliver(
        NotificationChannel::Email,
        &to,
        "Logistics email test",
        "This is a test message from the logistics system. Email delivery is working.",
    )
    .await;
    match result {
        Ok(()) => {
            println!("sent to {to}");
            ExitCode::SUCCESS
        }
        Err(DeliveryError::NotConfigured) => {
            eprintln!("no email provider configured: set SMTP_USERNAME and SMTP_PASSWORD");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("send failed: {e}");
            ExitCode::FAILURE
        }
    }
}
