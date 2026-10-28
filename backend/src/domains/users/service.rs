pub use crate::domains::users::repository::{AddUserInput, User};

use crate::shared::error::ServiceResult;

use super::repository::UserRepository;

#[derive(Clone)]
pub struct UserService {
    users: UserRepository,
}

impl UserService {
    pub fn new(users: UserRepository) -> Self {
        Self { users }
    }

    pub async fn create(&self, input: AddUserInput) -> ServiceResult<User> {
        self.users.create(input).await.map_err(Into::into)
    }
}
