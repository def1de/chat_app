use crate::AppState;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Redirect, Response},
};

#[allow(dead_code)]
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub username: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract session_token from Cookie header
        let token = parts
            .headers
            .get("cookie")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                s.split(';')
                    .find(|c| c.trim_start().starts_with("session_token="))
            })
            .map(|c| c.trim_start()[14..].to_string());

        if let Some(token) = token {
            if let Ok(Some((user_id, username))) = state.db.validate_session(&token).await {
                return Ok(AuthenticatedUser { user_id, username });
            }
        }
        Err(Redirect::to("/auth").into_response())
    }
}
