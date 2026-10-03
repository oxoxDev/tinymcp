//! The flow backed by a host's own secret store rather than the SQLite
//! [`Store`]: what a multi-tenant host needs to keep PKCE, registration and
//! refresh state in its own per-tenant secrets.

use super::*;
use crate::registry::OAuthCredentialStore;

/// A host's secret store: credentials in a map, keyed by whatever server id
/// the host chooses — here a tenant-qualified one.
#[derive(Debug, Default)]
struct HostSecrets {
    url: Option<String>,
    values: parking_lot::Mutex<BTreeMap<String, BTreeMap<String, String>>>,
    refuse_writes: std::sync::atomic::AtomicBool,
}

impl HostSecrets {
    fn remote(url: &str) -> Self {
        Self {
            url: Some(url.to_string()),
            ..Self::default()
        }
    }

    fn get(&self, server_id: &str, key: &str) -> Option<String> {
        self.values.lock().get(server_id)?.get(key).cloned()
    }
}

impl OAuthCredentialStore for HostSecrets {
    async fn remote_url(&self, _server_id: &str) -> crate::Result<Option<String>> {
        Ok(self.url.clone())
    }

    async fn load_credentials(&self, server_id: &str) -> crate::Result<BTreeMap<String, String>> {
        Ok(self
            .values
            .lock()
            .get(server_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn store_credentials(
        &self,
        server_id: &str,
        credentials: &BTreeMap<String, String>,
    ) -> crate::Result<()> {
        if self.refuse_writes.load(Ordering::SeqCst) {
            return Err(Error::CredentialStore {
                action: format!("writing credentials for {server_id}"),
                detail: "the vault is sealed".into(),
            });
        }
        self.values
            .lock()
            .insert(server_id.to_string(), credentials.clone());
        Ok(())
    }
}

const TENANT_SERVER: &str = "acme/linear";

#[tokio::test]
async fn a_host_store_backs_a_whole_sign_in() {
    let (endpoint, _state) = authority_with_challenge().await;
    let host = HostSecrets::remote(&endpoint);
    let flow = flow();

    let url = flow
        .begin(&host, TENANT_SERVER, "https://host.test/oauth/callback")
        .await
        .expect("begin");
    let state = authorize_param(&url, "state").unwrap();
    let server_id = flow.complete(&host, &state, "the-code").await.expect("complete");

    assert_eq!(server_id, TENANT_SERVER);
    assert_eq!(
        host.get(TENANT_SERVER, "Authorization").as_deref(),
        Some("Bearer at-1")
    );
    let bundle: Value =
        serde_json::from_str(&host.get(TENANT_SERVER, OAUTH_BUNDLE_KEY).unwrap()).unwrap();
    assert_eq!(bundle["refresh_token"], json!("rt-1"));
    assert_eq!(bundle["client_id"], json!("client-1"));
}

/// The redirect carries no session, so a multi-tenant host has to learn which
/// tenant's store to complete into from the `state` alone — before consuming
/// it.
#[tokio::test]
async fn the_server_a_pending_state_belongs_to_can_be_read_before_completing() {
    let (endpoint, _state) = authority_with_challenge().await;
    let host = HostSecrets::remote(&endpoint);
    let flow = flow();
    let url = flow
        .begin(&host, TENANT_SERVER, "https://host.test/oauth/callback")
        .await
        .unwrap();
    let state = authorize_param(&url, "state").unwrap();

    assert_eq!(flow.pending_server(&state).as_deref(), Some(TENANT_SERVER));
    assert_eq!(flow.pending_count(), 1, "peeking does not consume");
    assert_eq!(flow.pending_server("never-issued"), None);
}

#[tokio::test]
async fn a_host_store_backs_refresh() {
    let requests = TokenRequests::default();
    let base = serve(token_endpoint(
        requests.clone(),
        json!({ "access_token": "fresh", "expires_in": 3600 }),
    ))
    .await;
    let host = HostSecrets::default();
    let bundle = json!({
        "refresh_token": "r1",
        "client_id": "cli-1",
        "client_secret": null,
        "token_endpoint": format!("{base}/token"),
        "expires_at": 1,
    });
    host.values.lock().insert(
        TENANT_SERVER.to_string(),
        BTreeMap::from([(OAUTH_BUNDLE_KEY.to_string(), bundle.to_string())]),
    );

    let refreshed = refresh_if_expired(&host, &reqwest::Client::new(), TENANT_SERVER)
        .await
        .expect("refresh");

    assert!(refreshed);
    assert_eq!(
        host.get(TENANT_SERVER, "Authorization").as_deref(),
        Some("Bearer fresh")
    );
    assert_eq!(requests.count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_host_store_failure_is_reported_as_a_store_error() {
    let (endpoint, _state) = authority_with_challenge().await;
    let host = HostSecrets::remote(&endpoint);
    host.refuse_writes.store(true, Ordering::SeqCst);
    let flow = flow();
    let url = flow
        .begin(&host, TENANT_SERVER, "https://host.test/oauth/callback")
        .await
        .unwrap();
    let state = authorize_param(&url, "state").unwrap();

    let error = flow
        .complete(&host, &state, "the-code")
        .await
        .expect_err("a sealed vault");

    assert!(matches!(error, Error::CredentialStore { .. }), "{error:?}");
    assert_eq!(error.wire_name(), tinymcp_bus::errors::STORE);
    assert!(error.to_string().contains("sealed"), "{error}");
}

#[tokio::test]
async fn a_host_store_with_no_remote_endpoint_cannot_be_signed_in_to() {
    let error = flow()
        .begin(&HostSecrets::default(), TENANT_SERVER, "https://host.test/cb")
        .await
        .expect_err("no endpoint");
    assert!(matches!(error, Error::MalformedResponse { .. }), "{error:?}");
}
