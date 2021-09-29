use crate::auth::AuthProvider;
use crate::context::{Auth, Context};
use crate::error::Error;
use async_trait::async_trait;

pub struct BasicAuthProvider {
    username: String,
    password: Option<String>,
}

impl BasicAuthProvider {
    pub fn new(username: impl ToString, password: Option<impl ToString>) -> Self {
        Self {
            username: username.to_string(),
            password: password.map(|pwd| pwd.to_string()),
        }
    }
}

#[async_trait]
impl AuthProvider for BasicAuthProvider {
    async fn login(&self, mut ctx: Context) -> Result<Context, Error> {
        ctx.set_auth(Auth::Basic {
            username: self.username.clone(),
            password: self.password.clone(),
        });
        Ok(ctx)
    }
}
