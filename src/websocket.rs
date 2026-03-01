use crate::auth::AuthenticatedUser;
use crate::{AppState, SocketData};
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures_util::{sink::SinkExt, stream::StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

pub async fn chatsocket_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(mut state): axum::extract::State<AppState>,
    Path(chat_id): Path<i64>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    if state
        .db
        .check_chat_membership(user.user_id, chat_id)
        .await
        .unwrap_or(false)
        == false
    {
        return (StatusCode::FORBIDDEN, "You are not a member of this chat").into_response();
    }

    let username = user.username;
    ws.on_upgrade(move |socket| handle_socket(socket, state, chat_id, username))
}

async fn handle_socket(socket: WebSocket, mut state: AppState, chat_id: i64, username: String) {
    let socket_id = Uuid::new_v4().to_string(); // Unique ID for each socket
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Store the socket in the shared state
    {
        let mut sockets = state.sockets.lock().unwrap();
        let socket_data = SocketData {
            chat_id,
            socket: tx,
        };
        sockets.insert(socket_id.clone(), socket_data);
    }
    println!("New client connected with id: {}", socket_id);

    // Spawn a task to handle outgoing messages
    let socket_id_clone = socket_id.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
        println!("Disconnecting a client with id: {}", socket_id_clone);
        // Remove the socket from a HashMap when the connection is closed
        let mut sockets = state_clone.sockets.lock().unwrap();
        sockets.remove(&socket_id_clone);
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                match state.db.insert_message(&text, &username, chat_id).await {
                    Ok(_) => (),
                    Err(e) => {
                        eprintln!("Failed to inserte a message: {}", e);
                        continue;
                    }
                };

                let broadcast_message = format!("{}: {}", username, text);

                let sockets = state.sockets.lock().unwrap();
                for (id, socket_data) in sockets.iter() {
                    if id != &socket_id {
                        if socket_data.chat_id != chat_id {
                            continue; // Skip if chat IDs do not match
                        }
                        let sender = &socket_data.socket;
                        let _ = sender.send(Message::Text(broadcast_message.clone()));
                    }
                }
            }
            Message::Close(_) => {
                println!("Client {} disconnected", socket_id);
                break;
            }
            _ => {}
        }
    }

    // Cleanup
    let mut sockets = state.sockets.lock().unwrap();
    sockets.remove(&socket_id);
}
