use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Add;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use http::StatusCode;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RequestTokenError, Scope,
    StandardErrorResponse,
};
use serde::Deserialize;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, RwLock};
use url::Url;
use warp::reply::Html;
use warp::{Filter, Reply};

use crate::auth::AuthProvider;
use crate::context::{Auth, Context};
use crate::error::Error;

#[derive(Debug, Clone)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct OidcProvider {
    issuer_url: String,
    client_id: String,
    client: CoreClient,
    tokens: Option<TokenSet>,
    port: u16,
}

impl OidcProvider {
    pub async fn new(
        issuer_url: impl ToString,
        client_id: impl ToString,
        port: u16,
    ) -> Result<Self, Error> {
        let metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(issuer_url.to_string())?,
            openidconnect::reqwest::async_http_client,
        )
        .await
        .map_err(|err| Error::Auth(err.into()))?;
        Ok(Self {
            issuer_url: issuer_url.to_string(),
            client_id: client_id.to_string(),
            client: CoreClient::from_provider_metadata(
                metadata,
                ClientId::new(client_id.to_string()),
                None,
            )
            .set_redirect_uri(RedirectUrl::new(format!(
                "http://localhost:{}/callback",
                port
            ))?),
            tokens: None,
            port,
        })
    }
}

#[async_trait]
impl AuthProvider for OidcProvider {
    async fn login(&self, mut ctx: Context) -> Result<Context, Error> {
        // Generate the full authorization URL.
        let (auth_url, csrf_token, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            // Set the desired scopes.
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("email".to_string()))
            // .add_scope(Scope::new("groups".to_string()))
            .url();

        let this = Arc::new(RwLock::new(self.clone()));
        let (tx, mut rx) = mpsc::channel(1);
        let app = warp::get()
            .and(warp::path("callback"))
            .and(with_provider(this.clone()))
            .and(with_shutdown_signal(tx))
            .and(warp::query::query::<CallbackQuery>())
            .and_then(callback_handler);

        let (_addr, server) = warp::serve(app).bind_with_graceful_shutdown(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), self.port),
            async move {
                rx.recv().await.expect("shutdown::recv");
            },
        );

        open::that(&auth_url.to_string())?;
        eprintln!("The login page has been opened on your default browser. You can also manually visit {}", auth_url);
        server.await;

        let this = this.read().await;
        let tokens = this.tokens.as_ref().unwrap();
        ctx.set_auth(Auth::Oidc {
            issuer_url: this.issuer_url.clone(),
            client_id: this.client_id.clone(),
            access_token: tokens.access_token.clone(),
            refresh_token: tokens.refresh_token.clone(),
            expires_at: tokens.expires_at.clone(),
        });
        Ok(ctx)
    }
}

fn with_provider(
    provider: Arc<RwLock<OidcProvider>>,
) -> impl Filter<Extract = (Arc<RwLock<OidcProvider>>,), Error = Infallible> + Clone {
    warp::any().map(move || provider.clone())
}

fn with_shutdown_signal(
    tx: Sender<()>,
) -> impl Filter<Extract = (Sender<()>,), Error = Infallible> + Clone {
    warp::any().map(move || tx.clone())
}

async fn callback_handler(
    provider: Arc<RwLock<OidcProvider>>,
    tx: Sender<()>,
    CallbackQuery { code, state }: CallbackQuery,
) -> Result<impl Reply, warp::Rejection> {
    let mut provider = provider.write().await;
    let token_response = provider
        .client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(openidconnect::reqwest::async_http_client)
        .await;
    if let Err(err) = token_response {
        let msg = match &err {
            RequestTokenError::ServerResponse(res) => res.to_string(),
            RequestTokenError::Request(inner) => inner.to_string(),
            RequestTokenError::Parse(inner, _) => inner.to_string(),
            RequestTokenError::Other(_) => "".to_string(),
        };
        eprintln!("ERROR: {} {}", err, msg);
        tx.send(()).await.expect("shutdown::send");
        return Ok(warp::reply::with_status(
            warp::reply::html(format!(
                r#"
        <h1>ERROR</h1>
        <h2>{}</h2>
        <p>{}</p>
"#,
                err, msg
            )),
            StatusCode::BAD_REQUEST,
        ));
    }

    let token_response = token_response.unwrap();
    provider.tokens = Some(TokenSet {
        access_token: token_response.access_token().secret().clone(),
        refresh_token: token_response
            .refresh_token()
            .map(|token| token.secret().clone()),
        expires_at: Utc::now().add(
            token_response
                .expires_in()
                .map(|duration| Duration::from_std(duration).expect("Duration::from_std"))
                .unwrap_or_else(|| Duration::seconds(0)),
        ),
    });
    tx.send(()).await.expect("shutdown::send");
    Ok(warp::reply::with_status(
        warp::reply::html(
            "<h1>Authentication completed!</h1><p>You can close this window now.</p>".to_string(),
        ),
        StatusCode::OK,
    ))
}
#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}
