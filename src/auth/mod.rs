use async_trait::async_trait;

use crate::context::Context;
use crate::error::Error;

pub mod basic;
pub mod oidc;

#[async_trait]
pub trait AuthProvider {
    async fn login(&self, ctx: Context) -> Result<Context, Error>;
}
