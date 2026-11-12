//! Indexed prefix retrieval with the existing FTS5 weighted BM25 formula.
//! The supported query grammar is the safe quoted-prefix AND/OR expression emitted
//! by template retrieval. No SQL, operators, or identifiers come from query input.
use super::unicode61;
use super::{Condition, QueryKind, SqlBuilder, SqlQuery, SqlValue};
use crate::adapters::postgres::client::{PostgresClient, PostgresError, PostgresTransaction};
use std::collections::BTreeMap;
const WEIGHTS: [f64; 15] = [2., 5., 5., 5., 2., 2., 3., 3., 4., 3., 1., 4., 4., 2., 3.];
pub(crate) fn tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if unicode61::is_token(c) || (!current.is_empty() && unicode61::is_diacritic(c)) {
            if let Some(c) = unicode61::fold(c) {
                current.push(c);
            }
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}
pub(crate) async fn index(
    tx: &PostgresTransaction,
    id: &str,
    fields: &[String],
) -> Result<(), PostgresError> {
    let mut length = 0i64;
    for (field, text) in fields.iter().enumerate() {
        let mut counts = BTreeMap::new();
        for term in tokens(text) {
            *counts.entry(term).or_insert(0i64) += 1;
            length += 1;
        }
        for (term, count) in counts {
            tx.write(
                SqlBuilder::insert_into("template_lexical_terms")
                    .value("template_id", id)
                    .value("term", term)
                    .value(
                        "field_index",
                        i64::try_from(field).map_err(PostgresError::decode)?,
                    )
                    .value("frequency", count)
                    .build(),
            )
            .await?;
        }
    }
    tx.write(
        SqlBuilder::update("slide_classifications_fts")
            .set("token_count", length)
            .and_where(Condition::equal("template_id", id))
            .build(),
    )
    .await?;
    Ok(())
}
fn prefixes(expression: &str) -> Result<(Vec<(String, bool)>, bool), PostgresError> {
    let all = !expression.contains(" OR ");
    let separator = if all { " AND " } else { " OR " };
    let mut terms = Vec::new();
    for part in expression.split(separator) {
        let prefix = part.ends_with('*');
        let word = if prefix {
            &part[..part.len() - 1]
        } else {
            part
        };
        let word = word
            .strip_prefix('"')
            .and_then(|p| p.strip_suffix('"'))
            .unwrap_or(word);
        if word.chars().any(|c| !c.is_alphanumeric()) {
            return Err(PostgresError::transaction_aborted(
                "invalid lexical expression",
            ));
        }
        let parsed = tokens(word);
        if parsed.len() != 1 {
            return Err(PostgresError::transaction_aborted("invalid lexical token"));
        }
        terms.push((parsed[0].clone(), prefix));
    }
    if terms.is_empty() || terms.len() > 200 {
        return Err(PostgresError::transaction_aborted(
            "invalid lexical expression",
        ));
    }
    Ok((terms, all))
}
pub(crate) fn search_query(
    owner: Option<i64>,
    app: &str,
    expression: &str,
    limit: usize,
) -> Result<SqlQuery, PostgresError> {
    let (prefixes, all) = prefixes(expression)?;
    let total = prefixes.len();
    let mut params = vec![SqlValue::from(owner), SqlValue::from(app)];
    let mut values = Vec::new();
    for (i, (term, prefix)) in prefixes.into_iter().enumerate() {
        let mut upper = term.chars().collect::<Vec<_>>();
        let last = upper
            .pop()
            .ok_or_else(|| PostgresError::transaction_aborted("empty lexical term"))?;
        let next = u32::from(last)
            .checked_add(1)
            .and_then(|n| char::from_u32(if n == 0xd800 { 0xe000 } else { n }))
            .ok_or_else(|| PostgresError::transaction_aborted("invalid lexical range"))?;
        upper.push(next);
        params.push(term.into());
        let lower_parameter = params.len();
        params.push(upper.into_iter().collect::<String>().into());
        values.push(format!(
            "({i},${lower_parameter}::text,${}::text,{prefix})",
            params.len()
        ));
    }
    // Both document frequency and document length use the complete lexical corpus,
    // matching FTS5 even when results are filtered by app/owner.
    let sql = format!(
        r#"
        WITH prefixes(ordinal,term,upper_bound,is_prefix) AS (VALUES {}),
        corpus AS (SELECT document_count::float8 AS n, total_tokens::float8 / greatest(document_count,1) AS avdl FROM template_lexical_stats WHERE singleton=1),
        hits AS MATERIALIZED (
            SELECT p.ordinal,t.template_id,sum(t.frequency* (ARRAY[{}]::float8[])[t.field_index+1]) AS frequency
            FROM prefixes p JOIN template_lexical_terms t ON ((p.is_prefix AND t.term>=p.term AND t.term<p.upper_bound) OR (NOT p.is_prefix AND t.term=p.term))
            GROUP BY p.ordinal,t.template_id
        ),
        df AS (SELECT ordinal,count(*)::float8 AS n FROM hits GROUP BY ordinal),
        ranks AS (
            SELECT h.template_id,count(*) AS matched,
              -sum((CASE WHEN c.n<=2*df.n THEN 1e-6 ELSE ln((c.n-df.n+0.5)/(df.n+0.5)) END) *
              (h.frequency*2.2)/(h.frequency+1.2*(0.25+0.75*f.token_count/nullif(c.avdl,0))) ORDER BY h.ordinal) AS score
            FROM hits h JOIN df USING(ordinal) JOIN slide_classifications_fts f USING(template_id) CROSS JOIN corpus c
            GROUP BY h.template_id
        )
        SELECT r.template_id FROM ranks r
        JOIN templates t USING(template_id) JOIN app_templates a USING(template_id)
        JOIN template_classification_status s USING(template_id)
        WHERE t.owner_user_id IS NOT DISTINCT FROM $1::bigint AND a.app_id=$2
          AND s.metadata_json IS NOT NULL AND s.schema_version=2 {}
        ORDER BY r.score ASC,r.template_id COLLATE "C" ASC LIMIT {}
    "#,
        values.join(","),
        WEIGHTS
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        if all {
            format!("AND r.matched={total}")
        } else {
            String::new()
        },
        limit.min(200)
    );
    Ok(SqlQuery::new(sql, params, QueryKind::Select))
}
pub(crate) async fn search(
    client: &PostgresClient,
    owner: Option<i64>,
    app: &str,
    expression: &str,
    limit: usize,
) -> Result<Vec<String>, PostgresError> {
    client
        .read_with_async(search_query(owner, app, expression, limit), |r| r.get(0))
        .await
}
#[cfg(test)]
#[path = "../../../../tests/unit/adapters/postgres/query/lexical_tests.rs"]
mod tests;
