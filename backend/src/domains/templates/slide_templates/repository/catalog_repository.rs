//! Canonical template catalog, asset reads and atomic publication.
use super::*;

#[async_trait::async_trait]
impl TemplateAssetRepository for PostgresTemplateRepository {
    async fn find_asset(&self, id: &str, asset: &str) -> RepositoryResult<Option<TemplateAsset>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    Ok(tx
                        .read_one_with(
                            SqlBuilder::select("template_assets")
                                .and_where(Condition::equal("template_id", id))
                                .and_where(Condition::equal("asset_id", asset))
                                .build(),
                            map_asset,
                        )
                        .await?)
                })
                .await
        })
        .await
    }
}

#[async_trait::async_trait]
impl TemplateCatalogRepository for PostgresTemplateRepository {
    async fn insert_many_with_pending_classification_cancellable(
        &self,
        records: &[(TemplateInsert, PendingSlideClassification)],
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(async { self.insert_classifications(records, cancellation).await })
            .await
    }
    async fn insert_with_pending_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(async { self.insert_classification(record, pending, None).await }).await
    }
    async fn insert(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            Ok(self
                .insert_many(&[TemplateInsert {
                    template: template.clone(),
                    assets: assets.to_vec(),
                    metadata: metadata.clone(),
                    preview: preview.cloned(),
                }])
                .await?)
        })
        .await
    }
    async fn insert_many(&self, records: &[TemplateInsert]) -> RepositoryResult<()> {
        repository_boundary(async { Ok(self.insert_many_cancellable(records, None).await?) }).await
    }
    async fn insert_many_cancellable(
        &self,
        records: &[TemplateInsert],
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            for record in records {
                validate_records(
                    &record.template,
                    &record.assets,
                    &record.metadata,
                    record.preview.as_ref(),
                )?;
            }
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    check_publication(cancellation)?;
                    for record in records {
                        self.write_new(
                            tx,
                            &record.template,
                            &record.assets,
                            &record.metadata,
                            record.preview.as_ref(),
                        )
                        .await?;
                    }
                    check_publication(cancellation)
                })
                .await
        })
        .await
    }
    async fn insert_with_pending_classification_cancellable(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            self.insert_classification(record, pending, cancellation)
                .await
        })
        .await
    }
    async fn update(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            ensure!(
                assets.iter().all(|a| a.template_id == template.template_id),
                "asset ownership mismatch"
            );
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    ensure!(
                        self.visible(tx, &template.template_id).await?,
                        "template is unavailable to this user"
                    );
                    let builtin = tx
                        .read_one_with(
                            SqlBuilder::select("template_metadata")
                                .columns(["source"])
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .build(),
                            |r| r.get::<_, String>(0),
                        )
                        .await?
                        .is_some_and(|s| s == "builtin");
                    let json = serde_json::to_string(&if builtin {
                        normalized_document(&template.template_json)
                    } else {
                        compact_document(&template.template_json)
                    })?;
                    let changed = tx
                        .write(
                            SqlBuilder::update("templates")
                                .set("template_json", json)
                                .set(
                                    "revision",
                                    crate::shared::revision::next(template.revision)?,
                                )
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .and_where(Condition::equal("revision", template.revision))
                                .build(),
                        )
                        .await?;
                    if changed.rows_affected != 1 {
                        return Err(crate::shared::revision::conflict().into());
                    }
                    insert_assets(tx, assets).await?;
                    Ok(())
                })
                .await
        })
        .await
    }
    async fn upsert_builtin(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            validate_records(template, assets, metadata, preview)?;
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let initial_json =
                        serde_json::to_string(&normalized_document(&template.template_json))?;
                    tx.write(
                        SqlBuilder::insert_into("templates")
                            .value("template_id", &template.template_id)
                            .value("template_json", initial_json)
                            .value(
                                "owner_user_id",
                                self.owner_user_id
                                    .context("template owner scope is required")?,
                            )
                            .on_conflict_do_nothing(["template_id"])
                            .build(),
                    )
                    .await?;
                    let current = tx
                        .lock_with(
                            SqlBuilder::select("templates")
                                .columns(["revision"])
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .lock(RowLock::Update, LockWait::Wait)
                                .build(),
                            |r| r.get::<_, i64>(0),
                        )
                        .await?
                        .pop()
                        .context("missing builtin template")?;
                    ensure!(
                        self.visible(tx, &template.template_id).await?,
                        "template is unavailable to this user"
                    );
                    let existing = tx
                        .read_one_with(
                            SqlBuilder::select("template_metadata")
                                .columns(["source", "checksum"])
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
                        )
                        .await?;
                    let refreshing = existing.is_some();
                    if existing
                        .is_some_and(|(s, c)| s == "import" || c == metadata.catalog_checksum)
                    {
                        return Ok(());
                    }
                    let json =
                        serde_json::to_string(&normalized_document(&template.template_json))?;
                    tx.write(
                        SqlBuilder::insert_into("templates")
                            .value("template_id", &template.template_id)
                            .value("template_json", json)
                            .value(
                                "owner_user_id",
                                self.owner_user_id
                                    .context("template owner scope is required")?,
                            )
                            .on_conflict_update(
                                ConflictUpdate::new(["template_id"]).set_excluded("template_json"),
                            )
                            .build(),
                    )
                    .await?;
                    if refreshing {
                        tx.write(
                            SqlBuilder::update("templates")
                                .set("revision", crate::shared::revision::next(current)?)
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .and_where(Condition::equal("revision", current))
                                .build(),
                        )
                        .await?;
                    }
                    for table in ["template_assets", "template_previews"] {
                        tx.write(
                            SqlBuilder::delete_from(table)
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .build(),
                        )
                        .await?;
                    }
                    write_metadata(tx, metadata, true).await?;
                    insert_assets(tx, assets).await?;
                    write_preview(tx, preview).await?;
                    Ok(())
                })
                .await
        })
        .await
    }
    async fn find_by_id(&self, id: &str) -> RepositoryResult<Option<StoredTemplate>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    let json = tx
                        .read_one_with(
                            SqlBuilder::select("templates")
                                .columns(["template_json", "revision"])
                                .and_where(Condition::equal("template_id", id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>("revision")?)),
                        )
                        .await?;
                    match json {
                        Some((json, revision)) => Ok(Some(StoredTemplate {
                            revision,
                            template_id: id.into(),
                            template_json: decode_document(json).await?,
                        })),
                        None => Ok(None),
                    }
                })
                .await
        })
        .await
    }
    async fn find_metadata(&self, id: &str) -> RepositoryResult<Option<StoredTemplateMetadata>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    metadata_in(tx, id).await
                })
                .await
        })
        .await
    }
    async fn find_preview(&self, id: &str) -> RepositoryResult<Option<StoredTemplatePreview>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    Ok(tx
                        .read_one_with(
                            SqlBuilder::select("template_previews")
                                .columns(["template_previews.*", "templates.revision"])
                                .inner_join(
                                    "templates",
                                    "templates.template_id",
                                    "template_previews.template_id",
                                )
                                .and_where(Condition::equal("template_previews.template_id", id))
                                .build(),
                            map_preview,
                        )
                        .await?)
                })
                .await
        })
        .await
    }
    async fn list_previews(
        &self,
        limit: usize,
        offset: usize,
    ) -> RepositoryResult<StoredTemplatePreviewPage> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let query = SqlBuilder::select("template_previews")
                        .columns(["template_previews.*", "templates.revision"])
                        .join(JoinClause::inner("templates").on(JoinCondition::equal(
                            "templates.template_id",
                            "template_previews.template_id",
                        )))
                        .join(
                            JoinClause::inner("template_metadata").on(JoinCondition::equal(
                                "template_metadata.template_id",
                                "template_previews.template_id",
                            )),
                        )
                        .and_where(self.owner_condition());
                    let total = usize::try_from(
                        tx.read_one_with(query.clone().count_rows().build(), |r| {
                            r.get::<_, i64>(0)
                        })
                        .await?
                        .unwrap_or(0),
                    )?;
                    let previews = tx
                        .read_with(
                            query
                                .order_by("template_metadata.created_at", SortDirection::Descending)
                                .order_by("templates.catalog_sequence", SortDirection::Descending)
                                .limit(limit)
                                .offset(offset)
                                .build(),
                            map_preview,
                        )
                        .await?;
                    Ok(StoredTemplatePreviewPage { previews, total })
                })
                .await
        })
        .await
    }
    async fn list(
        &self,
        kind: Option<TemplateKind>,
    ) -> RepositoryResult<Vec<StoredTemplateCatalogItem>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let mut query = SqlBuilder::select("templates")
                        .columns([
                            "templates.template_id",
                            "templates.template_json",
                            "templates.revision",
                        ])
                        .join(
                            JoinClause::inner("template_metadata").on(JoinCondition::equal(
                                "template_metadata.template_id",
                                "templates.template_id",
                            )),
                        )
                        .and_where(self.owner_condition())
                        .order_by("template_metadata.created_at", SortDirection::Descending)
                        .order_by("templates.catalog_sequence", SortDirection::Descending);
                    if let Some(kind) = kind {
                        query = query
                            .and_where(Condition::equal("template_metadata.kind", kind.as_str()));
                    }
                    let rows = tx
                        .read_with(query.build(), |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, i64>("revision")?,
                            ))
                        })
                        .await?;
                    let mut items = Vec::with_capacity(rows.len());
                    for (id, json, revision) in rows {
                        let metadata = metadata_in(tx, &id)
                            .await?
                            .context("missing template metadata")?;
                        let has_preview = tx
                            .read_one(
                                SqlBuilder::select("template_previews")
                                    .columns(["template_id"])
                                    .and_where(Condition::equal("template_id", &id))
                                    .build(),
                            )
                            .await?
                            .is_some();
                        items.push(StoredTemplateCatalogItem {
                            template: StoredTemplate {
                                revision,
                                template_id: id,
                                template_json: decode_document(json).await?,
                            },
                            metadata,
                            has_preview,
                        })
                    }
                    Ok(items)
                })
                .await
        })
        .await
    }
    async fn delete(&self, id: &str, expected_revision: i64) -> RepositoryResult<bool> {
        repository_boundary(async {
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let revisions = tx
                        .lock_with(
                            SqlBuilder::select("templates")
                                .columns(["revision"])
                                .and_where(Condition::equal("template_id", id))
                                .lock(RowLock::Update, LockWait::Wait)
                                .build(),
                            |r| r.get::<_, i64>(0),
                        )
                        .await?;
                    if !self.visible(tx, id).await? {
                        return Ok(false);
                    }
                    if revisions.first().copied() != Some(expected_revision) {
                        return Err(crate::shared::revision::conflict().into());
                    }
                    // Lexical rows and every owned artifact cascade from the canonical template.
                    let changed = tx
                        .write(
                            SqlBuilder::delete_from("templates")
                                .and_where(Condition::equal("template_id", id))
                                .and_where(self.owner_condition())
                                .build(),
                        )
                        .await?
                        .rows_affected;
                    Ok(changed > 0)
                })
                .await
        })
        .await
    }
    async fn find_by_id_with_assets(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredTemplateWithAssets>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    let (json, revision) = tx
                        .read_one_with(
                            SqlBuilder::select("templates")
                                .columns(["template_json", "revision"])
                                .and_where(Condition::equal("template_id", id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>("revision")?)),
                        )
                        .await?
                        .context("missing template")?;
                    let assets = tx
                        .read_with(
                            SqlBuilder::select("template_assets")
                                .and_where(Condition::equal("template_id", id))
                                .order_by("asset_id", SortDirection::Ascending)
                                .build(),
                            map_asset,
                        )
                        .await?;
                    Ok(Some(StoredTemplateWithAssets {
                        revision,
                        template_id: id.into(),
                        template_json: decode_document(json).await?,
                        assets,
                    }))
                })
                .await
        })
        .await
    }
}
