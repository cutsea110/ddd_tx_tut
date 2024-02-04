set -e
psql -U admin sampledb <<EOSQL
CREATE TABLE person (
  id    SERIAL PRIMARY KEY,
  name  TEXT NOT NULL,
  age   INTEGER NOT NULL,
  data  BYTEA
);
EOSQL
