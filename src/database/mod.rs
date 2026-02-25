use sqlite;
use std::sync::{Arc, Mutex};
use crate::template::MessageView;

// Module to run tests on the database
mod test;
pub struct Database {
    connection: Arc<Mutex<sqlite::Connection>>,
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Database {
            connection: Arc::clone(&self.connection),
        }
    }
}

impl Database {
    pub fn new(path: &str) -> Self {
        let conn = match sqlite::open(path) {
            Ok(conn) => conn,
            Err(e) => panic!("Error opening database: {}", e),
        };

        conn.execute("PRAGMA foreign_keys = ON;").ok();

        Database { 
            connection: Arc::new(Mutex::new(conn)),
        }
    }

    pub fn create(&self) -> Result<(), sqlite::Error> {
        println!("Creating database schema...");
        self.connection.lock().unwrap().execute(
            "
            CREATE TABLE IF NOT EXISTS Messages (
                messageID INTEGER PRIMARY KEY,
                message_text TEXT NOT NULL,
                username TEXT NOT NULL,
                chatID INTEGER NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(chatID) REFERENCES Chats(chatID) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS Users (
                userID INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                password_hash TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS Sessions (
                sessionID INTEGER PRIMARY KEY,
                userID INTEGER NOT NULL,
                session_token TEXT NOT NULL,
                expires_at DATETIME NOT NULL,
                FOREIGN KEY(userID) REFERENCES Users(userID)
            );
            CREATE TABLE IF NOT EXISTS Chats (
                chatID INTEGER PRIMARY KEY,
                chat_name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS ChatMembers (
                chatID INTEGER NOT NULL,
                userID INTEGER NOT NULL,
                PRIMARY KEY (chatID, userID),
                FOREIGN KEY(chatID) REFERENCES Chats(chatID) ON DELETE CASCADE,
                FOREIGN KEY(userID) REFERENCES Users(userID) ON DELETE CASCADE
            ) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS InviteCodes (
                code TEXT PRIMARY KEY,
                chatID INTEGER NOT NULL,
                expires_at DATETIME NOT NULL,
                FOREIGN KEY(chatID) REFERENCES Chats(chatID) ON DELETE CASCADE
            );
            ",
        )
    }

    pub fn insert_message(&self, message_text: &str, username: &str, chat_id: i64) -> Result<(), sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT INTO Messages (message_text, username, chatID) VALUES (?, ?, ?);"
        )?;
        stmt.bind((1, message_text))?;
        stmt.bind((2, username))?;
        stmt.bind((3, chat_id))?;
        stmt.next()?;
        Ok(())
    }

        pub fn get_messages(&self, chat_id:i64, limit: i64) -> Result<Vec<MessageView>, sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT m.username, m.message_text
                        FROM Messages AS m
                        JOIN Chats AS c ON c.chatID = m.chatID
                        WHERE c.chatID = ?
                        ORDER BY timestamp DESC LIMIT ?;"
        )?;
        stmt.bind((1, chat_id))?;
        stmt.bind((2, limit))?;
        
