//! Free/Pro entitlements backed by Dodo Payments licence keys.
//!
//! * Uses Dodo's *public* licence endpoints (activate / validate /
//!   deactivate), which need no API key, so no merchant secret ships in the
//!   app. Sent: the licence key and a generic device label. Nothing about
//!   the user's files is ever sent.
//! * The activation is kept in the operating system's credential store.
//! * Pro stays active offline for `GRACE_SECONDS` after the last successful
//!   check. A licence problem only switches Pro features off; scanning and
//!   access to files never depend on it.
//! * Test mode only until the owner approves live checkout.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Live mode stays off until the owner approves it.
pub const LIVE_MODE_APPROVED: bool = false;
pub const GRACE_SECONDS: i64 = 30 * 86_400;
/// How often a still-valid licence is re-confirmed when online.
pub const REVALIDATE_AFTER_SECONDS: i64 = 3 * 86_400;
/// Dodo product id for Tessera Pro. Set once the product exists; while
/// `None`, any key from the merchant's test-mode products is accepted.
pub const PRO_PRODUCT_ID: Option<&str> = None;

pub fn base_url() -> &'static str {
    if LIVE_MODE_APPROVED {
        "https://live.dodopayments.com"
    } else {
        "https://test.dodopayments.com"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Feature {
    Cleanup,
    DeveloperStorage,
    History,
    Reports,
}

/// Which features need Pro. Kept in one place so tiers stay configurable.
pub fn requires_pro(f: Feature) -> bool {
    matches!(
        f,
        Feature::Cleanup | Feature::DeveloperStorage | Feature::History | Feature::Reports
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activation {
    pub key: String,
    pub instance_id: String,
    pub product_id: Option<String>,
    pub customer_email: Option<String>,
    pub mode: String,
    pub activated_at: i64,
    pub last_validated: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entitlement {
    pub pro: bool,
    /// Plain-language state for the interface.
    pub summary: String,
    pub customer_email: Option<String>,
    pub masked_key: Option<String>,
    pub test_mode: bool,
}

/// HTTP transport; real implementation uses HTTPS, tests use a fake.
pub trait Transport {
    /// Returns (status, JSON body or Null). `Err` means no response.
    fn post(&self, url: &str, body: &Value) -> Result<(u16, Value), String>;
}

/// Where the activation is kept.
pub trait Store {
    fn load(&self) -> Option<Activation>;
    fn save(&self, a: &Activation) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

fn mask(key: &str) -> String {
    let tail: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("•••• {tail}")
}

/// Entitlement implied by a stored activation at time `now`.
pub fn entitlement(a: Option<&Activation>, now: i64) -> Entitlement {
    let test_mode = !LIVE_MODE_APPROVED;
    match a {
        None => Entitlement {
            pro: false,
            summary: "Free".into(),
            customer_email: None,
            masked_key: None,
            test_mode,
        },
        Some(a) => {
            let age = now - a.last_validated;
            let pro = (0..=GRACE_SECONDS).contains(&age);
            let summary = if pro {
                "Pro".to_string()
            } else {
                "Pro licence needs to be confirmed. Connect to the internet and open Tessera again."
                    .to_string()
            };
            Entitlement {
                pro,
                summary,
                customer_email: a.customer_email.clone(),
                masked_key: Some(mask(&a.key)),
                test_mode,
            }
        }
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub struct Licensing<T: Transport, S: Store> {
    pub transport: T,
    pub store: S,
    pub device_label: String,
}

impl<T: Transport, S: Store> Licensing<T, S> {
    pub fn status(&self, now: i64) -> Entitlement {
        entitlement(self.store.load().as_ref(), now)
    }

    pub fn activate(&self, raw_key: &str, now: i64) -> Result<Entitlement, String> {
        let key = raw_key.trim();
        if !valid_key(key) {
            return Err(
                "That doesn't look like a licence key. Copy it exactly from your purchase email."
                    .into(),
            );
        }
        let url = format!("{}/licenses/activate", base_url());
        let (status, body) = self
            .transport
            .post(&url, &json!({ "license_key": key, "name": self.device_label }))
            .map_err(|_| "Tessera couldn't reach the licence server. Check your internet connection and try again.".to_string())?;
        match status {
            200 | 201 => {}
            403 => return Err("This licence key is not active. It may have been refunded or disabled.".into()),
            404 => return Err("That licence key wasn't found. Check it for typos.".into()),
            422 => return Err("This key has reached its activation limit. Deactivate it on another computer first.".into()),
            _ => return Err("The licence server had a problem. Please try again later.".into()),
        }
        let instance_id = body
            .get("id")
            .and_then(Value::as_str)
            .ok_or("The licence server sent an unexpected reply.")?;
        let product_id = body
            .pointer("/product/product_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(expected) = PRO_PRODUCT_ID
            && product_id.as_deref() != Some(expected)
        {
            return Err("This licence key is for a different product.".into());
        }
        let activation = Activation {
            key: key.to_string(),
            instance_id: instance_id.to_string(),
            product_id,
            customer_email: body
                .pointer("/customer/email")
                .and_then(Value::as_str)
                .map(str::to_string),
            mode: if LIVE_MODE_APPROVED {
                "live".into()
            } else {
                "test".into()
            },
            activated_at: now,
            last_validated: now,
        };
        self.store.save(&activation)?;
        Ok(entitlement(Some(&activation), now))
    }

    /// Re-confirms a stored licence if it is due. Network failures keep the
    /// current state (offline grace); a definite "not valid" removes it.
    pub fn refresh(&self, now: i64, force: bool) -> Entitlement {
        let Some(mut a) = self.store.load() else {
            return entitlement(None, now);
        };
        if !force && now - a.last_validated < REVALIDATE_AFTER_SECONDS {
            return entitlement(Some(&a), now);
        }
        let url = format!("{}/licenses/validate", base_url());
        match self.transport.post(
            &url,
            &json!({ "license_key": a.key, "license_key_instance_id": a.instance_id }),
        ) {
            Ok((200, body)) if body.get("valid").and_then(Value::as_bool) == Some(true) => {
                a.last_validated = now;
                let _ = self.store.save(&a);
                entitlement(Some(&a), now)
            }
            Ok((200, _)) | Ok((403, _)) | Ok((404, _)) => {
                let _ = self.store.clear();
                let mut e = entitlement(None, now);
                e.summary =
                    "Your Pro licence is no longer valid, so Tessera is back to Free.".into();
                e
            }
            _ => entitlement(Some(&a), now),
        }
    }

    pub fn deactivate(&self) -> Result<(), String> {
        let Some(a) = self.store.load() else {
            return Ok(());
        };
        let url = format!("{}/licenses/deactivate", base_url());
        match self.transport.post(
            &url,
            &json!({ "license_key": a.key, "license_key_instance_id": a.instance_id }),
        ) {
            Ok((200..=299, _)) | Ok((404, _)) => self.store.clear(),
            Ok(_) => Err(
                "The licence server refused to deactivate this computer. Please try again later."
                    .into(),
            ),
            Err(_) => Err(
                "Tessera couldn't reach the licence server, so this computer is still activated."
                    .into(),
            ),
        }
    }
}

/// HTTPS transport (rustls), 15-second timeout, no cookies, no redirects to
/// other hosts.
pub struct HttpsTransport {
    agent: ureq::Agent,
}

impl Default for HttpsTransport {
    fn default() -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(std::time::Duration::from_secs(15)))
            .build()
            .into();
        HttpsTransport { agent }
    }
}

impl Transport for HttpsTransport {
    fn post(&self, url: &str, body: &Value) -> Result<(u16, Value), String> {
        let mut resp = self
            .agent
            .post(url)
            .send_json(body)
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let value = resp.body_mut().read_json::<Value>().unwrap_or(Value::Null);
        Ok((status, value))
    }
}

/// Activation kept in Windows Credential Manager / macOS Keychain.
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new(service: &str) -> Self {
        KeyringStore {
            service: service.to_string(),
        }
    }
    fn entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(&self.service, "pro-licence").map_err(|e| e.to_string())
    }
}

impl Store for KeyringStore {
    fn load(&self) -> Option<Activation> {
        let secret = self.entry().ok()?.get_password().ok()?;
        serde_json::from_str(&secret).ok()
    }
    fn save(&self, a: &Activation) -> Result<(), String> {
        let text = serde_json::to_string(a).map_err(|e| e.to_string())?;
        self.entry()?
            .set_password(&text)
            .map_err(|_| "Tessera couldn't save the licence securely on this computer.".to_string())
    }
    fn clear(&self) -> Result<(), String> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Simulated Dodo server: a queue of canned responses. Mock only; this
    /// does not prove the real integration.
    struct FakeDodo {
        replies: RefCell<Vec<Result<(u16, Value), String>>>,
        calls: RefCell<Vec<(String, Value)>>,
    }
    impl FakeDodo {
        fn new(replies: Vec<Result<(u16, Value), String>>) -> Self {
            FakeDodo {
                replies: RefCell::new(replies),
                calls: RefCell::new(Vec::new()),
            }
        }
    }
    impl Transport for FakeDodo {
        fn post(&self, url: &str, body: &Value) -> Result<(u16, Value), String> {
            self.calls
                .borrow_mut()
                .push((url.to_string(), body.clone()));
            self.replies.borrow_mut().remove(0)
        }
    }
    #[derive(Default)]
    struct MemStore(RefCell<Option<Activation>>);
    impl Store for MemStore {
        fn load(&self) -> Option<Activation> {
            self.0.borrow().clone()
        }
        fn save(&self, a: &Activation) -> Result<(), String> {
            *self.0.borrow_mut() = Some(a.clone());
            Ok(())
        }
        fn clear(&self) -> Result<(), String> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    fn lic(replies: Vec<Result<(u16, Value), String>>) -> Licensing<FakeDodo, MemStore> {
        Licensing {
            transport: FakeDodo::new(replies),
            store: MemStore::default(),
            device_label: "Tessera on Windows".into(),
        }
    }
    fn ok_activation() -> Result<(u16, Value), String> {
        Ok((
            201,
            json!({ "id": "lki_1", "product": { "product_id": "pdt_1" }, "customer": { "email": "a@example.com" } }),
        ))
    }
    const DAY: i64 = 86_400;

    #[test]
    fn activation_grants_pro_and_sends_only_key_and_label() {
        let l = lic(vec![ok_activation()]);
        let e = l.activate("  ABC-123  ", 1000).unwrap();
        assert!(e.pro);
        assert_eq!(e.masked_key.as_deref(), Some("•••• -123"));
        let calls = l.transport.calls.borrow();
        assert!(
            calls[0]
                .0
                .starts_with("https://test.dodopayments.com/licenses/activate")
        );
        assert_eq!(
            calls[0].1,
            json!({ "license_key": "ABC-123", "name": "Tessera on Windows" })
        );
    }

    #[test]
    fn activation_errors_are_explained_and_store_nothing() {
        for status in [403, 404, 422, 500] {
            let l = lic(vec![Ok((status, Value::Null))]);
            assert!(l.activate("KEY", 0).is_err());
            assert!(l.store.load().is_none());
        }
        let l = lic(vec![Err("offline".into())]);
        assert!(l.activate("KEY", 0).unwrap_err().contains("internet"));
        assert!(
            lic(vec![]).activate("bad key with spaces", 0).is_err(),
            "rejected before any request"
        );
    }

    #[test]
    fn offline_grace_then_free_without_touching_anything_else() {
        let l = lic(vec![
            ok_activation(),
            Err("offline".into()),
            Err("offline".into()),
        ]);
        l.activate("KEY", 0).unwrap();
        assert!(l.refresh(10 * DAY, false).pro, "within grace while offline");
        let e = l.refresh(31 * DAY, false);
        assert!(!e.pro, "grace over");
        assert!(
            l.store.load().is_some(),
            "activation kept for when the computer is online again"
        );
    }

    #[test]
    fn validation_renews_and_revocation_returns_to_free() {
        let l = lic(vec![
            ok_activation(),
            Ok((200, json!({ "valid": true }))),
            Ok((200, json!({ "valid": false }))),
        ]);
        l.activate("KEY", 0).unwrap();
        assert!(l.refresh(5 * DAY, false).pro);
        assert_eq!(l.store.load().unwrap().last_validated, 5 * DAY);
        let e = l.refresh(9 * DAY, false);
        assert!(!e.pro);
        assert!(l.store.load().is_none());
    }

    #[test]
    fn recent_validation_is_not_repeated() {
        let l = lic(vec![ok_activation()]);
        l.activate("KEY", 0).unwrap();
        assert!(l.refresh(DAY, false).pro);
        assert_eq!(
            l.transport.calls.borrow().len(),
            1,
            "no network call within the revalidation window"
        );
    }

    #[test]
    fn deactivation_clears_only_after_the_server_agrees() {
        let l = lic(vec![
            ok_activation(),
            Err("offline".into()),
            Ok((200, Value::Null)),
        ]);
        l.activate("KEY", 0).unwrap();
        assert!(l.deactivate().is_err());
        assert!(l.store.load().is_some());
        l.deactivate().unwrap();
        assert!(l.store.load().is_none());
    }

    #[test]
    fn tiers_are_configurable_in_one_place() {
        assert!(requires_pro(Feature::Cleanup));
        assert!(
            !LIVE_MODE_APPROVED,
            "live payments must stay off until approved"
        );
    }
}
