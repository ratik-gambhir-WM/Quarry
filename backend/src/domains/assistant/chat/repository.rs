use serde::Serialize;

use crate::{
    adapters::sqlite::{
        client::{SqlRow, SqliteClient, SqliteClientError},
        query::{
            ComparisonOperator, Condition, SortDirection, SqlBuilder, SqlBuilderError, SqlQuery,
            SqlValue,
        },
    },
    shared::error::RepositoryError,
};

const THREAD_COLUMNS: [&str; 6] = [
    "thread_id",
    "title",
    "status",
    "created_at",
    "updated_at",
    "last_message_at",
];
const MESSAGE_COLUMNS: [&str; 10] = [
    "message_id",
    "thread_id",
    "parent_message_id",
    "sequence",
    "role",
    "content",
    "status",
    "model",
    "created_at",
    "completed_at",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantThread {
    pub thread_id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_message_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessage {
    pub message_id: String,
    pub thread_id: String,
    pub parent_message_id: Option<String>,
    pub sequence: i64,
    pub role: String,
    pub content: String,
    pub status: String,
    pub model: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantThreadDetail {
    #[serde(flatten)]
    pub thread: AssistantThread,
    pub messages: Vec<AssistantMessage>,
}

#[derive(Clone, Debug)]
pub struct StartRunInput {
    pub assistant_message_id: String,
    pub model: String,
    pub parent_message_id: Option<String>,
    pub prompt: String,
    pub request_id: String,
    pub thread_id: String,
    pub user_id: i64,
    pub user_message_id: String,
}

#[derive(Clone, Debug)]
pub enum StartRunResult {
    Started {
        prior_messages: Vec<AssistantMessage>,
    },
    Existing(AssistantMessage),
    Conflict,
}

#[derive(Clone)]
pub struct AssistantChatRepository {
    sqlite: SqliteClient,
}

impl AssistantChatRepository {
    pub fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }

    pub async fn create_thread(
        &self,
        user_id: i64,
        thread_id: String,
    ) -> Result<AssistantThread, RepositoryError> {
        let insert = query(
            SqlBuilder::insert_into("assistant_threads")
                .value("thread_id", &thread_id)
                .value("user_id", user_id)
                .value("title", "New chat")
                .on_conflict_do_nothing(["thread_id"])
                .build(),
            "assistant thread insert",
        )?;
        self.sqlite
            .write_async(insert)
            .await
            .map_err(storage("create assistant thread"))?;
        self.thread_by_id(user_id, thread_id).await?.ok_or_else(|| {
            RepositoryError::storage("inserted assistant thread could not be loaded")
        })
    }

    pub async fn list_threads(
        &self,
        user_id: i64,
        status: String,
        limit: usize,
        before: Option<String>,
    ) -> Result<Vec<AssistantThread>, RepositoryError> {
        let mut builder = SqlBuilder::select("assistant_threads")
            .columns(THREAD_COLUMNS)
            .and_where(Condition::equal("user_id", user_id))
            .and_where(Condition::equal("status", &status))
            .order_by("last_message_at", SortDirection::Descending)
            .order_by("thread_id", SortDirection::Descending)
            .limit(limit);
        if let Some(before) = before {
            if let Some((last_message_at, thread_id)) = before.rsplit_once('|') {
                if !last_message_at.is_empty() && !thread_id.is_empty() {
                    builder = builder
                        .and_where(Condition::compare(
                            "last_message_at",
                            ComparisonOperator::LessThan,
                            last_message_at,
                        ))
                        .or_where(Condition::equal("user_id", user_id))
                        .and_where(Condition::equal("status", &status))
                        .and_where(Condition::equal("last_message_at", last_message_at))
                        .and_where(Condition::compare(
                            "thread_id",
                            ComparisonOperator::LessThan,
                            thread_id,
                        ));
                } else {
                    builder = builder.and_where(Condition::compare(
                        "last_message_at",
                        ComparisonOperator::LessThan,
                        before,
                    ));
                }
            } else {
                builder = builder.and_where(Condition::compare(
                    "last_message_at",
                    ComparisonOperator::LessThan,
                    before,
                ));
            }
        }
        let query = query(builder.build(), "assistant thread list")?;
        let rows = self
            .sqlite
            .read_async(query)
            .await
            .map_err(storage("list assistant threads"))?;
        rows.into_iter().map(thread_from_row).collect()
    }

    pub async fn get_thread(
        &self,
        user_id: i64,
        thread_id: String,
    ) -> Result<Option<AssistantThreadDetail>, RepositoryError> {
        let Some(thread) = self.thread_by_id(user_id, thread_id.clone()).await? else {
            return Ok(None);
        };
        let query = query(
            SqlBuilder::select("assistant_messages")
                .columns(MESSAGE_COLUMNS)
                .and_where(Condition::equal("thread_id", thread_id))
                .order_by("sequence", SortDirection::Ascending)
                .build(),
            "assistant message list",
        )?;
        let rows = self
            .sqlite
            .read_async(query)
            .await
            .map_err(storage("load assistant messages"))?;
        let mut messages = rows
            .into_iter()
            .map(message_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        normalize_legacy_user_parents(&mut messages);
        Ok(Some(AssistantThreadDetail { thread, messages }))
    }

    pub async fn rename_thread(
        &self,
        user_id: i64,
        thread_id: String,
        title: String,
    ) -> Result<bool, RepositoryError> {
        self.update_thread(user_id, thread_id, "title", title).await
    }

    pub async fn set_thread_status(
        &self,
        user_id: i64,
        thread_id: String,
        status: String,
    ) -> Result<bool, RepositoryError> {
        self.update_thread(user_id, thread_id, "status", status)
            .await
    }

    async fn update_thread(
        &self,
        user_id: i64,
        thread_id: String,
        column: &'static str,
        value: String,
    ) -> Result<bool, RepositoryError> {
        let updated_at = now_timestamp();
        let update = query(
            SqlBuilder::update("assistant_threads")
                .set(column, value)
                .set("updated_at", updated_at)
                .and_where(Condition::equal("thread_id", thread_id))
                .and_where(Condition::equal("user_id", user_id))
                .build(),
            "assistant thread update",
        )?;
        let result = self
            .sqlite
            .write_async(update)
            .await
            .map_err(storage("update assistant thread"))?;
        Ok(result.rows_affected == 1)
    }

    pub async fn delete_thread(
        &self,
        user_id: i64,
        thread_id: String,
    ) -> Result<bool, RepositoryError> {
        let delete = query(
            SqlBuilder::delete_from("assistant_threads")
                .and_where(Condition::equal("thread_id", thread_id))
                .and_where(Condition::equal("user_id", user_id))
                .build(),
            "assistant thread delete",
        )?;
        let result = self
            .sqlite
            .write_async(delete)
            .await
            .map_err(storage("delete assistant thread"))?;
        Ok(result.rows_affected == 1)
    }

    pub async fn start_run(&self, input: StartRunInput) -> Result<StartRunResult, RepositoryError> {
        self.sqlite
            .transaction_async(move |transaction| {
                let owned_query = transaction_query(
                    SqlBuilder::select("assistant_threads")
                        .columns(["thread_id", "title"])
                        .and_where(Condition::equal("thread_id", &input.thread_id))
                        .and_where(Condition::equal("user_id", input.user_id))
                        .build(),
                    "assistant thread ownership select",
                )?;
                let Some(thread_row) = transaction.read_one(&owned_query)? else {
                    return Err(SqliteClientError::transaction_aborted(
                        "assistant thread was not found",
                    ));
                };
                let existing_query = transaction_query(
                    SqlBuilder::select("assistant_messages")
                        .columns(MESSAGE_COLUMNS)
                        .and_where(Condition::equal("thread_id", &input.thread_id))
                        .and_where(Condition::equal("request_id", &input.request_id))
                        .build(),
                    "assistant request id select",
                )?;
                if let Some(row) = transaction.read_one(&existing_query)? {
                    return Ok(StartRunResult::Existing(transaction_message(row)?));
                }
                if input.assistant_message_id == input.user_message_id {
                    return Ok(StartRunResult::Conflict);
                }
                let assistant_message_query = transaction_query(
                    SqlBuilder::select("assistant_messages")
                        .columns(["message_id"])
                        .and_where(Condition::equal("message_id", &input.assistant_message_id))
                        .build(),
                    "assistant message id select",
                )?;
                if transaction.read_one(&assistant_message_query)?.is_some() {
                    return Ok(StartRunResult::Conflict);
                }
                let user_message_query = transaction_query(
                    SqlBuilder::select("assistant_messages")
                        .columns(MESSAGE_COLUMNS)
                        .and_where(Condition::equal("message_id", &input.user_message_id))
                        .build(),
                    "assistant user message id select",
                )?;
                let existing_user_sequence = match transaction.read_one(&user_message_query)? {
                    Some(row) => {
                        let message = transaction_message(row)?;
                        let parent_matches = if message.parent_message_id.as_deref()
                            == Some(message.message_id.as_str())
                        {
                            let previous_assistant_query = transaction_query(
                                SqlBuilder::select("assistant_messages")
                                    .columns(["message_id"])
                                    .and_where(Condition::equal("thread_id", &input.thread_id))
                                    .and_where(Condition::equal("role", "assistant"))
                                    .and_where(Condition::compare(
                                        "sequence",
                                        ComparisonOperator::LessThan,
                                        message.sequence,
                                    ))
                                    .order_by("sequence", SortDirection::Descending)
                                    .limit(1)
                                    .build(),
                                "assistant previous message select",
                            )?;
                            let expected_parent = transaction
                                .read_one(&previous_assistant_query)?
                                .map(|row| transaction_text(&row, "message_id"))
                                .transpose()?;
                            expected_parent == input.parent_message_id
                        } else {
                            message.parent_message_id == input.parent_message_id
                        };
                        if message.thread_id != input.thread_id
                            || !parent_matches
                            || message.role != "user"
                            || message.content != input.prompt
                            || message.status != "completed"
                        {
                            return Ok(StartRunResult::Conflict);
                        }
                        Some(message.sequence)
                    }
                    None => None,
                };
                let latest_query = transaction_query(
                    SqlBuilder::select("assistant_messages")
                        .columns(["sequence"])
                        .and_where(Condition::equal("thread_id", &input.thread_id))
                        .order_by("sequence", SortDirection::Descending)
                        .limit(1)
                        .build(),
                    "assistant latest sequence select",
                )?;
                let next_sequence = transaction
                    .read_one(&latest_query)?
                    .map(|row| transaction_integer(&row, "sequence"))
                    .transpose()?
                    .map(|sequence| {
                        sequence.checked_add(1).ok_or_else(|| {
                            SqliteClientError::transaction_aborted(
                                "assistant message sequence overflow",
                            )
                        })
                    })
                    .transpose()?
                    .unwrap_or(0);
                let mut history_builder = SqlBuilder::select("assistant_messages")
                    .columns(MESSAGE_COLUMNS)
                    .and_where(Condition::equal("thread_id", &input.thread_id))
                    .and_where(Condition::equal("status", "completed"));
                if let Some(sequence) = existing_user_sequence {
                    history_builder = history_builder.and_where(Condition::compare(
                        "sequence",
                        ComparisonOperator::LessThan,
                        sequence,
                    ));
                }
                let history_query = transaction_query(
                    history_builder
                        .order_by("sequence", SortDirection::Descending)
                        .limit(64)
                        .build(),
                    "assistant context select",
                )?;
                let mut prior_messages = transaction
                    .read(&history_query)?
                    .into_iter()
                    .map(transaction_message)
                    .collect::<Result<Vec<_>, _>>()?;
                prior_messages.reverse();
                if existing_user_sequence.is_none() {
                    let user_insert = transaction_query(
                        SqlBuilder::insert_into("assistant_messages")
                            .value("message_id", &input.user_message_id)
                            .value("thread_id", &input.thread_id)
                            .value("parent_message_id", input.parent_message_id.as_deref())
                            .value("sequence", next_sequence)
                            .value("role", "user")
                            .value("content", &input.prompt)
                            .value("status", "completed")
                            .build(),
                        "assistant user message insert",
                    )?;
                    transaction.write(&user_insert)?;
                }
                let assistant_sequence = if existing_user_sequence.is_some() {
                    next_sequence
                } else {
                    next_sequence.checked_add(1).ok_or_else(|| {
                        SqliteClientError::transaction_aborted(
                            "assistant message sequence overflow",
                        )
                    })?
                };
                let assistant_insert = transaction_query(
                    SqlBuilder::insert_into("assistant_messages")
                        .value("message_id", &input.assistant_message_id)
                        .value("thread_id", &input.thread_id)
                        .value("parent_message_id", &input.user_message_id)
                        .value("sequence", assistant_sequence)
                        .value("role", "assistant")
                        .value("content", "")
                        .value("status", "streaming")
                        .value("model", &input.model)
                        .value("request_id", &input.request_id)
                        .build(),
                    "assistant pending message insert",
                )?;
                transaction.write(&assistant_insert)?;
                let title = transaction_text(&thread_row, "title")?;
                let updated_at = now_timestamp();
                let mut thread_update = SqlBuilder::update("assistant_threads")
                    .set("updated_at", &updated_at)
                    .set("last_message_at", updated_at)
                    .and_where(Condition::equal("thread_id", &input.thread_id))
                    .and_where(Condition::equal("user_id", input.user_id));
                if title == "New chat" {
                    thread_update = thread_update.set("title", derive_title(&input.prompt));
                }
                let thread_update =
                    transaction_query(thread_update.build(), "assistant thread activity update")?;
                transaction.write(&thread_update)?;
                Ok(StartRunResult::Started { prior_messages })
            })
            .await
            .map_err(|error| match error {
                SqliteClientError::TransactionAborted(message)
                    if message == "assistant thread was not found" =>
                {
                    RepositoryError::invalid(message)
                }
                other => {
                    RepositoryError::storage(format!("failed to start assistant run: {other}"))
                }
            })
    }

    pub async fn finish_run(
        &self,
        message_id: String,
        status: &'static str,
        content: String,
        error_code: Option<&'static str>,
    ) -> Result<(), RepositoryError> {
        let completed_at = now_timestamp();
        let update = query(
            SqlBuilder::update("assistant_messages")
                .set("content", content)
                .set("status", status)
                .set("error_code", error_code)
                .set("completed_at", completed_at)
                .and_where(Condition::equal("message_id", message_id))
                .and_where(Condition::equal("status", "streaming"))
                .build(),
            "assistant run finalization",
        )?;
        let result = self
            .sqlite
            .write_async(update)
            .await
            .map_err(storage("finish assistant run"))?;
        if result.rows_affected != 1 {
            return Err(RepositoryError::storage("assistant run was not pending"));
        }
        Ok(())
    }

    async fn thread_by_id(
        &self,
        user_id: i64,
        thread_id: String,
    ) -> Result<Option<AssistantThread>, RepositoryError> {
        let query = query(
            SqlBuilder::select("assistant_threads")
                .columns(THREAD_COLUMNS)
                .and_where(Condition::equal("thread_id", thread_id))
                .and_where(Condition::equal("user_id", user_id))
                .build(),
            "assistant thread select",
        )?;
        self.sqlite
            .read_one_async(query)
            .await
            .map_err(storage("load assistant thread"))?
            .map(thread_from_row)
            .transpose()
    }
}

fn query(
    result: Result<SqlQuery, SqlBuilderError>,
    operation: &str,
) -> Result<SqlQuery, RepositoryError> {
    result
        .map_err(|error| RepositoryError::storage(format!("failed to build {operation}: {error}")))
}

fn transaction_query(
    result: Result<SqlQuery, SqlBuilderError>,
    operation: &str,
) -> Result<SqlQuery, SqliteClientError> {
    result.map_err(|error| {
        SqliteClientError::transaction_aborted(format!("failed to build {operation}: {error}"))
    })
}

fn storage(context: &'static str) -> impl FnOnce(SqliteClientError) -> RepositoryError {
    move |error| RepositoryError::storage(format!("failed to {context}: {error}"))
}

fn thread_from_row(row: SqlRow) -> Result<AssistantThread, RepositoryError> {
    Ok(AssistantThread {
        thread_id: text(&row, "thread_id")?,
        title: text(&row, "title")?,
        status: text(&row, "status")?,
        created_at: text(&row, "created_at")?,
        updated_at: text(&row, "updated_at")?,
        last_message_at: text(&row, "last_message_at")?,
    })
}

fn message_from_row(row: SqlRow) -> Result<AssistantMessage, RepositoryError> {
    Ok(AssistantMessage {
        message_id: text(&row, "message_id")?,
        thread_id: text(&row, "thread_id")?,
        parent_message_id: optional_text(&row, "parent_message_id")?,
        sequence: integer(&row, "sequence")?,
        role: text(&row, "role")?,
        content: text(&row, "content")?,
        status: text(&row, "status")?,
        model: optional_text(&row, "model")?,
        created_at: text(&row, "created_at")?,
        completed_at: optional_text(&row, "completed_at")?,
    })
}

fn transaction_message(row: SqlRow) -> Result<AssistantMessage, SqliteClientError> {
    message_from_row(row).map_err(|error| SqliteClientError::transaction_aborted(error.to_string()))
}

fn normalize_legacy_user_parents(messages: &mut [AssistantMessage]) {
    let mut previous_assistant_id = None;
    for message in messages {
        if message.role == "user"
            && message.parent_message_id.as_deref() == Some(message.message_id.as_str())
        {
            message.parent_message_id = previous_assistant_id.clone();
        }
        if message.role == "assistant" {
            previous_assistant_id = Some(message.message_id.clone());
        }
    }
}

fn text(row: &SqlRow, column: &str) -> Result<String, RepositoryError> {
    match row.get(column) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(RepositoryError::invalid(format!(
            "assistant row has invalid `{column}`"
        ))),
    }
}

fn optional_text(row: &SqlRow, column: &str) -> Result<Option<String>, RepositoryError> {
    match row.get(column) {
        Some(SqlValue::Text(value)) => Ok(Some(value.clone())),
        Some(SqlValue::Null) => Ok(None),
        _ => Err(RepositoryError::invalid(format!(
            "assistant row has invalid `{column}`"
        ))),
    }
}

fn integer(row: &SqlRow, column: &str) -> Result<i64, RepositoryError> {
    match row.get(column) {
        Some(SqlValue::Integer(value)) => Ok(*value),
        _ => Err(RepositoryError::invalid(format!(
            "assistant row has invalid `{column}`"
        ))),
    }
}

fn transaction_text(row: &SqlRow, column: &str) -> Result<String, SqliteClientError> {
    text(row, column).map_err(|error| SqliteClientError::transaction_aborted(error.to_string()))
}

fn transaction_integer(row: &SqlRow, column: &str) -> Result<i64, SqliteClientError> {
    integer(row, column).map_err(|error| SqliteClientError::transaction_aborted(error.to_string()))
}

fn derive_title(prompt: &str) -> String {
    let title = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = title.chars();
    let shortened = chars.by_ref().take(80).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

fn now_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/assistant/chat/repository_tests.rs"]
mod tests;
