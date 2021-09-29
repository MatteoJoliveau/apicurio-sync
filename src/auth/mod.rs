pub mod oidc;

use async_trait::async_trait;
use crate::context::Context;
use crate::error::Error;

#[async_trait]
pub trait AuthProvider {
    async fn login(&self, ctx: Context) -> Result<Context, Error>;
}