        let mut messages = Vec::new();
        while let sqlite::State::Row = stmt.next()? {
            let username: String = stmt.read(0)?;
            let message_text: String = stmt.read(1)?;
            messages.push(MessageView { username, text: message_text });
        }
        Ok(messages)
    }

    pub fn get_user(&self, username: &str) -> Result<Option<(i64, String)>, sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT userID, username FROM Users WHERE username = ?;"
        )?;
        stmt.bind((1, username))?;
        if let sqlite::State::Row = stmt.next()? {
            let user_id: i64 = stmt.read(0)?;
            let username: String = stmt.read(1)?;
            Ok(Some((user_id, username)))
        } else {
            Ok(None)
        }
    }

    pub fn check_password(&self, username: &str, password_hash: &str) -> bool {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT password_hash FROM Users WHERE username = ?;"
        ).unwrap();

        // Return false if binding fails
        if stmt.bind((1, username)).is_err() {
            return false;
        }

        if let sqlite::State::Row = stmt.next().unwrap_or(sqlite::State::Done) {
            let stored_hash: String = stmt.read(0).unwrap_or_default();
            stored_hash == password_hash
        } else {
            false
        }
    }

    pub fn add_user(&self, username: &str, password_hash: &str) -> Result<(), sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT INTO Users (username, password_hash) VALUES (?, ?);"
        )?;
        stmt.bind((1, username))?;
        stmt.bind((2, password_hash))?;
        stmt.next()?;
        Ok(())
    }

    pub fn create_session(&self, user_id: i64, session_token: &str) -> Result<(), sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT INTO Sessions (userID, session_token, expires_at) VALUES (?, ?, datetime('now', '+7 days'));"
        )?;
        stmt.bind((1, user_id))?;
        stmt.bind((2, session_token))?;
        stmt.next()?;
        Ok(())
    }

    pub fn validate_session(&self, session_token: &str) -> Result<Option<(i64, String)>, sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.userID, u.username
                        FROM Sessions AS s
                        JOIN Users AS u ON u.userID = s.userID
                        WHERE session_token = ? AND expires_at > datetime('now');"
        )?;
        stmt.bind((1, session_token))?;
        if let sqlite::State::Row = stmt.next()? {
            let user_id: i64 = stmt.read(0)?;
            let username: String = stmt.read(1)?;
            Ok(Some((user_id, username)))
        } else {
            Ok(None)
        }
    }

    pub fn delete_session(&self, session_token: &str) -> Result<(), sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare("DELETE FROM Sessions WHERE session_token = ?;")?;
        stmt.bind((1, session_token))?;
        stmt.next()?;
        Ok(())
    }

    pub fn check_chat_membership(&self, user_id: i64, chat_id: i64) -> Result<bool, sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 1 FROM ChatMembers WHERE userID = ? AND chatID = ?;"
        )?;
        stmt.bind((1, user_id))?;
        stmt.bind((2, chat_id))?;
        if let sqlite::State::Row = stmt.next()? {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_user_chats(&self, user_id: i64) -> Result<Vec<(i64, String)>, sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT c.chatID, c.chat_name
                        FROM Chats AS c
                        JOIN ChatMembers AS cm ON cm.chatID = c.chatID
                        WHERE cm.userID = ?;"
        )?;
        stmt.bind((1, user_id))?;

        let mut chats = Vec::new();
        while let sqlite::State::Row = stmt.next()? {
            let chat_id: i64 = stmt.read(0)?;
            let chat_name: String = stmt.read(1)?;
            chats.push((chat_id, chat_name));
        }
        Ok(chats)
    }

    pub fn create_chat(&self, chat_name: &str, user_id: i64) -> Result<i64, sqlite::Error> {
        let conn = self.connection.lock().unwrap();

        let chat_id: i64 = {
            let mut stmt = conn.prepare("INSERT INTO Chats (chat_name) VALUES (?) RETURNING chatID;")?;
            stmt.bind((1, chat_name))?;
            match stmt.next()? { sqlite::State::Row => stmt.read(0)?, _ => unreachable!() }
        };

        {
            let mut stmt = conn.prepare(
                "INSERT INTO ChatMembers (chatID, userID) VALUES (?, ?);"
            )?;
            stmt.bind((1, chat_id))?;
            stmt.bind((2, user_id))?;
            stmt.next()?;
        }
        Ok(chat_id)
    }

    pub fn add_user_to_chat(&self, user_id: i64, chat_id: i64) -> Result<(), sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT INTO ChatMembers (chatID, userID) VALUES (?, ?);"
        )?;
        stmt.bind((1, chat_id))?;
        stmt.bind((2, user_id))?;
        stmt.next()?;
        Ok(())
    }

    pub fn get_chat_id_by_invite_code(&self, code: &str) -> Result<Option<i64>, sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT chatID FROM InviteCodes WHERE code = ? AND expires_at > datetime('now');"
        )?;
        stmt.bind((1, code))?;
        if let sqlite::State::Row = stmt.next()? {
            let chat_id: i64 = stmt.read(0)?;
            Ok(Some(chat_id))
        } else {
            Ok(None)
        }
    }

    pub fn create_invite_code(&self, chat_id: i64, code: &str) -> Result<(), sqlite::Error> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT INTO InviteCodes (code, chatID, expires_at) VALUES (?, ?, datetime('now', '+7 days'));"
        )?;
        stmt.bind((1, code))?;
        stmt.bind((2, chat_id))?;
        stmt.next()?;
        Ok(())
    }
}