CREATE TABLE IF NOT EXISTS lambda_tasks (
    id UUID PRIMARY KEY,
    batch_id UUID,
    kind JSONB NOT NULL,
    timeout_secs INTEGER NOT NULL,
    status VARCHAR(20) NOT NULL,
    result JSONB,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_lambda_tasks_status ON lambda_tasks(status);
CREATE INDEX IF NOT EXISTS idx_lambda_tasks_batch_id ON lambda_tasks(batch_id);
CREATE INDEX IF NOT EXISTS idx_lambda_tasks_created_at ON lambda_tasks(created_at);
