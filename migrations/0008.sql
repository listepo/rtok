-- T8.5: call edges. `end_line` closes a definition's span; `scope` is the innermost
-- definition enclosing a reference in the same file ('' at file level), which is the edge
-- `callers` groups by and `impact` walks. Existing rows keep 0/'' and are rebuilt on the
-- next index run, which the 0-stat from 0007 already forces for any file that changes.
ALTER TABLE symbols ADD COLUMN end_line INTEGER NOT NULL DEFAULT 0;
ALTER TABLE symbols ADD COLUMN scope TEXT NOT NULL DEFAULT '';
