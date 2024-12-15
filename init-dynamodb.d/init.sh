#!/bin/bash

AWS_ACCESS_KEY_ID=dummy AWS_SECRET_ACCESS_KEY=dummy aws dynamodb \
	--region us-west-1 \
	--endpoint-url http://dynamodb:8000 \
	create-table --table-name counter \
	--key-schema AttributeName=table_name,KeyType=HASH \
	--attribute-definitions AttributeName=table_name,AttributeType=S \
	--billing-mode PAY_PER_REQUEST

AWS_ACCESS_KEY_ID=dummy AWS_SECRET_ACCESS_KEY=dummy aws dynamodb \
	--region us-west-1 \
	--endpoint-url http://dynamodb:8000 \
	update-item \
	--table-name counter \
	--key '{"table_name": {"S": "person"}}' \
	--update-expression "SET id = if_not_exists(id, :start) + :incr" \
	--expression-attribute-values '{":start": {"N": "0"},":incr": {"N": "1"}}' \
	--return-values UPDATED_NEW

AWS_ACCESS_KEY_ID=dummy AWS_SECRET_ACCESS_KEY=dummy aws dynamodb \
	--region us-west-1 \
	--endpoint-url http://dynamodb:8000 \
	create-table --table-name person \
	--key-schema AttributeName=id,KeyType=HASH \
	--attribute-definitions AttributeName=id,AttributeType=N \
	--provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5 \
	--billing-mode PAY_PER_REQUEST
