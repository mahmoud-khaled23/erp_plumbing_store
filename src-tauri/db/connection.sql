-- Execute for EVERY connection, before any migration/transaction; verify returned values.
-- journal_mode=WAL is persistent, foreign_keys and busy_timeout are per connection.
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA busy_timeout = 5000;
PRAGMA recursive_triggers = ON;
PRAGMA temp_store = MEMORY;
PRAGMA cache_size = -8192;
PRAGMA wal_autocheckpoint = 1000;
