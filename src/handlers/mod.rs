use crate::auth::AuthenticatedUser;
use crate::AppState;
use askama::Template;
use axum::extract::{Json, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::Form;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct StatusResponse {
    connected_clients: usize,
}

pub async fn index(State(mut state): State<AppState>, user: AuthenticatedUser) -> Response {
    let chats = match state.db.get_user_chats(user.user_id).await {
        Ok(chats) => chats,
        Err(_) => {
            return "Error: failed to fetch chats".into_response();
        }
    };
    let template = crate::template::IndexTemplate {
        username: &user.username,
        chats: chats
            .into_iter()
            .map(|(id, name)| crate::template::ChatView { id, name })
            .collect(),
    };
    match template.render() {
        Ok(body) => Html(body).into_response(),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, "Template render error").into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct NewChatPayload {
    pub chat_name: String,
}

pub async fn newchat(
    State(mut state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<NewChatPayload>,
) -> Response {
    println!(
        "Creating new chat: {} for user: {}",
        payload.chat_name, user.username
    );
    match state.db.create_chat(&payload.chat_name, user.user_id).await {
        Ok(id) => println!("Created new chat with id: {}", id),
        Err(e) => {
            eprintln!("Error creating chat: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create chat").into_response();
        }
    };
    StatusCode::CREATED.into_response()
}

pub async fn chat(
    State(mut state): State<AppState>,
    Path(chat_id): Path<i64>,
    user: AuthenticatedUser,
) -> Response {
    let is_accessible = match state.db.check_chat_membership(user.user_id, chat_id).await {
        Ok(is_accessible) => is_accessible,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Server Error: {}", e),
            )
                .into_response()
        }
    };

    if !is_accessible {
        return (StatusCode::FORBIDDEN, "You are not a member of this chat").into_response();
    }

    let mut msgs = state.db.get_messages(chat_id, 50).await.unwrap_or_default();
    let chats = state.db.get_user_chats(user.user_id).await.unwrap();
    msgs.reverse();
    let template = crate::template::ChatTemplate {
        username: &user.username,
        messages: msgs,
        chats: chats
            .into_iter()
            .map(|(id, name)| crate::template::ChatView { id, name })
            .collect(),
    };
    match template.render() {
        Ok(body) => Html(body).into_response(),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, "Template render error").into_response(),
    }
}

pub async fn invite(
    State(mut state): State<AppState>,
    Path(code): Path<String>,
    user: AuthenticatedUser,
) -> Response {
    match state.db.get_chat_id_by_invite_code(&code).await {
        Ok(Some(chat_id)) => {
            // Check if user is already a member
            match state.db.check_chat_membership(user.user_id, chat_id).await {
                Ok(true) => return Redirect::to(&format!("/chat/{}", chat_id)).into_response(),
                Ok(false) => (),
                Err(e) => {
                    eprintln!("Error checking chat membership: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to process invite",
                    )
                        .into_response();
                }
            }
            // Add user to chat
            match state.db.add_user_to_chat(user.user_id, chat_id).await {
                Ok(_) => Redirect::to(&format!("/chat/{}", chat_id)).into_response(),
                Err(e) => {
                    eprintln!("Error adding user to chat: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to join chat").into_response()
                }
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Invalid invite code").into_response(),
        Err(e) => {
            eprintln!("Error retrieving chat by invite code: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process invite",
            )
                .into_response()
        }
    }
}

pub async fn create_invite(
    State(mut state): State<AppState>,
    Path(chat_id): Path<i64>,
    user: AuthenticatedUser,
) -> Response {
    // Check if user is a member of the chat
    if state
        .db
        .check_chat_membership(user.user_id, chat_id)
        .await
        .unwrap_or(false)
        == false
    {
        return (StatusCode::FORBIDDEN, "You are not a member of this chat").into_response();
    }

    // Generate invite code
    let invite_code = Uuid::new_v4().to_string();

    match state.db.create_invite_code(chat_id, &invite_code).await {
        Ok(_) => (StatusCode::CREATED, Json(json!({ "code": invite_code }))).into_response(),
        Err(e) => {
            eprintln!("Error creating invite code: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create invite").into_response()
        }
    }
}

pub async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    let response = StatusResponse {
        connected_clients: state.get_connected_clients(),
    };
    Json(response)
}

pub async fn auth_get() -> Html<&'static str> {
    Html(std::include_str!("../../templates/auth.html"))
}

#[derive(serde::Deserialize)]
pub struct AuthForm {
    username: String,
    password: String,
}

pub async fn auth_post(State(mut state): State<AppState>, Form(form): Form<AuthForm>) -> Response {
    // Extract username and password from the form
    let username = form.username.as_str();
    let password = form.password.as_str();
    // Hash the password
    let hash = format!("{:x}", Sha256::digest(password.as_bytes()));

    {
        match state.db.get_user(username).await {
            Ok(Some(_)) => {
                // User exists, check password
                if state.db.check_password(username, &hash).await {
                    return start_session(&mut state, &username).await;
                } else {
                    return Html("<p>Invalid password, perhaps user already exists, under a different password?
                    <a href=\"/auth\">Try again</a>
                    </p>".to_string()).into_response();
                }
            }
            Ok(None) => {
                // User not found, register new user
                state.db.add_user(username, &hash).await.unwrap();
                // Repeat the check to authorize the new user
                if state.db.check_password(username, &hash).await {
                    return start_session(&mut state, &username).await;
                } else {
                    return Html("<p>Invalid password, perhaps user already exists, under a different password?
                    <a href=\"/auth\">Try again</a>
                    </p>".to_string()).into_response();
                }
            }
            Err(e) => return Html(format!("<p>Error: {}</p>", e)).into_response(),
        }
    }
}

fn generate_session_token() -> String {
    Uuid::new_v4().to_string()
}

async fn start_session(state: &mut AppState, username: &str) -> Response {
    let session_token = generate_session_token();
    let user_id = match state
        .db
        .get_user(username)
        .await
        .ok()
        .and_then(|opt| opt.map(|(id, _)| id))
    {
        Some(id) => id,
        None => return Html("<p>Invalid credentials</p>".to_string()).into_response(),
    };

    if state
        .db
        .create_session(user_id, &session_token)
        .await
        .is_ok()
    {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Set-Cookie",
            HeaderValue::from_str(&format!(
                "session_token={}; HttpOnly; Path=/",
                session_token
            ))
            .unwrap(),
        );
        return (headers, Redirect::to("/")).into_response();
    }
    Html("<p>Invalid credentials</p>".to_string()).into_response()
}

pub async fn logout(State(mut state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    // Extract and delete session
    if let Some(cookie_header) = headers.get("cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if cookie.starts_with("session_token=") {
                    let token = &cookie[14..];
                    let _ = state.db.delete_session(token);
                    break;
                }
            }
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        "Set-Cookie",
        HeaderValue::from_str("session_token=; HttpOnly; Path=/; Max-Age=0").unwrap(),
    );
    (headers, Redirect::to("/auth"))
}
