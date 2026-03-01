mod auth;
mod database;
mod handlers;
mod template;
mod websocket;

use axum::extract::ws::Message;
use axum::Router;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tower_http::services::ServeDir;

use database::Database;
use handlers::*;
use websocket::chatsocket_handler;

pub struct SocketData {
    pub chat_id: i64,
    pub socket: mpsc::UnboundedSender<Message>,
}

#[derive(Clone)]
pub struct AppState {
    sockets: Arc<Mutex<HashMap<String, SocketData>>>,
    pub db: Database,
}

impl AppState {
    pub async fn new() -> Self {
        let db_path = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:database.db".to_string());
        let database: Database = Database::new(&db_path).await;
        match database.create().await {
            Ok(_) => println!("Database schema created successfully."),
            Err(e) => panic!("Error creating database schema: {}", e),
        }
        AppState {
            sockets: Arc::new(Mutex::new(HashMap::new())),
            db: database,
        }
    }

    pub fn get_connected_clients(&self) -> usize {
        let sockets = self.sockets.lock().unwrap();
        sockets.len()
    }
}

#[tokio::main]
async fn main() {
    let state = AppState::new();

    let app = Router::new()
        .route("/", axum::routing::get(index))
        .route("/chat/:id", axum::routing::get(chat))
        .route("/chatsocket/:id", axum::routing::get(chatsocket_handler))
        .route("/newchat", axum::routing::post(newchat))
        .route("/invite/:code", axum::routing::get(invite))
        .route(
            "/create_invite/:chat_id",
            axum::routing::post(create_invite),
        )
        .route("/status", axum::routing::get(status))
        .route("/auth", axum::routing::get(auth_get).post(auth_post))
        .route("/logout", axum::routing::post(logout))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state.await);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:1578").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
