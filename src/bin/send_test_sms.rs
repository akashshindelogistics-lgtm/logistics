//! Send one real, billed SMS through MSG91 using the driver template, to check
//! the key and template ID without creating a dispatch. See
//! docs/notifications.md.
//!
//!     cargo run --bin send_test_sms -- 9876543210

use logistics_system::logistics::notification::delivery::{DeliveryError, SmsTemplate, send_sms};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let Some(to) = std::env::args().nth(1) else {
        eprintln!("usage: send_test_sms <mobile number>");
        return ExitCode::from(2);
    };
    let vars = ["TEST0000".to_string(), "Test Customer".to_string()];
    match send_sms(&to, SmsTemplate::DispatchCreatedDriver, &vars).await {
        Ok(()) => {
            println!(
                "queued by MSG91 for {to}. Check that the phone receives it: MSG91 can also \
                 queue a request with a wrong key or template, then drop it."
            );
            ExitCode::SUCCESS
        }
        Err(DeliveryError::NotConfigured) => {
            eprintln!(
                "SMS not configured: set MSG91_AUTH_KEY and MSG91_TEMPLATE_DISPATCH_CREATED_DRIVER"
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("send failed: {e}");
            ExitCode::FAILURE
        }
    }
}
