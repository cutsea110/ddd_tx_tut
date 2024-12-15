#!/bin/bash
AWS_ACCESS_KEY_ID=dummy AWS_SECRET_ACCESS_KEY=dummy aws dynamodb \
	--region us-west-1 \
	--endpoint-url http://dynamodb:8000 \
	create-table --table-name person \
	--key-schema AttributeName=id,KeyType=HASH AttributeName=name,KeyType=RANGE \
	--attribute-definitions AttributeName=id,AttributeType=N AttributeName=name,AttributeType=S \
	--provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5 \
	--billing-mode PAY_PER_REQUEST
