mod websocket;
mod handlers;
mod database;
mod auth;
mod template;

use axum::Router;
use tower_http::services::ServeDir;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::sync::mpsc;
use axum::extract::ws::Message;

use websocket::{chatsocket_handler};
use handlers::*;
use database::Database;

pub struct SocketData {
    pub chat_id: i64,
    pub socket: mpsc::UnboundedSender<Message>,
}

#[derive(Clone)]
pub struct AppState {
    sockets: Arc<Mutex<HashMap<String, SocketData>>>,
    db: Database,
}

impl AppState {
    pub fn new() -> Self {
        let database: Database = Database::new("database.db");
        match database.create() {
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

    pub fn db_action(&self) -> Database {
        self.db.clone()
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
        .route("/create_invite/:chat_id", axum::routing::post(create_invite))
        .route("/status", axum::routing::get(status))
        .route("/auth", axum::routing::get(auth_get).post(auth_post))
        .route("/logout", axum::routing::post(logout))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:1578").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}