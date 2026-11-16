-- Run explicitly with the migration role against each selected database.
-- Never invoked by application startup or per-test schema setup/cleanup.
\set ON_ERROR_STOP on
BEGIN;
CREATE SCHEMA IF NOT EXISTS quarry_extensions;
REVOKE CREATE ON SCHEMA quarry_extensions FROM PUBLIC;
CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA quarry_extensions VERSION '0.8.6';
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_extension e JOIN pg_namespace n ON n.oid=e.extnamespace
        WHERE e.extname='vector' AND e.extversion='0.8.6' AND n.nspname='quarry_extensions'
    ) THEN
        RAISE EXCEPTION 'vector must be provisioned at version 0.8.6 in quarry_extensions; inspect existing installation without dropping it';
    END IF;
END
$$;
COMMIT;
