set -e
psql -U admin sampledb <<EOSQL
CREATE TABLE person (
  id           SERIAL PRIMARY KEY,
  name         TEXT NOT NULL,
  birth_date   DATE NOT NULL,
  data         BYTEA
);
EOSQL
