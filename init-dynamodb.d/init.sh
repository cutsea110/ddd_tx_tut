#!/bin/bash

AWS_ACCESS_KEY_ID=dummy AWS_SECRET_ACCESS_KEY=dummy aws dynamodb \
	--region us-west-1 \
	--endpoint-url http://dynamodb:8000 \
	create-table \
	--table-name person \
	--attribute-definitions \
	AttributeName=PK,AttributeType=S \
	AttributeName=SK,AttributeType=S \
	--key-schema \
	AttributeName=PK,KeyType=HASH \
	AttributeName=SK,KeyType=RANGE \
	--provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5 \
	--billing-mode PAY_PER_REQUEST
