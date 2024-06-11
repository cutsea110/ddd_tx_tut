set -e
psql -U admin sampledb <<EOSQL
CREATE TABLE person (
  id           SERIAL PRIMARY KEY,
  name         TEXT NOT NULL,
  birth_date   DATE NOT NULL,
  death_date   DATE,
  data         BYTEA,

  revision     INT NOT NULL
);
EOSQL
