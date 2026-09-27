use poker_backend::auth::{create_token, validate_token};

#[test]
fn test_create_and_validate_token() {
    let secret = "super-secret-key-123";
    let user_id = "507f1f77bcf86cd799439011";
    let username = "alice";

    let token = create_token(user_id, username, secret).expect("token creation should succeed");
    assert!(!token.is_empty());

    let claims = validate_token(&token, secret).expect("token validation should succeed");
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.username, username);
    assert!(claims.exp > chrono::Utc::now().timestamp() as usize);
}

#[test]
fn test_token_with_invalid_secret_fails() {
    let secret = "correct-secret";
    let wrong_secret = "wrong-secret";
    let token = create_token("user1", "alice", secret).unwrap();

    let result = validate_token(&token, wrong_secret);
    assert!(result.is_err());
}

#[test]
fn test_tampered_token_fails() {
    let secret = "test-secret";
    let token = create_token("user1", "alice", secret).unwrap();
    let tampered = format!("{token}tampered");

    let result = validate_token(&tampered, secret);
    assert!(result.is_err());
}

#[test]
fn test_bcrypt_hashing_and_verification() {
    let password = "mySecurePassword123!";
    let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).unwrap();

    assert!(bcrypt::verify(password, &hash).unwrap());
    assert!(!bcrypt::verify("wrongPassword", &hash).unwrap());
}
