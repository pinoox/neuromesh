pub fn verify_token(token: &str) -> bool {
    !token.is_empty() && token.len() > 8
}

pub fn issue_token(user: &str) -> String {
    format!("tok_{user}_secret")
}
