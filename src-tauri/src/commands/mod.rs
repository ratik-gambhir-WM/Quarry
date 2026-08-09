pub mod data_room;
pub mod deal;
pub mod documents;
pub mod research;
pub mod users;

pub use crate::errors::{AppError, ErrorCode};

pub type CommandResult<T> = Result<T, AppError>;

pub trait CommandResultExt<T> {
    fn command_context(self, command_name: &str) -> CommandResult<T>;
    fn validation_context(self, command_name: &str) -> CommandResult<T>;
}

impl<T, E> CommandResultExt<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn command_context(self, command_name: &str) -> CommandResult<T> {
        self.map_err(|err| AppError::from_source(command_name, err))
    }

    fn validation_context(self, command_name: &str) -> CommandResult<T> {
        self.map_err(|err| AppError::validation(command_name, err.to_string()))
    }
}

#[cfg(test)]
#[path = "../../tests/commands/mod_tests.rs"]
mod tests;
