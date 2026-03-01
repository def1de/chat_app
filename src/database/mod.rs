use crate::template::MessageView;
use sqlx::SqlitePool;

// Module to run tests on the database
// mod test;

pub struct Database {
    pool: SqlitePool,
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Database {
            pool: self.pool.clone(),
        }
    }
}

impl Database {
    pub async fn new(path: &str) -> Self {
        let pool = SqlitePool::connect(path)
            .await
            .expect("Error opening database");

        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await
            .expect("Failed to enable foreign keys");

        Database { pool: pool }
    }

    pub async fn create(&self) -> Result<(), sqlx::Error> {
        println!("Creating database schema...");
        sqlx::query(
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_message(
        &mut self,
        message_text: &str,
        username: &str,
        chat_id: i64,
    ) -> Result<(), sqlx::Error> {
        // TODO: Maybe use query_as()
        sqlx::query!(
            r#"INSERT INTO Messages (message_text, username, chatID) VALUES (?, ?, ?);"#,
            message_text,
            username,
            chat_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_messages(
        &mut self,
        chat_id: i64,
        limit: i64,
    ) -> Result<Vec<MessageView>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT m.username, m.message_text
                        FROM Messages AS m
                        JOIN Chats AS c ON c.chatID = m.chatID
                        WHERE c.chatID = ?
                        ORDER BY timestamp DESC LIMIT ?;"#,
            chat_id,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        let mut messages = Vec::new();

        for row in rows {
            let message = MessageView {
                username: row.username,
                text: row.message_text,
            };
            messages.push(message);
        }

        Ok(messages)
    }

    pub async fn get_user(&mut self, username: &str) -> Result<Option<(i64, String)>, sqlx::Error> {
        let user = sqlx::query!(
            r#"SELECT userID, username FROM Users WHERE username = ?;"#,
            username
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Some((user.userID, user.username)))
    }

    pub async fn check_password(&mut self, username: &str, password_hash: &str) -> bool {
        let result = sqlx::query!(
            r#"SELECT password_hash FROM Users WHERE username = ?;"#,
            username
        )
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(row)) => row.password_hash == password_hash,
            _ => false,
        }
    }

    pub async fn add_user(
        &mut self,
        username: &str,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        Box::new(sqlx::query!(
            "INSERT INTO Users (username, password_hash) VALUES (?, ?);",
            username,
            password_hash
        ))
        .execute(&self.pool)
        .await?;

        Ok(())
    }
    //
    pub async fn create_session(
        &mut self,
        user_id: i64,
        session_token: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"INSERT INTO Sessions (userID, session_token, expires_at) VALUES (?, ?, datetime('now', '+7 days'));"#,
            user_id,
            session_token,
        ).execute(&self.pool).await?;

        Ok(())
    }

    pub async fn validate_session(
        &self,
        session_token: &str,
    ) -> Result<Option<(i64, String)>, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT s.userID, u.username
                        FROM Sessions AS s
                        JOIN Users AS u ON u.userID = s.userID
                        WHERE session_token = ? AND expires_at > datetime('now');"#,
            session_token,
        )
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some((row.userID, row.username))),
            None => Ok(None),
        }
    }

    pub async fn delete_session(&mut self, session_token: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"DELETE FROM Sessions WHERE session_token = ?;"#,
            session_token
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn check_chat_membership(
        &mut self,
        user_id: i64,
        chat_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT * FROM ChatMembers WHERE userID = ? AND chatID = ?;"#,
            user_id,
            chat_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.is_some())
    }

    pub async fn get_user_chats(
        &mut self,
        user_id: i64,
    ) -> Result<Vec<(i64, String)>, sqlx::Error> {
        let chats_query = sqlx::query!(
            r#"SELECT c.chatID, c.chat_name
                        FROM Chats AS c
                        JOIN ChatMembers AS cm ON cm.chatID = c.chatID
                        WHERE cm.userID = ?;"#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;

        let mut chats = Vec::new();

        for chat in chats_query {
            chats.push((chat.chatID, chat.chat_name));
        }

        Ok(chats)
    }

    pub async fn create_chat(&mut self, chat_name: &str, user_id: i64) -> Result<i64, sqlx::Error> {
        let chat_id: i64 = {
            sqlx::query!(
                r#"INSERT INTO Chats (chat_name) VALUES (?) RETURNING chatID;"#,
                chat_name
            )
            .fetch_one(&self.pool)
            .await?
            .chatID
        };

        {
            sqlx::query!(
                r#"INSERT INTO ChatMembers (chatID, userID) VALUES (?, ?);"#,
                chat_id,
                user_id
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(chat_id)
    }

    pub async fn add_user_to_chat(
        &mut self,
        user_id: i64,
        chat_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"INSERT INTO ChatMembers (chatID, userID) VALUES (?, ?);"#,
            chat_id,
            user_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_chat_id_by_invite_code(
        &mut self,
        code: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT chatID FROM InviteCodes WHERE code = ? AND expires_at > datetime('now');"#,
            code
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(row.chatID)),
            None => Ok(None),
        }
    }

    pub async fn create_invite_code(
        &mut self,
        chat_id: i64,
        code: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"INSERT INTO InviteCodes (code, chatID, expires_at) VALUES (?, ?, datetime('now', '+7 days'));"#,
            code,
            chat_id,
        ).execute(&self.pool).await?;

        Ok(())
    }
}
