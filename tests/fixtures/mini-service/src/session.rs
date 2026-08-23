use crate::auth::{issue_token, verify_token};

pub fn start_session(user: &str) -> String {
    let token = issue_token(user);
    if verify_token(&token) {
        token
    } else {
        String::new()
    }
}
