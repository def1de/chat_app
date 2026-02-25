#[cfg(test)]
mod test {

    use crate::database::{Database};

    fn setup() -> Database {
        let db = Database::new(":memory:");
        db.create().unwrap();
        db
    }

    fn raw(conn: &Database, sql: &str) {
        conn.connection.lock().unwrap().execute(sql).unwrap();
    }

    // ------------------------
    // USERS
    // ------------------------

    #[test]
    fn add_and_get_user() {
        let db = setup();

        db.add_user("alice", "hash").unwrap();
        let user = db.get_user("alice").unwrap().unwrap();

        assert_eq!(user.1, "alice");
    }

    #[test]
    fn get_nonexistent_user() {
        let db = setup();
        assert!(db.get_user("ghost").unwrap().is_none());
    }

    #[test]
    fn password_check() {
        let db = setup();

        db.add_user("bob", "secret").unwrap();

        assert!(db.check_password("bob", "secret"));
        assert!(!db.check_password("bob", "wrong"));
        assert!(!db.check_password("ghost", "secret"));
    }

    // ------------------------
    // SESSIONS
    // ------------------------

    #[test]
    fn session_lifecycle() {
        let db = setup();

        raw(&db, "INSERT INTO Users (username, password_hash) VALUES ('u', 'p');");

        db.create_session(1, "token").unwrap();

        let session = db.validate_session("token").unwrap().unwrap();
        assert_eq!(session.0, 1);
        assert_eq!(session.1, "u");

        db.delete_session("token").unwrap();
        assert!(db.validate_session("token").unwrap().is_none());
    }

    #[test]
    fn session_fk_violation() {
        let db = setup();

        let result = db.create_session(999, "bad");
        assert!(result.is_err());
    }

    // ------------------------
    // CHATS + MEMBERSHIP
    // ------------------------

    #[test]
    fn create_chat_and_membership() {
        let db = setup();

        raw(&db, "INSERT INTO Users (username, password_hash) VALUES ('u', 'p');");

        let chat_id = db.create_chat("chat", 1).unwrap();

        assert!(db.check_chat_membership(1, chat_id).unwrap());

        let chats = db.get_user_chats(1).unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].1, "chat");
    }

    #[test]
    fn add_user_to_chat() {
        let db = setup();

        raw(&db, "
            INSERT INTO Users (username, password_hash) VALUES ('u1','p');
            INSERT INTO Users (username, password_hash) VALUES ('u2','p');
            INSERT INTO Chats (chat_name) VALUES ('chat');
            INSERT INTO ChatMembers (chatID, userID) VALUES (1,1);
        ");

        db.add_user_to_chat(2, 1).unwrap();

        assert!(db.check_chat_membership(2, 1).unwrap());
    }

    #[test]
    fn chat_membership_fk_violation() {
        let db = setup();

        let result = db.add_user_to_chat(1, 1);
        assert!(result.is_err());
    }

    // ------------------------
    // MESSAGES
    // ------------------------

    #[test]
    fn insert_and_get_messages() {
        let db = setup();

        raw(&db, "
            INSERT INTO Users (username, password_hash) VALUES ('u','p');
            INSERT INTO Chats (chat_name) VALUES ('chat');
            INSERT INTO ChatMembers (chatID, userID) VALUES (1,1);
        ");

        db.insert_message("hello", "u", 1).unwrap();
        db.insert_message("world", "u", 1).unwrap();

        let msgs = db.get_messages(1, 10).unwrap();

        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn message_chat_fk_violation() {
        let db = setup();

        let result = db.insert_message("hi", "ghost", 1);
        assert!(result.is_err());
    }

    // ------------------------
    // CASCADE DELETE
    // ------------------------

    #[test]
    fn cascade_delete_chat_deletes_messages() {
        let db = setup();

        raw(&db, "
            INSERT INTO Users (username, password_hash) VALUES ('u','p');
            INSERT INTO Chats (chat_name) VALUES ('chat');
            INSERT INTO ChatMembers (chatID, userID) VALUES (1,1);
            INSERT INTO Messages (message_text, username, chatID)
            VALUES ('hello','u',1);
        ");

        raw(&db, "DELETE FROM Chats WHERE chatID = 1;");

        let msgs = db.get_messages(1, 10).unwrap();
        assert_eq!(msgs.len(), 0);
    }

    // ------------------------
    // INVITE CODES
    // ------------------------

    #[test]
    fn invite_code_flow() {
        let db = setup();

        raw(&db, "
            INSERT INTO Users (username, password_hash) VALUES ('u','p');
            INSERT INTO Chats (chat_name) VALUES ('chat');
            INSERT INTO ChatMembers (chatID, userID) VALUES (1,1);
        ");

        db.create_invite_code(1, "code").unwrap();

        let chat_id = db.get_chat_id_by_invite_code("code").unwrap().unwrap();
        assert_eq!(chat_id, 1);
    }

    #[test]
    fn invite_code_fk_violation() {
        let db = setup();

        let result = db.create_invite_code(999, "bad");
        assert!(result.is_err());
    }

    #[test]
    fn invite_code_not_found() {
        let db = setup();
        assert!(db.get_chat_id_by_invite_code("ghost").unwrap().is_none());
    }
}