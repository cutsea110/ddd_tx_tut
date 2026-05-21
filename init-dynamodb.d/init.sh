#!/bin/bash

set -e

until curl -s http://dynamodb:8000 > /dev/null; do
  echo "Waiting for DynamoDB..."
  sleep 1
done

export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy

create_table_if_not_exists() {
  local table=$1
  if aws dynamodb --region us-west-1 --endpoint-url http://dynamodb:8000 \
      describe-table --table-name "$table" > /dev/null 2>&1; then
    echo "Table '$table' already exists, skipping."
    return 0
  fi
  aws dynamodb \
    --region us-west-1 \
    --endpoint-url http://dynamodb:8000 \
    create-table \
    --table-name "$table" \
    --attribute-definitions \
      AttributeName=PK,AttributeType=S \
      AttributeName=SK,AttributeType=S \
    --key-schema \
      AttributeName=PK,KeyType=HASH \
      AttributeName=SK,KeyType=RANGE \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5 \
    --billing-mode PAY_PER_REQUEST
  echo "Table '$table' created."
}

create_table_if_not_exists person
