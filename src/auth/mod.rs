pub mod basic;
pub mod oidc;

use crate::context::Context;
use crate::error::Error;
use async_trait::async_trait;

#[async_trait]
pub trait AuthProvider {
    async fn login(&self, ctx: Context) -> Result<Context, Error>;
}
