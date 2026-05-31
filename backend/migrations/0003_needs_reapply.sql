-- Add needs_reapply flag: set true when selections change on an already-applied
-- provider, reset to false on successful apply. Used to show the "pending" yellow
-- dot in the UI.
ALTER TABLE providers ADD COLUMN needs_reapply BOOLEAN NOT NULL DEFAULT 0;
