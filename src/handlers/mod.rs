use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::extract::{State, Path, Json};
use axum::Form;
use crate::AppState;
use crate::auth::AuthenticatedUser;
use sha2::{Sha256, Digest};
use uuid::Uuid;
use askama::Template;
use serde_json::json;

#[derive(serde::Serialize)]
pub struct StatusResponse {
    connected_clients: usize,
}

pub async fn index(State(state): State<AppState>, user: AuthenticatedUser) -> Response {
    let chats = state.db_action().get_user_chats(user.user_id).unwrap_or_default();
    let template = crate::template::IndexTemplate {
        username: &user.username,
        chats: chats.into_iter().map(|(id, name)| crate::template::ChatView { id, name }).collect(),
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

pub async fn newchat(State(state): State<AppState>, user: AuthenticatedUser, Json(payload): Json<NewChatPayload>) -> Response {
    println!("Creating new chat: {} for user: {}", payload.chat_name, user.username);
    match state.db_action().create_chat(&payload.chat_name, user.user_id) {
        Ok(id) => println!("Created new chat with id: {}", id),
        Err(e) => {
            eprintln!("Error creating chat: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create chat").into_response();
        },
    };
    StatusCode::CREATED.into_response()
}

pub async fn chat(State(state): State<AppState>, Path(chat_id): Path<i64>, user: AuthenticatedUser) -> Response {
    if state.db_action().check_chat_membership(user.user_id, chat_id).unwrap_or(false) == false {
        return (StatusCode::FORBIDDEN, "You are not a member of this chat").into_response();
    }

    let mut msgs = state.db_action().get_messages(chat_id, 50).unwrap_or_default();
    let chats = state.db_action().get_user_chats(user.user_id).unwrap();
    msgs.reverse();
    let template = crate::template::ChatTemplate {
        username: &user.username,
        messages: msgs,
        chats: chats.into_iter().map(|(id, name)| crate::template::ChatView { id, name }).collect(),
    };
    match template.render() {
        Ok(body) => Html(body).into_response(),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, "Template render error").into_response(),
    }
}

pub async fn invite(State(state): State<AppState>, Path(code): Path<String>, user: AuthenticatedUser) -> Response {
    match state.db_action().get_chat_id_by_invite_code(&code) {
        Ok(Some(chat_id)) => {
            // Check if user is already a member
            match state.db_action().check_chat_membership(user.user_id, chat_id) {
                Ok(true) => return Redirect::to(&format!("/chat/{}", chat_id)).into_response(),
                Ok(false) => (),
                Err(e) => {
                    eprintln!("Error checking chat membership: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to process invite").into_response();
                }
            }
            // Add user to chat
            match state.db_action().add_user_to_chat(user.user_id, chat_id) {
                Ok(_) => Redirect::to(&format!("/chat/{}", chat_id)).into_response(),
                Err(e) => {
                    eprintln!("Error adding user to chat: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to join chat").into_response()
                }
            }
        },
        Ok(None) => (StatusCode::NOT_FOUND, "Invalid invite code").into_response(),
        Err(e) => {
            eprintln!("Error retrieving chat by invite code: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to process invite").into_response()
        }
    }
}

pub async fn create_invite(State(state): State<AppState>, Path(chat_id): Path<i64>, user: AuthenticatedUser) -> Response {
    // Check if user is a member of the chat
    if state.db_action().check_chat_membership(user.user_id, chat_id).unwrap_or(false) == false {
        return (StatusCode::FORBIDDEN, "You are not a member of this chat").into_response();
    }

    // Generate invite code
    let invite_code = Uuid::new_v4().to_string();

    match state.db_action().create_invite_code(chat_id, &invite_code) {
        Ok(_) => {
            (StatusCode::CREATED, Json(json!({ "code": invite_code }))).into_response()
        },
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

pub async fn auth_post(
    State(state): State<AppState>,
    Form(form): Form<AuthForm>
) -> Response {

    // Extract username and password from the form
    let username = form.username.as_str();
    let password = form.password.as_str();
    // Hash the password
    let hash = format!("{:x}", Sha256::digest(password.as_bytes()));

    {
        match state.db_action().get_user(username) {
            Ok(Some(_)) => {
                // User exists, check password
                if state.db_action().check_password(username, &hash) {
                    return start_session(&state, &username)
                } else {
                    return Html("<p>Invalid password, perhaps user already exists, under a different password?
                    <a href=\"/auth\">Try again</a>
                    </p>".to_string()).into_response()
                }
            }
            Ok(None) => {
                // User not found, register new user
                state.db_action().add_user(username, &hash).unwrap();
                // Repeat the check to authorize the new user
                if state.db_action().check_password(username, &hash) {
                    return start_session(&state, &username)
                } else {
                    return Html("<p>Invalid password, perhaps user already exists, under a different password?
                    <a href=\"/auth\">Try again</a>
                    </p>".to_string()).into_response()
                }
            }
            Err(e) => {
                return Html(format!("<p>Error: {}</p>", e)).into_response()
            }
        }
    }
}

fn generate_session_token() -> String {
    Uuid::new_v4().to_string()
}

fn start_session(state: &AppState, username: &str) -> Response {
    let session_token = generate_session_token();
                let user_id = match state.db_action().get_user(username).ok().and_then(|opt| opt.map(|(id, _)| id)) {
                    Some(id) => id,
                    None => return Html("<p>Invalid credentials</p>".to_string()).into_response(),
                };
                
                if state.db_action().create_session(user_id, &session_token).is_ok() {
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        "Set-Cookie",
                        HeaderValue::from_str(&format!("session_token={}; HttpOnly; Path=/", session_token)).unwrap()
                    );
                    return (headers, Redirect::to("/")).into_response();
                }
                Html("<p>Invalid credentials</p>".to_string()).into_response()
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap
) -> impl IntoResponse {
    // Extract and delete session
    if let Some(cookie_header) = headers.get("cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if cookie.starts_with("session_token=") {
                    let token = &cookie[14..];
                    let _ = state.db_action().delete_session(token);
                    break;
                }
            }
        }
    }
    
    let mut headers = HeaderMap::new();
    headers.insert(
        "Set-Cookie",
        HeaderValue::from_str("session_token=; HttpOnly; Path=/; Max-Age=0").unwrap()
    );
    (headers, Redirect::to("/auth"))
}