-- Schema v4: the change log asks "has this group been undone?" for every group of a page and, for
-- the unreviewed view, for every group of the log. Without an index each answer scans the table.
CREATE INDEX change_log_reverts ON change_log(reverts_group);
