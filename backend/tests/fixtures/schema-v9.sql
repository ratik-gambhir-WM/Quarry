-- Frozen Quarry schema 9, before shared template persistence.

            CREATE TABLE app_metadata (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );

            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                first_name TEXT NOT NULL,
                last_name TEXT NOT NULL,
                email TEXT NOT NULL UNIQUE,
                api_key TEXT NOT NULL,
                role TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE assistant_threads (
                thread_id TEXT PRIMARY KEY NOT NULL,
                deal_id TEXT REFERENCES deals(deal_id) ON DELETE RESTRICT,
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'regular'
                    CHECK (status IN ('regular', 'archived')),
                metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_message_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                CHECK (length(trim(thread_id)) > 0),
                CHECK (length(trim(title)) > 0)
            );

            CREATE INDEX idx_assistant_threads_user_status_recent
                ON assistant_threads(user_id, status, last_message_at DESC, thread_id);

            CREATE TABLE assistant_messages (
                message_id TEXT PRIMARY KEY NOT NULL,
                thread_id TEXT NOT NULL REFERENCES assistant_threads(thread_id) ON DELETE CASCADE,
                parent_message_id TEXT REFERENCES assistant_messages(message_id),
                sequence INTEGER NOT NULL CHECK (sequence >= 0),
                role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
                content TEXT NOT NULL,
                status TEXT NOT NULL
                    CHECK (status IN ('streaming', 'completed', 'failed', 'cancelled')),
                model TEXT,
                request_id TEXT,
                error_code TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                completed_at TEXT,
                UNIQUE (thread_id, sequence),
                UNIQUE (thread_id, request_id),
                CHECK (length(trim(message_id)) > 0),
                CHECK (request_id IS NULL OR length(trim(request_id)) > 0)
            );

            CREATE INDEX idx_assistant_messages_thread_sequence
                ON assistant_messages(thread_id, sequence);
            CREATE INDEX idx_assistant_messages_thread_parent
                ON assistant_messages(thread_id, parent_message_id);

            CREATE TABLE reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                reminder TEXT NOT NULL,
                notes TEXT NOT NULL,
                date TEXT NOT NULL,
                link TEXT NOT NULL,
                time TEXT,
                deal TEXT,
                tag TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE deals (
                deal_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(deal_id)) > 0),
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
                deal_name TEXT NOT NULL CHECK (length(trim(deal_name)) > 0),
                deal_code TEXT CHECK (deal_code IS NULL OR length(trim(deal_code)) > 0),
                engagement_type TEXT NOT NULL CHECK (engagement_type IN ('buy_side', 'sell_side', 'other')),
                transaction_type TEXT NOT NULL CHECK (transaction_type IN ('platform_acquisition', 'add_on_acquisition', 'strategic_acquisition', 'carve_out', 'merger', 'divestiture', 'recapitalization', 'growth_equity', 'other', 'unknown')),
                status TEXT NOT NULL CHECK (status IN ('pipeline', 'active', 'completed', 'on_hold', 'cancelled')),
                start_date TEXT CHECK (start_date IS NULL OR length(trim(start_date)) > 0),
                expected_close_date TEXT CHECK (expected_close_date IS NULL OR length(trim(expected_close_date)) > 0),
                actual_close_date TEXT CHECK (actual_close_date IS NULL OR length(trim(actual_close_date)) > 0),
                engagement_completed_date TEXT CHECK (engagement_completed_date IS NULL OR length(trim(engagement_completed_date)) > 0),
                archived_at TEXT CHECK (archived_at IS NULL OR length(trim(archived_at)) > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX idx_deals_user_id ON deals(user_id);
            CREATE INDEX idx_deals_status ON deals(status);
            CREATE INDEX idx_deals_transaction_type ON deals(transaction_type);
            CREATE INDEX idx_deals_expected_close_date ON deals(expected_close_date);
            CREATE INDEX idx_deals_actual_close_date ON deals(actual_close_date);
            CREATE TABLE companies (
                company_id TEXT PRIMARY KEY NOT NULL,
                user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
                name TEXT NOT NULL CHECK (length(trim(name)) > 0),
                website TEXT CHECK (website IS NULL OR length(trim(website)) > 0),
                industry TEXT CHECK (industry IS NULL OR length(trim(industry)) > 0),
                description TEXT CHECK (description IS NULL OR length(trim(description)) > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX idx_companies_owner_name ON companies(user_id, name, company_id);
            CREATE TABLE deal_parties (
                deal_party_id TEXT PRIMARY KEY NOT NULL,
                deal_id TEXT NOT NULL REFERENCES deals(deal_id) ON DELETE CASCADE,
                company_id TEXT NOT NULL REFERENCES companies(company_id) ON DELETE RESTRICT,
                role TEXT NOT NULL CHECK (role IN ('target', 'buyer', 'seller', 'pe_sponsor', 'portfolio_company', 'parent_company', 'client', 'other')),
                notes TEXT CHECK (notes IS NULL OR length(trim(notes)) > 0),
                UNIQUE (deal_id, company_id, role)
            );
            CREATE INDEX idx_parties_deal_role ON deal_parties(deal_id, role);
            CREATE INDEX idx_parties_company ON deal_parties(company_id);
            CREATE TABLE deal_scopes (
                deal_id TEXT NOT NULL REFERENCES deals(deal_id) ON DELETE CASCADE,
                scope TEXT NOT NULL CHECK (scope IN ('product_architecture', 'corporate_it', 'cybersecurity', 'application_security', 'sdlc', 'it_organization', 'it_financials')),
                PRIMARY KEY (deal_id, scope)
            );
            CREATE TABLE deal_metadata (
                deal_id TEXT PRIMARY KEY NOT NULL REFERENCES deals(deal_id) ON DELETE CASCADE,
                key_questions_json TEXT NOT NULL DEFAULT '[]'
                    CHECK (json_valid(key_questions_json) AND json_type(key_questions_json) = 'array'),
                local_path TEXT CHECK (local_path IS NULL OR length(trim(local_path)) > 0),
                sharepoint_link TEXT CHECK (sharepoint_link IS NULL OR length(trim(sharepoint_link)) > 0),
                sow_link TEXT CHECK (sow_link IS NULL OR length(trim(sow_link)) > 0),
                fact_sheet_link TEXT CHECK (fact_sheet_link IS NULL OR length(trim(fact_sheet_link)) > 0),
                rl_link TEXT CHECK (rl_link IS NULL OR length(trim(rl_link)) > 0),
                CHECK (NOT (local_path IS NOT NULL AND sharepoint_link IS NOT NULL))
            );

            CREATE TABLE quarry_files (
                file_id       TEXT PRIMARY KEY NOT NULL,
                deal_id       TEXT NOT NULL REFERENCES deals(deal_id) ON DELETE CASCADE,
                workspace_id  TEXT NOT NULL,
                display_name  TEXT NOT NULL,
                source_uri    TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL,
                deleted_at    TEXT,
                CHECK (length(trim(file_id)) > 0),
                CHECK (length(trim(deal_id)) > 0),
                CHECK (length(trim(workspace_id)) > 0),
                CHECK (length(trim(display_name)) > 0),
                CHECK (source_uri IS NULL OR length(trim(source_uri)) > 0)
            );

            CREATE TABLE quarry_file_versions (
                version_id        TEXT PRIMARY KEY NOT NULL,
                file_id           TEXT NOT NULL REFERENCES quarry_files(file_id) ON DELETE CASCADE,
                version_number    INTEGER NOT NULL CHECK (version_number > 0),
                original_filename TEXT NOT NULL,
                mime_type         TEXT NOT NULL,
                content_sha256    TEXT NOT NULL,
                byte_size         INTEGER NOT NULL CHECK (byte_size >= 0),
                is_current        INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
                created_at        TEXT NOT NULL,
                UNIQUE (file_id, version_number),
                UNIQUE (file_id, content_sha256),
                CHECK (length(trim(version_id)) > 0),
                CHECK (length(trim(original_filename)) > 0),
                CHECK (length(content_sha256) = 64)
            );

            CREATE UNIQUE INDEX uq_quarry_file_versions_current
                ON quarry_file_versions(file_id)
                WHERE is_current = 1;
            CREATE INDEX idx_quarry_files_deal
                ON quarry_files(deal_id, deleted_at);
            CREATE INDEX idx_quarry_files_workspace_deal
                ON quarry_files(workspace_id, deal_id, deleted_at);
            CREATE INDEX idx_quarry_file_versions_file
                ON quarry_file_versions(file_id, version_number DESC);
            CREATE INDEX idx_quarry_file_versions_hash
                ON quarry_file_versions(content_sha256);

            CREATE TABLE quarry_file_blobs (
                version_id  TEXT PRIMARY KEY NOT NULL
                    REFERENCES quarry_file_versions(version_id) ON DELETE CASCADE,
                file_bytes  BLOB NOT NULL
            );

            CREATE INDEX idx_assistant_threads_deal_recent
                ON assistant_threads(user_id, deal_id, status, last_message_at DESC, thread_id);

PRAGMA user_version = 9;
